//! 外部 ICS の安全な取得と表示期間への展開。DB の予定には混ぜない。
use std::{
    sync::Arc,
    time::{Duration as StdDuration, Instant},
};

use chrono::{Duration, NaiveDateTime};
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
        let parsed = ical_import::parse_subscription(text).map_err(|_| "ICS を解析できません")?;
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
) -> Result<Vec<ExternalEvent>, &'static str> {
    let mut result = Vec::new();
    for item in &parsed.events {
        let event = &item.event;
        let source = parsed
            .sources
            .get(&item.source_index)
            .ok_or("元日時を読み取れません")?;
        let occurrences = source_occurrences(
            source,
            event.recurrence.as_ref().filter(|rule| rule.enabled()),
            &parsed.vtimezones,
            event,
            start,
            end,
        )?;
        for (occurrence, ends_at) in occurrences {
            let exclusive_end = if event.is_all_day {
                ends_at
                    .checked_add_signed(Duration::days(1))
                    .unwrap_or(NaiveDateTime::MAX)
            } else {
                ends_at
            };
            let overlaps = if occurrence == exclusive_end {
                occurrence >= start
            } else {
                exclusive_end > start
            };
            if occurrence < end && overlaps {
                if result.len() == MAX_VISIBLE {
                    return Err("表示する予定が 1000 件を超えています。表示期間を短くしてください");
                }
                result.push(ExternalEvent {
                    id: format!("{calendar_id}:{}:{occurrence}", item.source_index),
                    calendar_id,
                    name: event.name.clone(),
                    description: event.description.clone(),
                    is_all_day: event.is_all_day,
                    start_at: occurrence,
                    end_at: ends_at,
                });
            }
        }
    }
    Ok(result)
}

/// 元の壁時計で開始を計算してから JST に変換する。UTC の UNTIL は変換後に判定する。
fn source_occurrences(
    source: &ical_import::SourceRecurrence,
    rule: Option<&recurrence::Rule>,
    vtimezones: &[mailrs_ical::VTimezone],
    event: &ical_import::ImportEventInput,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> Result<Vec<(NaiveDateTime, NaiveDateTime)>, &'static str> {
    let local_start = ical_import::source_local(&source.start)?;
    let until = source
        .rule
        .as_deref()
        .and_then(|raw| {
            raw.split(';').find_map(|part| {
                part.split_once('=')
                    .filter(|(key, _)| key.eq_ignore_ascii_case("UNTIL"))
                    .map(|(_, value)| value)
            })
        })
        .map(ical_import::parse_until)
        .transpose()
        .map_err(|_| "繰り返し終了日が不正です")?;
    // 日付変更と夏時間の差を含めて候補を拾い、変換後に正確な表示範囲で絞る。
    let overlap = event.end_at - event.start_at + Duration::days(2);
    let from = start
        .checked_sub_signed(overlap)
        .unwrap_or(NaiveDateTime::MIN);
    let to = end
        .checked_add_signed(Duration::days(2))
        .unwrap_or(NaiveDateTime::MAX);
    let starts = if let Some(rule) = rule {
        let raw = rule
            .rrule(local_start)
            .map_err(|_| "繰り返し条件を読み取れません")?;
        recurrence::between(&raw, local_start, from, to)
            .map_err(|_| "繰り返しの計算上限に達しました。表示期間を短くしてください")?
    } else {
        vec![local_start]
    };
    let mut result = Vec::new();
    for local in starts {
        let delta = local - local_start;
        let occurrence = shift_source(&source.start, delta).ok_or("日時が範囲外です")?;
        // 夏時間開始時の存在しない壁時計時刻は発生しない回として扱う。
        let Ok(start_at) = ical_import::to_jst(&occurrence, vtimezones) else {
            continue;
        };
        if let Some((limit, utc)) = until {
            let compared = if utc {
                start_at
                    .checked_sub_signed(Duration::hours(9))
                    .ok_or("日時が範囲外です")?
            } else {
                local
            };
            if compared > limit {
                continue;
            }
        }
        // RFC 5545 3.8.5.3: DTEND は初回と同じ実時間、DURATION の日・週だけは暦上の長さ。
        let end_at = if !event.is_all_day && source.duration_days > 0 {
            let days = Duration::try_days(source.duration_days).ok_or("日時が範囲外です")?;
            let after_days = shift_source(&occurrence, days).ok_or("日時が範囲外です")?;
            ical_import::to_jst(&after_days, vtimezones)?
                .checked_add_signed(source.duration.unwrap_or_default() - days)
                .ok_or("日時が範囲外です")?
        } else {
            start_at
                .checked_add_signed(event.end_at - event.start_at)
                .ok_or("日時が範囲外です")?
        };
        result.push((start_at, end_at));
    }
    Ok(result)
}

