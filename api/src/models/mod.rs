//! DB アクセス層。テーブル定義は `migrations/` を参照 (旧実装・Bot と共有)。
//!
//! 日時はすべてタイムゾーンなしの JST (`TIMESTAMP`) で保存されている (旧 Web/Bot の慣習)。
//! Bot の移行が終わったら TIMESTAMPTZ への移行を検討する。

pub mod admin_audit;
pub mod admin_guilds;
pub mod admin_ops;
pub mod admin_sql;
pub mod events;
pub mod guilds;
pub mod notifications;

use chrono::{FixedOffset, NaiveDateTime, Utc};

/// 現在時刻をアプリの慣習どおり「タイムゾーンなしの JST」で返す
pub fn now_jst() -> NaiveDateTime {
    let jst = FixedOffset::east_opt(9 * 3600).expect("valid offset");
    Utc::now().with_timezone(&jst).naive_local()
}
