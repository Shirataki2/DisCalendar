//! 予定の通知タイミング。
//!
//! DB (`events.notifications JSONB`) には api の入出力と同じ `[{ "num": 30, "unit": "minutes" }]`
//! の形で入っている (#15 で旧 Web / 旧 Bot 由来の JSON 文字列配列から移行した)。
//! api (`api/src/models/notifications.rs`) と同じ読み書きをここでも行い、
//! `/create` で保存した予定を web が読め、web で作った予定を `/list` が読めるようにする。

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 予定開始の「num unit 前」に通知する
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Notification {
    pub num: u32,
    pub unit: NotificationUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationUnit {
    Minutes,
    Hours,
    Days,
    Weeks,
}

impl NotificationUnit {
    /// 通知の埋め込みや `/list` の表示に使う日本語の単位
    pub fn label(self) -> &'static str {
        match self {
            Self::Minutes => "分前",
            Self::Hours => "時間前",
            Self::Days => "日前",
            Self::Weeks => "週間前",
        }
    }

    /// 1 単位あたりの分数 (通知タスクが「num unit 前」を分に換算するのに使う)
    fn minutes_per_unit(self) -> i64 {
        match self {
            Self::Minutes => 1,
            Self::Hours => 60,
            Self::Days => 60 * 24,
            Self::Weeks => 60 * 24 * 7,
        }
    }
}

impl Notification {
    pub const fn new(num: u32, unit: NotificationUnit) -> Self {
        Self { num, unit }
    }

    /// 予定開始の何分前に通知するか
    pub fn total_minutes(self) -> i64 {
        i64::from(self.num) * self.unit.minutes_per_unit()
    }

    /// DB の JSONB → 構造化した一覧。解釈できない要素は無視する
    /// (api の `Notification::decode_all` と同じ扱い)
    pub fn decode_all(raw: &Value) -> Vec<Self> {
        let Some(items) = raw.as_array() else {
            return Vec::new();
        };
        items
            .iter()
            .filter_map(|item| Self::deserialize(item).ok())
            .collect()
    }

    /// 構造化した一覧 → DB の JSONB
    pub fn encode_all(list: &[Self]) -> Value {
        serde_json::to_value(list).expect("Notification is always serializable")
    }
}

/// "30分前" のような表示用文字列
impl fmt::Display for Notification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.num, self.unit.label())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn decodes_the_stored_json() {
        let raw = json!([
            { "num": 1, "unit": "days" },
            { "num": 30, "unit": "minutes" },
        ]);
        assert_eq!(
            Notification::decode_all(&raw),
            vec![
                Notification::new(1, NotificationUnit::Days),
                Notification::new(30, NotificationUnit::Minutes),
            ]
        );
    }

    #[test]
    fn skips_unparseable_entries() {
        let raw = json!([
            "garbage",
            { "num": -1, "unit": "minutes" },
            { "num": 2, "unit": "years" },
            { "num": 2, "unit": "hours" },
        ]);
        assert_eq!(
            Notification::decode_all(&raw),
            vec![Notification::new(2, NotificationUnit::Hours)]
        );
    }

    #[test]
    fn treats_non_array_values_as_empty() {
        assert!(Notification::decode_all(&json!(null)).is_empty());
        assert!(Notification::decode_all(&json!({ "num": 30, "unit": "minutes" })).is_empty());
    }

    #[test]
    fn encodes_in_the_format_the_api_reads() {
        let list = [
            Notification::new(1, NotificationUnit::Weeks),
            Notification::new(15, NotificationUnit::Minutes),
        ];
        assert_eq!(
            Notification::encode_all(&list),
            json!([
                { "num": 1, "unit": "weeks" },
                { "num": 15, "unit": "minutes" },
            ])
        );
    }

    #[test]
    fn roundtrip() {
        let list = [
            Notification::new(3, NotificationUnit::Days),
            Notification::new(0, NotificationUnit::Minutes),
        ];
        assert_eq!(
            Notification::decode_all(&Notification::encode_all(&list)),
            list
        );
    }

    #[test]
    fn displays_like_the_web() {
        assert_eq!(
            Notification::new(30, NotificationUnit::Minutes).to_string(),
            "30分前"
        );
        assert_eq!(
            Notification::new(2, NotificationUnit::Weeks).to_string(),
            "2週間前"
        );
    }

    #[test]
    fn converts_to_minutes_before_start() {
        assert_eq!(
            Notification::new(30, NotificationUnit::Minutes).total_minutes(),
            30
        );
        assert_eq!(
            Notification::new(2, NotificationUnit::Hours).total_minutes(),
            120
        );
        assert_eq!(
            Notification::new(1, NotificationUnit::Days).total_minutes(),
            1440
        );
        assert_eq!(
            Notification::new(1, NotificationUnit::Weeks).total_minutes(),
            10080
        );
    }
}
