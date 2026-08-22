//! 予定の通知タイミング。
//!
//! DB (`events.notifications TEXT[]`) には旧 Web が保存し旧 Bot が読む
//! `{"key":0,"num":30,"type":"分前"}` という JSON 文字列の配列で入っている。
//! api (`api/src/models/notifications.rs`) と同じ変換をここでも行い、
//! `/create` で保存した予定を web が読め、web で作った予定を `/list` が読めるようにする。

use std::fmt;

use serde::{Deserialize, Serialize};

/// 予定開始の「num unit 前」に通知する
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Notification {
    pub num: u32,
    pub unit: NotificationUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotificationUnit {
    Minutes,
    Hours,
    Days,
    Weeks,
}

impl NotificationUnit {
    /// 旧 Web / Bot が使う表現。表示にもそのまま使う
    pub fn label(self) -> &'static str {
        match self {
            Self::Minutes => "分前",
            Self::Hours => "時間前",
            Self::Days => "日前",
            Self::Weeks => "週間前",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "分前" => Some(Self::Minutes),
            "時間前" => Some(Self::Hours),
            "日前" => Some(Self::Days),
            "週間前" => Some(Self::Weeks),
            _ => None,
        }
    }
}

/// DB に保存されている形式
#[derive(Serialize, Deserialize)]
struct Legacy {
    /// 旧 Web の v-for 用キー。意味はないが旧 Bot がデシリアライズ時に要求する
    key: i64,
    num: i64,
    #[serde(rename = "type")]
    ty: String,
}

impl Notification {
    pub const fn new(num: u32, unit: NotificationUnit) -> Self {
        Self { num, unit }
    }

    pub fn from_legacy(raw: &str) -> Option<Self> {
        let legacy: Legacy = serde_json::from_str(raw).ok()?;
        Some(Self {
            num: u32::try_from(legacy.num).ok()?,
            unit: NotificationUnit::from_label(&legacy.ty)?,
        })
    }

    pub fn to_legacy(self, key: usize) -> String {
        serde_json::to_string(&Legacy {
            key: key as i64,
            num: i64::from(self.num),
            ty: self.unit.label().to_owned(),
        })
        .expect("Legacy is always serializable")
    }

    /// DB の配列 → 構造化した一覧。解釈できない要素は無視する
    pub fn decode_all(raw: &[String]) -> Vec<Self> {
        raw.iter().filter_map(|s| Self::from_legacy(s)).collect()
    }

    /// 構造化した一覧 → DB の配列
    pub fn encode_all(list: &[Self]) -> Vec<String> {
        list.iter()
            .enumerate()
            .map(|(i, n)| n.to_legacy(i))
            .collect()
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
    use super::*;

    #[test]
    fn decodes_legacy_format_saved_by_web() {
        let raw = vec![
            r#"{"key":0,"num":1,"type":"日前"}"#.to_owned(),
            r#"{"key":1,"num":30,"type":"分前"}"#.to_owned(),
        ];
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
        let raw = vec![
            "garbage".to_owned(),
            r#"{"key":0,"num":-1,"type":"分前"}"#.to_owned(),
            r#"{"key":0,"num":2,"type":"年前"}"#.to_owned(),
            r#"{"key":0,"num":2,"type":"時間前"}"#.to_owned(),
        ];
        assert_eq!(
            Notification::decode_all(&raw),
            vec![Notification::new(2, NotificationUnit::Hours)]
        );
    }

    #[test]
    fn encodes_in_format_the_old_bot_and_api_read() {
        let list = [
            Notification::new(1, NotificationUnit::Weeks),
            Notification::new(15, NotificationUnit::Minutes),
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
}
