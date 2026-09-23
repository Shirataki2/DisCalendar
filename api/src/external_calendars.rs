//! 外部 ICS の安全な取得と表示期間への展開。DB の予定には混ぜない。
use std::{
    sync::Arc,
    time::{Duration as StdDuration, Instant},
};

use chrono::{Duration, FixedOffset, NaiveDateTime};
use mailrs_ical::CalDateTime;
use reqwest::header;
use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;

use crate::{
    ical_import::{self, ParsedCalendar},
    outbound_http, recurrence,
    state::AppState,
};

pub const MAX_CALENDARS: i64 = 5;
pub const MAX_EVENTS: usize = 200;
const MAX_VISIBLE: usize = 1_000;
const FRESH_FOR: StdDuration = StdDuration::from_secs(30 * 60);
const RETRY_AFTER_FAILURE: StdDuration = StdDuration::from_secs(60);

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
    fetched_at_jst: NaiveDateTime,
    url: String,
    events: Option<Arc<ParsedCalendar>>,
    error: Option<String>,
}

impl CachedFeed {
    fn is_fresh(&self) -> bool {
        self.fetched_at.elapsed()
            < if self.events.is_some() {
                FRESH_FOR
            } else {
                RETRY_AFTER_FAILURE
            }
    }

    fn read(&self, calendar: &mut Calendar) -> Option<Arc<ParsedCalendar>> {
        calendar.last_fetched_at = Some(self.fetched_at_jst);
        calendar.last_error = self.error.clone();
        self.events.clone()
    }
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
) -> Option<Arc<ParsedCalendar>> {
    let cached = state
        .external_feeds
        .get(&calendar.id)
        .await
        .filter(|feed| feed.url == calendar.url);
    if !force
        && let Some(feed) = &cached
        && feed.is_fresh()
    {
        return feed.read(calendar);
    }
    let _slot = state.external_fetch_slots.acquire().await.ok()?;
    // セマフォ待ち中に別リクエストが取得を終えた場合はその結果を使う。
    if !force
        && let Some(feed) = state
            .external_feeds
            .get(&calendar.id)
            .await
            .filter(|feed| feed.url == calendar.url)
        && feed.is_fresh()
    {
        return feed.read(calendar);
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
            calendar.last_error = None;
            Some(Arc::new(events))
        }
        Ok(Ok(FetchResult::Unchanged)) => {
            if let Some(feed) = cached {
                calendar.last_error = None;
                feed.events
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
    let fetched_at_jst = crate::models::now_jst();
    calendar.last_fetched_at = Some(fetched_at_jst);
    state
        .external_feeds
        .insert(
            calendar.id,
            CachedFeed {
                fetched_at: Instant::now(),
                fetched_at_jst,
                url: calendar.url.clone(),
                events: result.clone(),
                error: calendar.last_error.clone(),
            },
        )
        .await;
    let _ = sqlx::query("UPDATE guild_external_calendars SET last_fetched_at=$2,last_error=$3,etag=$4,last_modified=$5 WHERE id=$1 AND guild_id=$6 AND url=$7")
        .bind(calendar.id).bind(calendar.last_fetched_at).bind(&calendar.last_error)
        .bind(&calendar.etag).bind(&calendar.last_modified).bind(&calendar.guild_id).bind(&calendar.url)
        .execute(&state.pool).await;
    result
}

enum FetchResult {
    Fresh {
        events: ParsedCalendar,
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
        if hop == 0 && cached.is_some_and(|feed| feed.events.is_some()) {
            if let Some(etag) = &calendar.etag {
                request = request.header(header::IF_NONE_MATCH, etag);
            }
            if let Some(modified) = &calendar.last_modified {
                request = request.header(header::IF_MODIFIED_SINCE, modified);
            }
        }
        let mut response = request.send().await.map_err(|_| "接続に失敗しました")?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED
            && cached.is_some_and(|feed| feed.events.is_some())
        {
            return Ok(FetchResult::Unchanged);
        }
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
        if parsed
            .skipped
            .iter()
            .any(|item| item.reason == "recurrence_exceptions")
        {
            return Err("例外付きの繰り返し予定には対応していません");
        }
        if parsed.skipped.iter().any(|item| item.reason != "cancelled") {
            return Err("未対応の予定、または読み取れない予定が含まれています");
        }
        return Ok(FetchResult::Fresh {
            events: parsed,
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
    parsed: &ParsedCalendar,
    calendar_id: i64,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> Vec<ExternalEvent> {
    let mut result = Vec::new();
    for item in &parsed.events {
        let event = &item.event;
        let duration = event.end_at - event.start_at;
        let overlap = duration
            + if event.is_all_day {
                Duration::days(1)
            } else {
                Duration::zero()
            };
        let starts = if let Some(rule) = event.recurrence.as_ref().filter(|rule| rule.enabled()) {
            match parsed.sources.get(&item.source_index) {
                Some(source) if matches!(source.start, CalDateTime::Zoned { .. }) => {
                    zoned_starts(source, &parsed.vtimezones, start, end, overlap)
                }
                _ => rule
                    .rrule(event.start_at)
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
                    .unwrap_or_default(),
            }
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

/// 元の壁時計で各回を計算してから JST に変換する。UTC の UNTIL は変換後に判定する。
fn zoned_starts(
    source: &ical_import::SourceRecurrence,
    vtimezones: &[mailrs_ical::VTimezone],
    start: NaiveDateTime,
    end: NaiveDateTime,
    overlap: Duration,
) -> Vec<NaiveDateTime> {
    let CalDateTime::Zoned { tz_name, local } = &source.start else {
        return Vec::new();
    };
    let Some(raw) = source.rule.as_deref() else {
        return Vec::new();
    };
    let parts: Vec<_> = raw.split(';').collect();
    let until = parts.iter().find_map(|part| {
        part.split_once('=')
            .filter(|(key, _)| key.eq_ignore_ascii_case("UNTIL"))
            .map(|(_, value)| value)
    });
    let local_rule = parts
        .iter()
        .filter(|part| !part.to_ascii_uppercase().starts_with("UNTIL="))
        .copied()
        .collect::<Vec<_>>()
        .join(";");
    let Some(starts) = ical_import::parse_rule(&local_rule, *local)
        .ok()
        .and_then(|rule| rule.rrule(*local).ok())
        .and_then(|rule| {
            let from = start
                .checked_sub_signed(overlap + Duration::days(2))
                .unwrap_or(NaiveDateTime::MIN);
            let to = end
                .checked_add_signed(Duration::days(2))
                .unwrap_or(NaiveDateTime::MAX);
            recurrence::between(&rule, *local, from, to).ok()
        })
    else {
        return Vec::new();
    };
    starts
        .into_iter()
        .filter_map(|local| {
            let utc = mailrs_ical::vtimezone::caldatetime_to_utc(
                &CalDateTime::Zoned {
                    tz_name: tz_name.clone(),
                    local,
                },
                vtimezones,
            )?;
            if !until.is_none_or(|value| {
                if value.ends_with('Z') {
                    NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ")
                        .is_ok_and(|limit| utc.naive_utc() <= limit)
                } else if value.len() == 8 {
                    chrono::NaiveDate::parse_from_str(value, "%Y%m%d")
                        .is_ok_and(|limit| local.date() <= limit)
                } else {
                    NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")
                        .is_ok_and(|limit| local <= limit)
                }
            }) {
                return None;
            }
            Some(
                utc.with_timezone(&FixedOffset::east_opt(9 * 3600).expect("JST offset is valid"))
                    .naive_local(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn displays_only_overlapping_events() {
        let parsed = ical_import::parse("BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:one\nDTSTAMP:20260101T000000Z\nDTSTART:20261001T100000\nDTEND:20261001T110000\nSUMMARY:大会\nEND:VEVENT\nEND:VCALENDAR").unwrap();
        let start = "2026-10-01T10:30:00".parse().unwrap();
        let end = "2026-10-01T11:30:00".parse().unwrap();
        assert_eq!(visible(&parsed, 1, start, end).len(), 1);
        assert!(visible(&parsed, 1, end, end + Duration::hours(1)).is_empty());
    }

    #[test]
    fn expands_zoned_recurrence_across_dst_in_source_timezone() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:ny-weekly\nDTSTAMP:20260101T000000Z\nDTSTART;TZID=America/New_York:20261025T090000\nDTEND;TZID=America/New_York:20261025T100000\nRRULE:FREQ=WEEKLY;BYDAY=SU;COUNT=3\nSUMMARY:Morning\nEND:VEVENT\nEND:VCALENDAR";
        let parsed = ical_import::parse(ics).unwrap();
        let start = "2026-10-25T00:00:00".parse().unwrap();
        let end = "2026-11-09T00:00:00".parse().unwrap();
        let dates = visible(&parsed, 1, start, end);
        assert_eq!(dates.len(), 3);
        assert_eq!(dates[0].start_at.to_string(), "2026-10-25 22:00:00");
        assert_eq!(dates[1].start_at.to_string(), "2026-11-01 23:00:00");
        assert_eq!(dates[2].start_at.to_string(), "2026-11-08 23:00:00");
        let until = ical_import::parse(&ics.replace("COUNT=3", "UNTIL=20261101T133000Z")).unwrap();
        assert_eq!(visible(&until, 1, start, end).len(), 1);
    }
}
