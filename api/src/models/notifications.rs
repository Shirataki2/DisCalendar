//! 予定の通知タイミング。
//!
//! DB (`events.notifications JSONB`) には API の入出力と同じ `[{ "num": 30, "unit": "minutes" }]`
//! の形で入っている。旧 Web / 旧 Bot が使っていた `{"key":0,"num":30,"type":"分前"}` の
//! JSON 文字列配列からは #15 のマイグレーションで移行済み。

use chrono::{Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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

impl Notification {
    /// 「予定の開始から何分前か」。Bot はこの分だけ手前で通知を送る
    /// (bot/src/models/notifications.rs の同名の関数と同じ値)
    pub fn total_minutes(self) -> i64 {
        i64::from(self.num) * self.unit.minutes_per_unit()
    }

    /// DB の JSONB → API 表現。解釈できない要素は無視する。
    ///
    /// 配列でない値や壊れた要素が入っていても予定そのものは読めるようにしている
    /// (DB の CHECK 制約は配列であることしか保証せず、管理コンソールの定型操作や
    /// 手作業での修正で api / bot 以外が書き込む余地があるため)
    pub fn decode_all(raw: &Value) -> Vec<Self> {
        let Some(items) = raw.as_array() else {
            return Vec::new();
        };
        items
            .iter()
            .filter_map(|item| Self::deserialize(item).ok())
            .collect()
    }

    /// API 表現 → DB の JSONB
    pub fn encode_all(list: &[Self]) -> Value {
        serde_json::to_value(list).expect("Notification is always serializable")
    }

    /// DB の値 → Bot が実際に発火させる「開始の何分前か」の一覧 (昇順・重複なし)。
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
    pub fn fire_minutes(raw: &Value) -> Vec<i64> {
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

    /// 予定の開始時刻。終日予定は 0:00 に丸める (web / api / Bot 共通の規約)
    pub fn effective_start(start_at: NaiveDateTime, is_all_day: bool) -> NaiveDateTime {
        if is_all_day {
            start_at
                .date()
                .and_hms_opt(0, 0, 0)
                .expect("date always has midnight")
        } else {
            start_at
        }
    }

    /// その予定について Bot が実際に送る通知の発火時刻 ([`Notification::fire_minutes`] の順、発火が遅い順)。
    ///
    /// `num` の値域は api で検証していない (#46) ので、`total_minutes()` に直した時点や
    /// 開始からの減算で溢れることがある。Bot (`fire_at`) はそれを送れないので、ここでも除く。
    /// **開始時刻の通知は必ず計算できるため、戻り値は必ず 1 件以上**になる
    pub fn fire_times(
        start_at: NaiveDateTime,
        is_all_day: bool,
        raw: &Value,
    ) -> Vec<NaiveDateTime> {
        let start = Self::effective_start(start_at, is_all_day);
        Self::fire_minutes(raw)
            .into_iter()
            .filter_map(|minutes| {
                Duration::try_minutes(minutes).and_then(|offset| start.checked_sub_signed(offset))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
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
        let raw = json!([
            "garbage",
            { "num": -1, "unit": "minutes" },
            { "num": 2, "unit": "years" },
            { "num": 2, "unit": "hours" },
        ]);
        assert_eq!(
            Notification::decode_all(&raw),
            vec![Notification {
                num: 2,
                unit: NotificationUnit::Hours
            }]
        );
    }

    #[test]
    fn keeps_entries_that_carry_unknown_fields() {
        // 将来この JSON を拡張しても (#93 のメンション先など)、古い api が予定を読めなくならないこと
        let raw = json!([{ "num": 30, "unit": "minutes", "mention": "@everyone" }]);
        assert_eq!(
            Notification::decode_all(&raw),
            vec![Notification {
                num: 30,
                unit: NotificationUnit::Minutes
            }]
        );
    }

    #[test]
    fn treats_non_array_values_as_empty() {
        assert!(Notification::decode_all(&json!(null)).is_empty());
        assert!(Notification::decode_all(&json!({ "num": 30, "unit": "minutes" })).is_empty());
    }

    #[test]
    fn encodes_in_the_api_representation() {
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
            json!([
                { "num": 1, "unit": "weeks" },
                { "num": 15, "unit": "minutes" },
            ])
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
        let raw = json!([
            { "num": 60, "unit": "minutes" },
            // 「60 分前」と発火時刻が同じ (Bot は 1 回にまとめる)
            { "num": 1, "unit": "hours" },
            // 保存済みの 0 分前は、必ず送られる開始時刻通知と同じ
            { "num": 0, "unit": "minutes" },
            { "num": 1, "unit": "days" },
            "garbage",
        ]);
        assert_eq!(Notification::fire_minutes(&raw), vec![0, 60, 1440]);
    }

    #[test]
    fn fire_minutes_always_contains_the_start_notification() {
        assert_eq!(Notification::fire_minutes(&json!([])), vec![0]);
        assert_eq!(Notification::fire_minutes(&json!(["garbage"])), vec![0]);
    }

    #[test]
    fn fire_times_drop_notifications_that_cannot_be_computed() {
        let start: NaiveDateTime = "2026-08-25T10:00:00".parse().unwrap();
        let raw = json!([
            { "num": 30, "unit": "minutes" },
            // 分に直した時点で chrono の Duration に収まらない (Bot も送れない)
            { "num": 4294967295u32, "unit": "weeks" },
        ]);
        // 30 分前と、必ず送られる開始時刻の通知だけが残る
        assert_eq!(
            Notification::fire_times(start, false, &raw),
            vec![start, "2026-08-25T09:30:00".parse().unwrap()]
        );
    }

    #[test]
    fn fire_times_round_all_day_events_to_midnight() {
        let start: NaiveDateTime = "2026-08-25T15:30:00".parse().unwrap();
        let raw = json!([{ "num": 30, "unit": "minutes" }]);
        assert_eq!(
            Notification::fire_times(start, true, &raw),
            vec![
                "2026-08-25T00:00:00".parse().unwrap(),
                "2026-08-24T23:30:00".parse().unwrap(),
            ]
        );
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
