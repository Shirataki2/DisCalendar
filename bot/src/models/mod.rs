//! DB アクセス層。テーブル定義は `api/migrations/` を参照 (api と共有)。
//!
//! 日時はすべてタイムゾーンなしの JST (`TIMESTAMP`) で保存されている (旧 Web / 旧 Bot の慣習で、api も同じ)。
//! 通知設定 (`events.notifications`) も旧形式の JSON 文字列のまま保存する (`notifications` モジュール)。
//! 保存形式の見直しは Bot 移行後 (#15)。

pub mod event_settings;
pub mod events;
pub mod guild_config;
pub mod guilds;
pub mod notifications;

use chrono::{FixedOffset, NaiveDateTime, Utc};

/// JST (UTC+9)
pub fn jst() -> FixedOffset {
    FixedOffset::east_opt(9 * 3600).expect("valid offset")
}

/// 現在時刻をアプリの慣習どおり「タイムゾーンなしの JST」で返す (api の `now_jst` と同じ)
pub fn now_jst() -> NaiveDateTime {
    Utc::now().with_timezone(&jst()).naive_local()
}
