//! 予定の通知タイミング。
//!
//! DB (`events.notifications TEXT[]`) には旧 Web が保存し旧 Bot が読む
//! `{"key":0,"num":30,"type":"分前"}` という JSON 文字列の配列で入っている。
//! API の入出力では構造化した `{ "num": 30, "unit": "minutes" }` を使い、
//! ここで相互変換する (Bot 移行後に保存形式を見直す際はこのモジュールだけ直せばよい)。

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// 予定開始の「num unit 前」に通知する
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Notification {
    /// 単位ごとの数値 (例: 30 分前なら 30)
    #[schema(example = 30)]
    pub num: u32,
    pub unit: NotificationUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotificationUnit {
    Minutes,
    Hours,
    Days,
    Weeks,
}

impl NotificationUnit {
    /// 旧 Web / Bot が使う表現
    fn legacy_label(self) -> &'static str {
        match self {
            Self::Minutes => "分前",
            Self::Hours => "時間前",
            Self::Days => "日前",
            Self::Weeks => "週間前",
        }
    }

    fn from_legacy_label(label: &str) -> Option<Self> {
        match label {
            "分前" => Some(Self::Minutes),
            "時間前" => Some(Self::Hours),
            "日前" => Some(Self::Days),
            "週間前" => Some(Self::Weeks),
            _ => None,
        }
    }

    /// 1 単位あたりの分数 (bot/src/models/notifications.rs と同じ)
    fn minutes_per_unit(self) -> i64 {
        match self {
            Self::Minutes => 1,
            Self::Hours => 60,
            Self::Days => 24 * 60,
            Self::Weeks => 7 * 24 * 60,
        }
    }
}

/// DB に保存されている形式
#[derive(Serialize, Deserialize)]
struct Legacy {
    /// 旧 Web の v-for 用キー。意味はないが Bot がデシリアライズ時に要求する
    key: i64,
    num: i64,
    #[serde(rename = "type")]
    ty: String,
}

impl Notification {
    /// 「予定の開始から何分前か」。Bot はこの分だけ手前で通知を送る
    /// (bot/src/models/notifications.rs の同名の関数と同じ値)
    pub fn total_minutes(self) -> i64 {
        i64::from(self.num) * self.unit.minutes_per_unit()
    }

    pub fn from_legacy(raw: &str) -> Option<Self> {
        let legacy: Legacy = serde_json::from_str(raw).ok()?;
        Some(Self {
            num: u32::try_from(legacy.num).ok()?,
            unit: NotificationUnit::from_legacy_label(&legacy.ty)?,
        })
    }

    pub fn to_legacy(self, key: usize) -> String {
        serde_json::to_string(&Legacy {
            key: key as i64,
            num: i64::from(self.num),
            ty: self.unit.legacy_label().to_owned(),
        })
        .expect("Legacy is always serializable")
    }

    /// DB の配列 → API 表現。解釈できない要素は無視する
    pub fn decode_all(raw: &[String]) -> Vec<Self> {
        raw.iter().filter_map(|s| Self::from_legacy(s)).collect()
    }

    /// DB の配列 → Bot が実際に発火させる「開始の何分前か」の一覧 (昇順・重複なし)。
    ///
    /// Bot (bot/src/tasks/notify.rs の `notify_for_event`) の扱いに合わせる:
    ///
    /// - 解釈できない要素は捨てる ([`Notification::decode_all`])
    /// - 保存済みの設定に関係なく**必ず開始時刻 (0 分前) の通知を送る**
    /// - `total_minutes()` が同じもの (「60 分前」と「1 時間前」、保存済みの「0 分前」と
    ///   開始時刻通知など) は 1 回にまとめる (`dedup_notifications`)
    ///
    /// 通知の数を数える側 (`admin_stats` の今日の通知予定数、`admin_analytics` の内訳) が
    /// それぞれ同じ規則を書き写すとずれるので、ここに一本化する
    pub fn fire_minutes(raw: &[String]) -> Vec<i64> {
        let mut minutes: Vec<i64> = Self::decode_all(raw)
            .into_iter()
            .map(Self::total_minutes)
            // Bot は保存済みの設定に関係なく開始時刻の通知を送る
            .chain(std::iter::once(0))
            .collect();
        minutes.sort_unstable();
        minutes.dedup();
        minutes
    }

