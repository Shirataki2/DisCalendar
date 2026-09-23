//! 外部 ICS の安全な取得と表示期間への展開。DB の予定には混ぜない。
use std::{
    sync::Arc,
    time::{Duration as StdDuration, Instant},
};

use chrono::{Duration, NaiveDateTime};
use reqwest::header;
use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;

use crate::{
    ical_import::{self, ParsedImportEvent},
    outbound_http, recurrence,
    state::AppState,
};

pub const MAX_CALENDARS: i64 = 5;
pub const MAX_EVENTS: usize = 200;
const MAX_VISIBLE: usize = 1_000;
const FRESH_FOR: StdDuration = StdDuration::from_secs(30 * 60);

#[derive(Debug, Clone, FromRow)]
pub struct Calendar {
    pub id: i64,
    pub guild_id: String,
    pub url: String,
    pub name: String,
    pub color: String,
    pub created_by: String,
    pub created_at: NaiveDateTime,
    pub last_fetched_at: Option<NaiveDateTime>,
    pub last_error: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CalendarView {
    pub id: i64,
    pub name: String,
    pub color: String,
    /// 限定公開 URL の場合があるため、管理権限を持つ人だけに返す。
    pub url: Option<String>,
    pub created_by: String,
    pub created_at: NaiveDateTime,
    pub last_fetched_at: Option<NaiveDateTime>,
    pub last_error: Option<String>,
}

impl Calendar {
    pub fn view(&self, can_manage: bool) -> CalendarView {
        CalendarView {
            id: self.id,
            name: self.name.clone(),
            color: self.color.clone(),
            url: can_manage.then(|| self.url.clone()),
            created_by: self.created_by.clone(),
            created_at: self.created_at,
            last_fetched_at: self.last_fetched_at,
            last_error: self.last_error.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CachedFeed {
    fetched_at: Instant,
    url: String,
    events: Arc<Vec<ParsedImportEvent>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExternalEvent {
    pub id: String,
    pub calendar_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub is_all_day: bool,
    pub start_at: NaiveDateTime,
    pub end_at: NaiveDateTime,
}

/// 失敗しても他の購読先は表示する。エラーは URL や外部レスポンスを含まない定型文だけにする。
pub async fn load(
    state: &AppState,
    calendar: &mut Calendar,
    force: bool,
) -> Option<Arc<Vec<ParsedImportEvent>>> {
    let cached = state
        .external_feeds
        .get(&calendar.id)
        .await
        .filter(|feed| feed.url == calendar.url);
    if !force
        && let Some(feed) = &cached
        && feed.fetched_at.elapsed() < FRESH_FOR
    {
        return Some(feed.events.clone());
    }
    let _slot = state.external_fetch_slots.acquire().await.ok()?;
    // セマフォ待ち中に別リクエストが取得を終えた場合はその結果を使う。
    if !force
        && let Some(feed) = state
            .external_feeds
            .get(&calendar.id)
            .await
            .filter(|feed| feed.url == calendar.url)
        && feed.fetched_at.elapsed() < FRESH_FOR
    {
        return Some(feed.events);
    }
    let outcome =
        tokio::time::timeout(StdDuration::from_secs(10), fetch(calendar, cached.as_ref())).await;
    let result = match outcome {
        Ok(Ok(FetchResult::Fresh {
            events,
            etag,
            last_modified,
        })) => {
            calendar.etag = etag;
            calendar.last_modified = last_modified;
            let events = Arc::new(events);
            state
                .external_feeds
                .insert(
                    calendar.id,
                    CachedFeed {
                        fetched_at: Instant::now(),
                        url: calendar.url.clone(),
                        events: events.clone(),
                    },
                )
                .await;
            calendar.last_error = None;
            Some(events)
        }
        Ok(Ok(FetchResult::Unchanged)) => {
            if let Some(feed) = cached {
                state
                    .external_feeds
                    .insert(
                        calendar.id,
                        CachedFeed {
                            fetched_at: Instant::now(),
                            url: calendar.url.clone(),
                            events: feed.events.clone(),
                        },
                    )
                    .await;
                calendar.last_error = None;
                Some(feed.events)
            } else {
                calendar.last_error = Some("再取得してください".into());
                None
            }
        }
        Ok(Err(message)) => {
            calendar.last_error = Some(message.into());
            None
        }
        Err(_) => {
            calendar.last_error = Some("取得がタイムアウトしました".into());
            None
        }
    };
    calendar.last_fetched_at = Some(crate::models::now_jst());
    let _ = sqlx::query("UPDATE guild_external_calendars SET last_fetched_at=$2,last_error=$3,etag=$4,last_modified=$5 WHERE id=$1 AND guild_id=$6 AND url=$7")
        .bind(calendar.id).bind(calendar.last_fetched_at).bind(&calendar.last_error)
        .bind(&calendar.etag).bind(&calendar.last_modified).bind(&calendar.guild_id).bind(&calendar.url)
        .execute(&state.pool).await;
    result
}

enum FetchResult {
    Fresh {
        events: Vec<ParsedImportEvent>,
        etag: Option<String>,
        last_modified: Option<String>,
    },
    Unchanged,
}

async fn fetch(
    calendar: &Calendar,
    cached: Option<&CachedFeed>,
) -> Result<FetchResult, &'static str> {
    let mut url = outbound_http::parse_url(&calendar.url)?;
    for hop in 0..=3 {
        let client = outbound_http::client_for(&url).await?;
        let mut request = client.get(url.clone());
        if hop == 0 && cached.is_some() {
            if let Some(etag) = &calendar.etag {
                request = request.header(header::IF_NONE_MATCH, etag);
            }
            if let Some(modified) = &calendar.last_modified {
                request = request.header(header::IF_MODIFIED_SINCE, modified);
            }
        }
        let mut response = request.send().await.map_err(|_| "接続に失敗しました")?;
        if response.status().is_redirection() {
            if hop == 3 {
                return Err("リダイレクトが多すぎます");
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or("リダイレクト先が不正です")?;
            url = outbound_http::parse_url(
                url.join(location)
                    .map_err(|_| "リダイレクト先が不正です")?
                    .as_str(),
            )?;
            continue;
        }
        if response.status() == reqwest::StatusCode::NOT_MODIFIED && cached.is_some() {
            return Ok(FetchResult::Unchanged);
        }
        if !response.status().is_success() {
            return Err("取得先がエラーを返しました");
        }
        let etag = safe_validator(response.headers().get(header::ETAG));
        let last_modified = safe_validator(response.headers().get(header::LAST_MODIFIED));
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "読み込みに失敗しました")?
        {
            if body.len() + chunk.len() > ical_import::ICS_MAX_BYTES {
                return Err("ICS が 1 MiB を超えています");
            }
            body.extend_from_slice(&chunk);
        }
        let text = std::str::from_utf8(&body).map_err(|_| "ICS の文字コードを読み取れません")?;
        let parsed = ical_import::parse(text).map_err(|_| "ICS を解析できません")?;
        if parsed.total_count > MAX_EVENTS {
            return Err("予定が 200 件を超えています");
        }
        return Ok(FetchResult::Fresh {
            events: parsed.events,
            etag,
            last_modified,
        });
    }
    Err("リダイレクトが多すぎます")
}

fn safe_validator(value: Option<&header::HeaderValue>) -> Option<String> {
    value
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.len() <= 256 && !v.contains(['\r', '\n']))
        .map(str::to_owned)
}

pub fn visible(
    events: &[ParsedImportEvent],
    calendar_id: i64,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> Vec<ExternalEvent> {
    let mut result = Vec::new();
    for item in events {
        let event = &item.event;
        let duration = event.end_at - event.start_at;
        let overlap = duration
            + if event.is_all_day {
                Duration::days(1)
            } else {
                Duration::zero()
            };
        let starts = if let Some(rule) = event.recurrence.as_ref().filter(|rule| rule.enabled()) {
            rule.rrule(event.start_at)
                .ok()
                .and_then(|rrule| {
                    recurrence::between(
                        &rrule,
                        event.start_at,
                        start
                            .checked_sub_signed(overlap)
                            .unwrap_or(NaiveDateTime::MIN),
                        end,
                    )
                    .ok()
                })
                .unwrap_or_default()
        } else {
            vec![event.start_at]
        };
        for occurrence in starts {
            if occurrence < end
                && occurrence
                    .checked_add_signed(overlap)
                    .unwrap_or(NaiveDateTime::MAX)
                    > start
                && let Some(ends_at) = occurrence.checked_add_signed(duration)
            {
                result.push(ExternalEvent {
                    id: format!("{calendar_id}:{}:{occurrence}", item.source_index),
                    calendar_id,
                    name: event.name.clone(),
                    description: event.description.clone(),
                    is_all_day: event.is_all_day,
                    start_at: occurrence,
                    end_at: ends_at,
                });
                if result.len() == MAX_VISIBLE {
                    return result;
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn displays_only_overlapping_events() {
        let parsed = ical_import::parse("BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:one\nDTSTAMP:20260101T000000Z\nDTSTART:20261001T100000\nDTEND:20261001T110000\nSUMMARY:大会\nEND:VEVENT\nEND:VCALENDAR").unwrap();
        let start = "2026-10-01T10:30:00".parse().unwrap();
        let end = "2026-10-01T11:30:00".parse().unwrap();
        assert_eq!(visible(&parsed.events, 1, start, end).len(), 1);
        assert!(visible(&parsed.events, 1, end, end + Duration::hours(1)).is_empty());
    }
}
