//! DB アクセス層。テーブル定義は `api/migrations/` を参照 (api と共有)。
//!
//! 日時はすべてタイムゾーンなしの JST (`TIMESTAMP`) で保存されている (api も同じ)。
//! 通知設定 (`events.notifications`) は api の入出力と同じ JSONB で保存する (`notifications` モジュール)。

pub mod event_settings;
pub mod event_share_links;
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
