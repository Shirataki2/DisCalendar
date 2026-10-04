//! Discord の通知・コマンドで共有する日時表示。DB の日時はタイムゾーンなしの JST。

use chrono::{FixedOffset, NaiveDateTime, TimeZone};

fn timestamp(datetime: NaiveDateTime, style: char) -> String {
    let unix = FixedOffset::east_opt(9 * 3600)
        .expect("valid offset")
        .from_local_datetime(&datetime)
        .single()
        .expect("JST has no ambiguous or nonexistent local times")
        .timestamp();
    format!("<t:{unix}:{style}>")
}

/// 時刻付き予定は閲覧者の日時と相対時間、終日予定は固定の日付を表示する。
pub fn format_datetime(datetime: NaiveDateTime, all_day: bool) -> String {
    if all_day {
        return datetime.format("%Y/%m/%d").to_string();
    }
    format!(
        "{} ({})",
        timestamp(datetime, 'F'),
        timestamp(datetime, 'R')
    )
}

/// 終日は日付を変換せず、時刻付き予定は閲覧者ごとの日付の違いを考慮して両端の日付を表示する。
pub fn format_date_range(all_day: bool, start: NaiveDateTime, end: NaiveDateTime) -> String {
    if all_day {
        let start_date = format_datetime(start, true);
        if start.date() == end.date() {
            return start_date;
        }
        return format!("{} - {}", start_date, format_datetime(end, true));
    }
    format!(
        "{} - {} ({})",
        timestamp(start, 'F'),
        timestamp(end, 'F'),
        timestamp(start, 'R')
    )
}

/// JST の終日日付を保ったまま表記だけを切り替える。
pub fn format_datetime_localized(
    datetime: NaiveDateTime,
    all_day: bool,
    locale: crate::i18n::Locale,
) -> String {
    if all_day {
        locale.date(datetime)
    } else {
        format_datetime(datetime, false)
    }
}
pub fn format_date_range_localized(
    all_day: bool,
    start: NaiveDateTime,
    end: NaiveDateTime,
    locale: crate::i18n::Locale,
) -> String {
    if !all_day {
        return format_date_range(false, start, end);
    }
    let start_date = locale.date(start);
    if start.date() == end.date() {
        start_date
    } else {
        format!("{start_date} - {}", locale.date(end))
    }
}
