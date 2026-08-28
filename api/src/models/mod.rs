//! DB アクセス層。テーブル定義は `migrations/` を参照 (Bot と共有)。
//!
//! 日時はすべてタイムゾーンなしの JST (`TIMESTAMP`) で保存されている。
//! 予定は「JST の壁時計時刻」であって絶対時刻ではない (終日予定の 0:00 判定も JST の日付で決まる) ため、
//! Bot 移行後のスキーマ整理 (#15) でも TIMESTAMPTZ には変えず naive のままにしている。

pub mod admin_analytics;
pub mod admin_audit;
pub mod admin_guilds;
pub mod admin_ops;
pub mod admin_sql;
pub mod admin_stats;
pub mod admin_status;
pub mod admin_users;
pub mod events;
pub mod guilds;
pub mod notifications;
pub mod user_activity;

use chrono::{FixedOffset, NaiveDateTime, Utc};

/// 現在時刻をアプリの慣習どおり「タイムゾーンなしの JST」で返す
pub fn now_jst() -> NaiveDateTime {
    let jst = FixedOffset::east_opt(9 * 3600).expect("valid offset");
    Utc::now().with_timezone(&jst).naive_local()
}

/// 部分一致検索用に `ILIKE` のパターンを作る (`%` / `_` / `\` はリテラル扱い)。
/// 管理コンソールの検索 (ギルド名・ユーザー名) で使う
pub fn like_pattern(q: &str) -> String {
    let mut escaped = String::with_capacity(q.len() + 2);
    escaped.push('%');
    for c in q.chars() {
        if matches!(c, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped.push('%');
    escaped
}

#[cfg(test)]
mod tests {
    use super::like_pattern;

    #[test]
    fn escapes_like_metacharacters() {
        assert_eq!(like_pattern("abc"), "%abc%");
        assert_eq!(like_pattern("50%_off\\"), "%50\\%\\_off\\\\%");
    }
}