    /// API 表現 → DB の配列
    pub fn encode_all(list: &[Self]) -> Vec<String> {
        list.iter()
            .enumerate()
            .map(|(i, n)| n.to_legacy(i))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_legacy_format_saved_by_old_web() {
        let raw = vec![
            r#"{"key":0,"num":1,"type":"日前"}"#.to_owned(),
            r#"{"key":1,"num":30,"type":"分前"}"#.to_owned(),
        ];
        assert_eq!(
            Notification::decode_all(&raw),
            vec![
                Notification {
                    num: 1,
                    unit: NotificationUnit::Days
                },
                Notification {
                    num: 30,
                    unit: NotificationUnit::Minutes
                },
            ]
        );
    }

    #[test]
    fn skips_unparseable_entries() {
        let raw = vec![
            "garbage".to_owned(),
            r#"{"key":0,"num":-1,"type":"分前"}"#.to_owned(),
            r#"{"key":0,"num":2,"type":"年前"}"#.to_owned(),
            r#"{"key":0,"num":2,"type":"時間前"}"#.to_owned(),
        ];
        assert_eq!(
            Notification::decode_all(&raw),
            vec![Notification {
                num: 2,
                unit: NotificationUnit::Hours
            }]
        );
    }

    #[test]
    fn encodes_in_format_the_old_bot_reads() {
        let list = [
            Notification {
                num: 1,
                unit: NotificationUnit::Weeks,
            },
            Notification {
                num: 15,
                unit: NotificationUnit::Minutes,
            },
        ];
        assert_eq!(
            Notification::encode_all(&list),
            vec![
                r#"{"key":0,"num":1,"type":"週間前"}"#,
                r#"{"key":1,"num":15,"type":"分前"}"#,
            ]
        );
    }

    #[test]
    fn roundtrip() {
        let list = [
            Notification {
                num: 3,
                unit: NotificationUnit::Days,
            },
            Notification {
                num: 0,
                unit: NotificationUnit::Minutes,
            },
        ];
        assert_eq!(
            Notification::decode_all(&Notification::encode_all(&list)),
            list
        );
    }

    #[test]
    fn fire_minutes_follows_the_bot_rules() {
        let raw = vec![
            r#"{"key":0,"num":60,"type":"分前"}"#.to_owned(),
            // 「60 分前」と発火時刻が同じ (Bot は 1 回にまとめる)
            r#"{"key":1,"num":1,"type":"時間前"}"#.to_owned(),
            // 保存済みの 0 分前は、必ず送られる開始時刻通知と同じ
            r#"{"key":2,"num":0,"type":"分前"}"#.to_owned(),
            r#"{"key":3,"num":1,"type":"日前"}"#.to_owned(),
            "garbage".to_owned(),
        ];
        assert_eq!(Notification::fire_minutes(&raw), vec![0, 60, 1440]);
    }

    #[test]
    fn fire_minutes_always_contains_the_start_notification() {
        assert_eq!(Notification::fire_minutes(&[]), vec![0]);
        assert_eq!(Notification::fire_minutes(&["garbage".to_owned()]), vec![0]);
    }

    #[test]
    fn converts_to_minutes_before_start() {
        let minutes = |num, unit| Notification { num, unit }.total_minutes();
        assert_eq!(minutes(30, NotificationUnit::Minutes), 30);
        assert_eq!(minutes(2, NotificationUnit::Hours), 120);
        assert_eq!(minutes(1, NotificationUnit::Days), 1440);
        assert_eq!(minutes(1, NotificationUnit::Weeks), 10080);
        assert_eq!(minutes(0, NotificationUnit::Minutes), 0);
    }

    #[test]
    fn json_representation_uses_snake_case_units() {
        let json = serde_json::to_string(&Notification {
            num: 2,
            unit: NotificationUnit::Hours,
        })
        .unwrap();
        assert_eq!(json, r#"{"num":2,"unit":"hours"}"#);
    }
}