fn shift_source(value: &CalDateTime, delta: Duration) -> Option<CalDateTime> {
    Some(match value {
        CalDateTime::Zoned { tz_name, local } => CalDateTime::Zoned {
            tz_name: tz_name.clone(),
            local: local.checked_add_signed(delta)?,
        },
        CalDateTime::Utc(utc) => CalDateTime::Utc(utc.checked_add_signed(delta)?),
        CalDateTime::Floating(local) => CalDateTime::Floating(local.checked_add_signed(delta)?),
        CalDateTime::Date(date) => CalDateTime::Date(date.checked_add_signed(delta)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn displays_only_overlapping_events() {
        let parsed = ical_import::parse_subscription("BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:one\nDTSTAMP:20260101T000000Z\nDTSTART:20261001T100000\nDTEND:20261001T110000\nSUMMARY:大会\nEND:VEVENT\nEND:VCALENDAR").unwrap();
        let start = "2026-10-01T10:30:00".parse().unwrap();
        let end = "2026-10-01T11:30:00".parse().unwrap();
        assert_eq!(visible(&parsed, 1, start, end).unwrap().len(), 1);
        assert!(
            visible(&parsed, 1, end, end + Duration::hours(1))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn expands_zoned_recurrence_across_dst_in_source_timezone() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:ny-weekly\nDTSTAMP:20260101T000000Z\nDTSTART;TZID=America/New_York:20261025T090000\nDTEND;TZID=America/New_York:20261025T100000\nRRULE:FREQ=WEEKLY;BYDAY=SU;COUNT=3\nSUMMARY:Morning\nEND:VEVENT\nEND:VCALENDAR";
        let parsed = ical_import::parse_subscription(ics).unwrap();
        let start = "2026-10-25T00:00:00".parse().unwrap();
        let end = "2026-11-09T00:00:00".parse().unwrap();
        let dates = visible(&parsed, 1, start, end).unwrap();
        assert_eq!(dates.len(), 3);
        assert_eq!(dates[0].start_at.to_string(), "2026-10-25 22:00:00");
        assert_eq!(dates[1].start_at.to_string(), "2026-11-01 23:00:00");
        assert_eq!(dates[2].start_at.to_string(), "2026-11-08 23:00:00");
        let until =
            ical_import::parse_subscription(&ics.replace("COUNT=3", "UNTIL=20261101T133000Z"))
                .unwrap();
        assert_eq!(visible(&until, 1, start, end).unwrap().len(), 1);
    }

    #[test]
    fn validates_weekdays_before_jst_conversion_and_keeps_exact_dtend_duration() {
        let ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:weekly\nDTSTAMP:20260101T000000Z\nDTSTART;TZID=America/Los_Angeles:20261025T230000\nDTEND;TZID=America/Los_Angeles:20261026T000000\nRRULE:FREQ=WEEKLY;BYDAY=SU;COUNT=2\nSUMMARY:Night\nEND:VEVENT\nEND:VCALENDAR";
        let parsed = ical_import::parse_subscription(ics).unwrap();
        assert!(parsed.skipped.is_empty());
        let start = "2026-10-25T00:00:00".parse().unwrap();
        let end = "2026-11-04T00:00:00".parse().unwrap();
        let dates = visible(&parsed, 1, start, end).unwrap();
        assert_eq!(dates[0].start_at.to_string(), "2026-10-26 15:00:00");
        assert_eq!(dates[1].start_at.to_string(), "2026-11-02 16:00:00");

        let ny = ics
            .replace("America/Los_Angeles", "America/New_York")
            .replace("20261025T230000", "20261025T013000")
            .replace("20261026T000000", "20261025T033000");
        let parsed = ical_import::parse_subscription(&ny).unwrap();
        let dates = visible(&parsed, 1, start, end).unwrap();
        // DTEND は初回と同じ実時間 (2 時間) を保つ。
        assert_eq!(dates[1].end_at - dates[1].start_at, Duration::hours(2));
        let nominal = ny.replace(
            "DTEND;TZID=America/New_York:20261025T033000",
            "DURATION:P1D",
        );
        let parsed = ical_import::parse_subscription(&nominal).unwrap();
        let dates = visible(&parsed, 1, start, end).unwrap();
        assert_eq!(dates[0].end_at - dates[0].start_at, Duration::hours(24));
        assert_eq!(dates[1].end_at - dates[1].start_at, Duration::hours(25));
    }

    #[test]
    fn includes_zero_length_at_range_start_and_reports_expansion_limit() {
        let event = "BEGIN:VEVENT\nUID:one\nDTSTAMP:20260101T000000Z\nDTSTART:20261001T000000\nSUMMARY:大会\nEND:VEVENT\n";
        let parsed = ical_import::parse_subscription(&format!(
            "BEGIN:VCALENDAR\nVERSION:2.0\n{event}END:VCALENDAR"
        ))
        .unwrap();
        let start = "2026-10-01T00:00:00".parse().unwrap();
        assert_eq!(
            visible(&parsed, 1, start, start + Duration::days(1))
                .unwrap()
                .len(),
            1
        );
        let repeated = event
            .replace("SUMMARY:", "RRULE:FREQ=DAILY;COUNT=400\nSUMMARY:")
            .repeat(3);
        let parsed = ical_import::parse_subscription(&format!(
            "BEGIN:VCALENDAR\nVERSION:2.0\n{repeated}END:VCALENDAR"
        ))
        .unwrap();
        assert!(
            visible(&parsed, 1, start, start + Duration::days(400))
                .unwrap_err()
                .contains("1000")
        );
    }
}
