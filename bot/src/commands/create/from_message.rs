//! メッセージの候補をモーダルで修正し、`/quick` と同じ確認・保存経路へ渡す。

use std::{sync::LazyLock, time::Duration as Timeout};

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime};
use poise::serenity_prelude as serenity;
use regex::Regex;

use super::{DateTimeInput, ValidatedEvent, confirm_and_save, quick_input};
use crate::{
    data::{Context, Data},
    error::BotError,
    models::{events::NAME_MAX_CHARS, now_jst},
};

#[derive(Debug, poise::Modal)]
#[name = "予定にする（日本時間 / JST）"]
struct MessageEvent {
    #[name = "タイトル"]
    #[max_length = 32]
    name: String,
    #[name = "日付（YYYY-MM-DD・今日・明日）"]
    #[placeholder = "例: 2026-10-03"]
    date: String,
    #[name = "開始時刻（HH:mm）"]
    #[placeholder = "例: 21:00"]
    time: String,
    #[name = "所要時間（分・1〜10080）"]
    #[placeholder = "例: 120"]
    duration: String,
}

/// メッセージから予定の候補を読み取り、確認して作成します
#[poise::command(context_menu_command = "予定にする", guild_only, ephemeral)]
pub async fn from_message(
    ctx: poise::ApplicationContext<'_, Data, BotError>,
    message: serenity::Message,
) -> Result<(), BotError> {
    // 実行時の JST を使う。古い投稿の「明日」も実行日の翌日として表示し、必ず確認する。
    let now = now_jst();
    let locale = crate::i18n::user_locale(Context::Application(ctx));
    let defaults = LocalizedModal {
        input: MessageEvent::from_content(&message.content, now),
        locale,
    };
    // 元の interaction の有効期間 (15 分) 内に、入力と確認 (2 分) を終える。
    let Some(input) =
        poise::execute_modal(ctx, Some(defaults), Some(Timeout::from_secs(300))).await?
    else {
        return Ok(());
    };
    // resolved の Message には guild_id がない場合があるため、コマンドのサーバー ID を使う。
    let jump_url = message.id.link(message.channel_id, ctx.guild_id());
    let validated = input.input.validate_localized(&jump_url, now, locale)?;
    confirm_and_save(Context::Application(ctx), validated).await
}

impl MessageEvent {
    fn from_content(content: &str, now: NaiveDateTime) -> Self {
        static CANDIDATES: LazyLock<Regex> = LazyLock::new(|| {
            // 所要時間を時刻より先に判定し、「2時間」を「2時」として拾わない。
            Regex::new(concat!(
                r"(?i)(?P<date>[0-9]{4}-[0-9]{2}-[0-9]{2}|今日|明日|[月火水木金土日]曜日?|\b(?:today|tomorrow|monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b)",
                r"|(?P<duration>-?[0-9]+(?:\.[0-9]+)?時間(?:[0-9]+分|半)?|-?[0-9]+(?:\.[0-9]+)?分)",
                r"|(?P<english_duration>-?[0-9]+(?:\.[0-9]+)?\s*(?:hours?|minutes?)\b)",
                r"|(?P<time>[0-9]+:[0-9]+|[0-9]+時(?:[0-9]+分|半)?)",
            ))
            .expect("日時候補の正規表現")
        });
        let normalized: String = content
            .chars()
            .map(|c| match c {
                '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap(),
                '：' => ':',
                _ => c,
            })
            .collect();
        let mut dates = Vec::new();
        let mut times = Vec::new();
        let mut durations = Vec::new();
        for capture in CANDIDATES.captures_iter(&normalized) {
            if let Some(value) = capture.name("date") {
                let date = match value.as_str().to_ascii_lowercase().as_str() {
                    "今日" | "today" => Some(now.date()),
                    "明日" | "tomorrow" => now.date().succ_opt(),
                    day if day.contains('曜') => {
                        let weekday = "月火水木金土日"
                            .chars()
                            .position(|c| day.starts_with(c))
                            .unwrap();
                        let days =
                            (weekday as i64 - now.weekday().num_days_from_monday() as i64 + 7) % 7;
                        now.date().checked_add_signed(Duration::days(days))
                    }
                    day if [
                        "monday",
                        "tuesday",
                        "wednesday",
                        "thursday",
                        "friday",
                        "saturday",
                        "sunday",
                    ]
                    .contains(&day) =>
                    {
                        let weekday = [
                            "monday",
                            "tuesday",
                            "wednesday",
                            "thursday",
                            "friday",
                            "saturday",
                            "sunday",
                        ]
                        .iter()
                        .position(|value| *value == day)
                        .unwrap();
                        let days =
                            (weekday as i64 - now.weekday().num_days_from_monday() as i64 + 7) % 7;
                        now.date().checked_add_signed(Duration::days(days))
                    }
                    date => NaiveDate::parse_from_str(date, "%Y-%m-%d").ok(),
                };
                dates.push(date.filter(|date| DateTimeInput::YEARS.contains(&date.year())));
            }
            if let Some(value) = capture.name("time") {
                let value = value.as_str();
                let parts = value.split_once(':').or_else(|| value.split_once('時'));
                let time = parts.and_then(|(hour, minute)| {
                    let minute = match minute {
                        "" => 0,
                        "半" => 30,
                        minute => minute.trim_end_matches('分').parse().ok()?,
                    };
                    NaiveTime::from_hms_opt(hour.parse().ok()?, minute, 0)
                });
                times.push(time);
            }
            if let Some(value) = capture.name("english_duration") {
                let value = value.as_str().to_ascii_lowercase();
                let (number, unit) =
                    value.split_at(value.find(|ch: char| ch.is_ascii_alphabetic()).unwrap());
                let minutes = number.trim().parse::<i64>().ok().and_then(|number| {
                    number.checked_mul(if unit.starts_with("hour") { 60 } else { 1 })
                });
                durations.push(minutes.filter(|minutes| (1..=10080).contains(minutes)));
            }
            if let Some(value) = capture.name("duration") {
                let minutes = if let Some((hours, minutes)) = value.as_str().split_once("時間") {
                    hours.parse::<i64>().ok().and_then(|hours| {
                        let minutes = if minutes.is_empty() {
                            0
                        } else {
                            minutes.strip_suffix('分')?.parse().ok()?
                        };
                        hours.checked_mul(60)?.checked_add(minutes)
                    })
                } else {
                    value.as_str().trim_end_matches('分').parse().ok()
                };
                durations.push(minutes.filter(|minutes| (1..=10080).contains(minutes)));
            }
        }
        // 複数候補がある項目は決めつけず、空欄にして手入力を促す。
        Self {
            name: content
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or_default()
                .chars()
                .take(NAME_MAX_CHARS)
                .collect(),
            date: match dates.as_slice() {
                [Some(date)] => date.format("%Y-%m-%d").to_string(),
                _ => String::new(),
            },
            time: match times.as_slice() {
                [Some(time)] => time.format("%H:%M").to_string(),
                _ => String::new(),
            },
            duration: match durations.as_slice() {
                [Some(minutes)] => minutes.to_string(),
                _ => String::new(),
            },
        }
    }

