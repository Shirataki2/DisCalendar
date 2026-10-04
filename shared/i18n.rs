//! 通知・連携投稿の言語。日時の保存と発火判定は JST のまま維持する。
use chrono::NaiveDateTime;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Locale {
    #[default]
    Ja,
    En,
}

impl Locale {
    pub fn resolve(value: &str) -> Self {
        let value = value.to_ascii_lowercase();
        if value == "en" || value.starts_with("en-") {
            Self::En
        } else {
            Self::Ja
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ja => "ja",
            Self::En => "en",
        }
    }

    /// 未定義翻訳も日本語に戻し、利用者の入力には翻訳を適用しない。
    pub fn text<'a>(self, ja: &'a str, en: &'a str) -> &'a str {
        if self == Self::En && !en.is_empty() {
            en
        } else {
            ja
        }
    }

    pub fn date(self, value: NaiveDateTime) -> String {
        value.format(self.text("%Y/%m/%d", "%b %d, %Y")).to_string()
    }

    pub fn datetime(self, value: NaiveDateTime) -> String {
        value
            .format(self.text("%Y/%m/%d %H:%M JST", "%b %d, %Y %H:%M JST"))
            .to_string()
    }

    pub fn count(self, value: impl ToString) -> String {
        let value = value.to_string();
        if self == Self::Ja {
            return value;
        }
        let mut result = String::new();
        for (i, ch) in value.chars().enumerate() {
            if i > 0 && (value.len() - i).is_multiple_of(3) {
                result.push(',');
            }
            result.push(ch);
        }
        result
    }
}

/// 両言語の書式をコンパイル時に検査し、引数（予定名など）はそのまま埋め込む。
#[macro_export]
macro_rules! tr {
    ($locale:expr, $ja:literal, $en:literal $(, $args:expr)* $(,)?) => {
        if $locale == $crate::i18n::Locale::En && !$en.is_empty() {
            format!($en $(, $args)*)
        } else {
            format!($ja $(, $args)*)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolves_regions_and_falls_back_to_japanese() {
        for value in ["en", "en-US", "EN-gb"] {
            assert_eq!(Locale::resolve(value), Locale::En);
        }
        for value in ["ja", "fr", "", "english", "en_US"] {
            assert_eq!(Locale::resolve(value), Locale::Ja);
        }
        assert_eq!(Locale::En.text("日本語", ""), "日本語");
        assert_eq!(Locale::En.count(12345), "12,345");
        assert_eq!(Locale::Ja.count(12345), "12345");
        let date = "2026-12-31T23:59:00".parse().unwrap();
        assert_eq!(Locale::Ja.date(date), "2026/12/31");
        assert_eq!(Locale::En.date(date), "Dec 31, 2026");
    }
}
