//! Discord の通知・コマンドで共有する日時表示。DB の日時はタイムゾーンなしの JST。

use chrono::{NaiveDateTime, TimeZone};

use crate::models::jst;

fn timestamp(datetime: NaiveDateTime, style: char) -> String {
    let unix = jst()
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

/// 終日は日付を変換せず、時刻付き予定の終了は JST で同日なら時刻だけにする。
pub fn format_date_range(all_day: bool, start: NaiveDateTime, end: NaiveDateTime) -> String {
    if all_day {
        let start_date = format_datetime(start, true);
        if start.date() == end.date() {
            return start_date;
        }
        return format!("{} - {}", start_date, format_datetime(end, true));
    }
    let end_style = if start.date() == end.date() { 't' } else { 'F' };
    format!(
        "{} - {} ({})",
        timestamp(start, 'F'),
        timestamp(end, end_style),
        timestamp(start, 'R')
    )
}