    #[cfg(test)]
    fn validate(self, jump_url: &str, now: NaiveDateTime) -> Result<ValidatedEvent, BotError> {
        self.validate_localized(jump_url, now, crate::i18n::Locale::Ja)
    }

    fn validate_localized(
        self,
        jump_url: &str,
        now: NaiveDateTime,
        locale: crate::i18n::Locale,
    ) -> Result<ValidatedEvent, BotError> {
        let minutes = self.duration.trim().parse().map_err(|_| {
            crate::user_error!(
                "所要時間は 1〜10080 分（7 日）の整数で入力してください",
                "Duration must be a whole number from 1 to 10080 minutes (7 days)."
            )
        })?;
        let mut event = quick_input(
            self.name,
            self.date.trim(),
            self.time.trim(),
            Some(minutes),
            now,
        )?;
        // 本文は転載せず、リンクのみを保存するので説明の 1000 文字制限にも収まる。
        event.description = Some(crate::tr!(
            locale,
            "元メッセージ: {jump_url}",
            "Source message: {jump_url}"
        ));
        Ok(event)
    }
}

/// Modal のラベルをコマンド実行者の言語に合わせる。
struct LocalizedModal {
    input: MessageEvent,
    locale: crate::i18n::Locale,
}
impl poise::Modal for LocalizedModal {
    fn create(defaults: Option<Self>, custom_id: String) -> serenity::CreateInteractionResponse {
        let Self { input, locale } = defaults.expect("modal defaults");
        let fields = [
            ("name", "タイトル", input.name, ""),
            (
                "date",
                "日付（YYYY-MM-DD・今日・明日）",
                input.date,
                "例: 2026-10-03",
            ),
            ("time", "開始時刻（HH:mm）", input.time, "例: 21:00"),
            (
                "duration",
                "所要時間（分・1〜10080）",
                input.duration,
                "例: 120",
            ),
        ];
        let rows = fields
            .into_iter()
            .map(|(id, label, value, placeholder)| {
                let mut field = serenity::CreateInputText::new(
                    serenity::InputTextStyle::Short,
                    crate::messages::message(locale, label),
                    id,
                )
                .value(value)
                .placeholder(crate::messages::message(locale, placeholder));
                if id == "name" {
                    field = field.max_length(32);
                }
                serenity::CreateActionRow::InputText(field)
            })
            .collect();
        serenity::CreateInteractionResponse::Modal(
            serenity::CreateModal::new(
                custom_id,
                crate::messages::message(locale, "予定にする（日本時間 / JST）"),
            )
            .components(rows),
        )
    }
    fn parse(data: serenity::ModalInteractionData) -> Result<Self, &'static str> {
        Ok(Self {
            input: MessageEvent::parse(data)?,
            locale: crate::i18n::Locale::Ja,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poise::Modal;

    #[test]
    fn reads_jst_relative_dates_weekdays_and_time_formats() {
        let now = "2026-12-31T23:59:59".parse().unwrap(); // 木曜
        for (content, date, time, duration) in [
            ("今日21時から2時間集まろう", "2026-12-31", "21:00", "120"),
            ("明日 9:05 30分", "2027-01-01", "09:05", "30"),
            ("土曜 21時30分 1時間30分", "2027-01-02", "21:30", "90"),
            ("木曜日 ２１：００ ２時間", "2026-12-31", "21:00", "120"),
            ("月曜21時半 60分", "2027-01-04", "21:30", "60"),
            ("2028-02-29 0:00 1分", "2028-02-29", "00:00", "1"),
        ] {
            let input = MessageEvent::from_content(content, now);
            assert_eq!(
                (
                    input.date.as_str(),
                    input.time.as_str(),
                    input.duration.as_str()
                ),
                (date, time, duration),
                "{content}"
            );
        }
    }

    #[test]
    fn reads_english_relative_dates_weekdays_and_durations() {
        let now = "2026-12-31T23:59:00".parse().unwrap();
        for (text, date, duration) in [
            ("tomorrow 21:00 2 hours", "2027-01-01", "120"),
            ("Saturday 09:05 30 minutes", "2027-01-02", "30"),
            ("TODAY 21:00 1 hour", "2026-12-31", "60"),
        ] {
            let input = MessageEvent::from_content(text, now);
            assert_eq!(input.date, date);
            assert_eq!(input.duration, duration);
            assert!(!input.time.is_empty());
        }
        let response = LocalizedModal::create(
            Some(LocalizedModal {
                input: MessageEvent::from_content("", now),
                locale: crate::i18n::Locale::En,
            }),
            "test".into(),
        );
        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["data"]["title"], "Create event (Japan time / JST)");
        assert_eq!(
            json["data"]["components"][0]["components"][0]["label"],
            "Title"
        );
    }

    #[test]
    fn leaves_unknown_invalid_and_ambiguous_fields_empty() {
        let now = "2026-09-27T00:00:00".parse().unwrap();
        for content in [
            "",
            "集まろう",
            "2026-02-30 25:00 0分",
            "2100-01-01 21:60 10081分",
            "今日か明日 20時か21時 1時間か2時間",
        ] {
            let input = MessageEvent::from_content(content, now);
            assert_eq!(
                (input.date, input.time, input.duration),
                (String::new(), String::new(), String::new()),
                "{content}"
            );
        }
        let duration_only = MessageEvent::from_content("2時間", now);
        assert!(duration_only.time.is_empty());
        assert_eq!(duration_only.duration, "120");
        for content in ["999999999999999999999時間", "-2時間", "1.5時間", "1時間半"] {
            assert!(
                MessageEvent::from_content(content, now).duration.is_empty(),
                "{content}"
            );
        }
        assert!(
            MessageEvent::from_content("明日21時", now)
                .duration
                .is_empty()
        );
    }

    #[test]
    fn validates_manual_input_and_keeps_source_link_across_midnight() {
        let now = "2026-12-31T23:59:59".parse().unwrap();
        let jump_url =
            "https://discord.com/channels/123456789012345678/234567890123456789/345678901234567890";
        let mut input =
            MessageEvent::from_content(&format!("\n{}\n明日23:30 2時間", "あ".repeat(40)), now);
        assert_eq!(input.name.chars().count(), NAME_MAX_CHARS);
        let event = input.validate(jump_url, now).unwrap();
        assert_eq!(
            event.start,
            "2027-01-01T23:30:00".parse::<NaiveDateTime>().unwrap()
        );
        assert_eq!(
            event.end,
            "2027-01-02T01:30:00".parse::<NaiveDateTime>().unwrap()
        );
        assert_eq!(
            event.description.as_deref(),
            Some(format!("元メッセージ: {jump_url}").as_str())
        );
        assert!(
            event.description.unwrap().chars().count()
                <= crate::models::events::DESCRIPTION_MAX_CHARS
        );
        input = MessageEvent {
            name: "定例".into(),
            date: " 今日 ".into(),
            time: " 21:00 ".into(),
            duration: " 60 ".into(),
        };
        assert!(input.validate(jump_url, now).is_ok());
        for duration in ["", "0", "-1", "10081", "1.5", "999999999999999999999"] {
            let input = MessageEvent {
                name: "定例".into(),
                date: "今日".into(),
                time: "21:00".into(),
                duration: duration.into(),
            };
            assert!(matches!(
                input.validate(jump_url, now),
                Err(BotError::User(_))
            ));
        }
    }
}
