//! `/create`: 予定の作成 (旧 `commands/create.rs`)。
//!
//! 予定の入力は web のフォームに寄せる方針なので、引数は旧版と同じ最低限
//! (名称 / 開始・終了の年月日時分 / 説明 / 終日 / 色 / 事前通知 4 つまで)。
//! 保存形式は api と同じ (タイムゾーンなしの JST、通知は旧形式の JSON、終日予定は開始日 0:00 〜 終了日 0:00) で、
//! ここで作った予定は web のカレンダーにそのまま表示される
#![allow(clippy::too_many_arguments)]

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use poise::serenity_prelude::{CreateEmbed, Timestamp};

use super::format_datetime;
use crate::{
    checks,
    data::Context,
    error::BotError,
    models::{
        events::{self, DESCRIPTION_MAX_CHARS, NAME_MAX_CHARS, NewEvent},
        guild_config,
        notifications::{Notification, NotificationUnit},
        now_jst,
    },
};

/// 予定を新たに作成します
#[poise::command(slash_command, guild_only)]
pub async fn create(
    ctx: Context<'_>,
    #[description = "予定の名称 (32 文字まで)"]
    #[max_length = 32]
    name: String,
    #[description = "開始日時 (年)"]
    #[min = 1970]
    #[max = 2099]
    start_year: i32,
    #[description = "開始日時 (月)"]
    #[min = 1]
    #[max = 12]
    start_month: u32,
    #[description = "開始日時 (日)"]
    #[min = 1]
    #[max = 31]
    start_day: u32,
    #[description = "開始日時 (時)"]
    #[min = 0]
    #[max = 23]
    start_hour: u32,
    #[description = "開始日時 (分)"]
    #[min = 0]
    #[max = 59]
    start_minute: u32,
    #[description = "終了日時 (年)"]
    #[min = 1970]
    #[max = 2099]
    end_year: i32,
    #[description = "終了日時 (月)"]
    #[min = 1]
    #[max = 12]
    end_month: u32,
    #[description = "終了日時 (日)"]
    #[min = 1]
    #[max = 31]
    end_day: u32,
    #[description = "終了日時 (時)"]
    #[min = 0]
    #[max = 23]
    end_hour: u32,
    #[description = "終了日時 (分)"]
    #[min = 0]
    #[max = 59]
    end_minute: u32,
    #[description = "予定の説明 (1000 文字まで)"]
    #[max_length = 1000]
    description: Option<String>,
    #[description = "終日の予定にする (時・分は無視されます)"] is_all_day: Option<bool>,
    #[description = "予定の色 (省略時は青)"] color: Option<Color>,
    #[description = "事前通知 (1 つ目)"] notify_1: Option<NotifyBefore>,
    #[description = "事前通知 (2 つ目)"] notify_2: Option<NotifyBefore>,
    #[description = "事前通知 (3 つ目)"] notify_3: Option<NotifyBefore>,
    #[description = "事前通知 (4 つ目)"] notify_4: Option<NotifyBefore>,
) -> Result<(), BotError> {
    let Some(guild_id) = ctx.guild_id() else {
        return Err(BotError::user("このコマンドはサーバー内でのみ実行できます"));
    };
    let guild_id = guild_id.to_string();
    let pool = &ctx.data().pool;

    // restricted モードのサーバーでは管理権限を持つユーザーだけが予定を作れる (api の `ensure_can_edit` と同じ)
    if guild_config::is_restricted(pool, &guild_id).await?
        && !checks::author_can_manage_server(ctx).await?
    {
        return Err(BotError::user(format!(
            "このサーバーでは予定の作成が制限されています。{}",
            checks::MANAGE_PERMISSIONS_REQUIRED
        )));
    }

    let input = EventInput {
        name,
        description,
        start: DateTimeInput::new(start_year, start_month, start_day, start_hour, start_minute),
        end: DateTimeInput::new(end_year, end_month, end_day, end_hour, end_minute),
        is_all_day: is_all_day.unwrap_or(false),
        color: color.unwrap_or_default(),
        notifications: [notify_1, notify_2, notify_3, notify_4]
            .into_iter()
            .flatten()
            .collect(),
    };
    let validated = input.validate()?;

    let event = events::create(
        pool,
        &NewEvent {
            guild_id: &guild_id,
            name: &validated.name,
            description: validated.description.as_deref(),
            notifications: &validated.notifications,
            color: validated.color.hex(),
            is_all_day: validated.is_all_day,
            start_at: validated.start,
            end_at: validated.end,
            created_at: now_jst(),
        },
    )
    .await?;
    tracing::info!(
        guild_id,
        event_id = event.id,
        user_id = ctx.author().id.get(),
        "event created"
    );

    let mut embed = CreateEmbed::new()
        .title(&event.name)
        .colour(validated.color.rgb())
        .timestamp(Timestamp::now())
        .fields([
            (
                "開始",
                format_datetime(event.start_at, event.is_all_day),
                true,
            ),
            (
                "終了",
                format_datetime(event.end_at, event.is_all_day),
                true,
            ),
        ]);
    if let Some(description) = &event.description {
        embed = embed.description(description);
    }
    if !validated.notifications.is_empty() {
        embed = embed.field(
            "通知",
            validated
                .notifications
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            true,
        );
    }
    ctx.send(
        poise::CreateReply::default()
            .content("予定を作成しました")
            .embed(embed),
    )
    .await?;
    Ok(())
}

/// コマンドの引数をまとめたもの。`validate` で保存できる形にする
#[derive(Debug, Clone, PartialEq, Eq)]
struct EventInput {
    name: String,
    description: Option<String>,
    start: DateTimeInput,
    end: DateTimeInput,
    is_all_day: bool,
    color: Color,
    notifications: Vec<NotifyBefore>,
}

/// 検証済みの入力
#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidatedEvent {
    name: String,
    description: Option<String>,
    start: NaiveDateTime,
    end: NaiveDateTime,
    is_all_day: bool,
    color: Color,
    notifications: Vec<Notification>,
}

impl EventInput {
    /// 上限値は api の `EventInput::validate` と同じ。Discord 側でも `max_length` / `min` / `max` で弾かれるが、
    /// 保存するのは Bot なのでここでも確認する
    fn validate(self) -> Result<ValidatedEvent, BotError> {
        let name = self.name.trim().to_owned();
        if name.is_empty() {
            return Err(BotError::user("予定の名称を入力してください"));
        }
        if name.chars().count() > NAME_MAX_CHARS {
            return Err(BotError::user(format!(
                "予定の名称は {NAME_MAX_CHARS} 文字以内で入力してください"
            )));
        }
        let description = self
            .description
            .map(|d| d.trim().to_owned())
            .filter(|d| !d.is_empty());
        if description
            .as_ref()
            .is_some_and(|d| d.chars().count() > DESCRIPTION_MAX_CHARS)
        {
            return Err(BotError::user(format!(
                "予定の説明は {DESCRIPTION_MAX_CHARS} 文字以内で入力してください"
            )));
        }
        let start = self.start.resolve(self.is_all_day)?;
        let end = self.end.resolve(self.is_all_day)?;
        // 同時刻は許可 (api と同じ)
        if end < start {
            return Err(BotError::user("終了日時が開始日時より前になっています"));
        }
        // 同じ通知を 2 回指定しても 1 回にまとめる
        let mut notifications: Vec<Notification> = Vec::new();
        for n in self
            .notifications
            .into_iter()
            .map(NotifyBefore::notification)
        {
            if !notifications.contains(&n) {
                notifications.push(n);
            }
        }
        Ok(ValidatedEvent {
            name,
            description,
            start,
            end,
            is_all_day: self.is_all_day,
            color: self.color,
            notifications,
        })
    }
}

/// 年月日時分の引数。存在しない日付 (2/30 など) は `resolve` で弾く
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DateTimeInput {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
}

impl DateTimeInput {
    /// 旧 Bot の `check_date` と同じ範囲 (Discord の `min` / `max` と二重だが、保存前の最終確認)
    const YEARS: std::ops::RangeInclusive<i32> = 1970..=2099;

    fn new(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
        }
    }

    /// 終日予定は時・分を無視して 0:00 にする (web と同じ表現: 開始日 0:00 〜 終了日 0:00)
    fn resolve(self, all_day: bool) -> Result<NaiveDateTime, BotError> {
        let date = Self::YEARS
            .contains(&self.year)
            .then(|| NaiveDate::from_ymd_opt(self.year, self.month, self.day))
            .flatten()
            .ok_or_else(|| {
                BotError::user(format!(
                    "日付が正しくありません: {}/{}/{}",
                    self.year, self.month, self.day
                ))
            })?;
        let time = if all_day {
            NaiveTime::MIN
        } else {
            NaiveTime::from_hms_opt(self.hour, self.minute, 0).ok_or_else(|| {
                BotError::user(format!(
                    "時刻が正しくありません: {}:{:02}",
                    self.hour, self.minute
                ))
            })?
        };
        Ok(date.and_time(time))
    }
}

/// 予定の色。web のカラーピッカーの swatch (`COLOR_SWATCHES`) から選ぶので、web 側でもそのまま選択できる
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, poise::ChoiceParameter)]
pub enum Color {
    #[name = "赤"]
    Red,
    /// 旧 Bot と同じく既定は青
    #[default]
    #[name = "青"]
    Blue,
    #[name = "緑"]
    Green,
    #[name = "黄"]
    Yellow,
    #[name = "紫"]
    Purple,
    #[name = "水色"]
    LightBlue,
    #[name = "橙"]
    Orange,
    #[name = "ピンク"]
    Pink,
    #[name = "灰"]
    Gray,
    #[name = "黒"]
    Black,
}

impl Color {
    /// DB に保存する `#RRGGBB`
    pub fn hex(self) -> &'static str {
        match self {
            Self::Red => "#F44336",
            Self::Blue => "#2196F3",
            Self::Green => "#4CAF50",
            Self::Yellow => "#FFEB3B",
            Self::Purple => "#9C27B0",
            Self::LightBlue => "#03A9F4",
            Self::Orange => "#FF9800",
            Self::Pink => "#E91E63",
            Self::Gray => "#9E9E9E",
            Self::Black => "#212121",
        }
    }

    /// 埋め込みの色
    pub fn rgb(self) -> u32 {
        u32::from_str_radix(&self.hex()[1..], 16).expect("hex() is always #RRGGBB")
    }
}

/// 事前通知の選択肢 (旧 Bot と同じ 13 種類)
#[derive(Debug, Clone, Copy, PartialEq, Eq, poise::ChoiceParameter)]
pub enum NotifyBefore {
    #[name = "5分前"]
    Minutes5,
    #[name = "10分前"]
    Minutes10,
    #[name = "15分前"]
    Minutes15,
    #[name = "30分前"]
    Minutes30,
    #[name = "1時間前"]
    Hours1,
    #[name = "2時間前"]
    Hours2,
    #[name = "3時間前"]
    Hours3,
    #[name = "6時間前"]
    Hours6,
    #[name = "12時間前"]
    Hours12,
    #[name = "1日前"]
    Days1,
    #[name = "2日前"]
    Days2,
    #[name = "3日前"]
    Days3,
    #[name = "7日前"]
    Days7,
}

impl NotifyBefore {
    pub fn notification(self) -> Notification {
        let (num, unit) = match self {
            Self::Minutes5 => (5, NotificationUnit::Minutes),
            Self::Minutes10 => (10, NotificationUnit::Minutes),
            Self::Minutes15 => (15, NotificationUnit::Minutes),
            Self::Minutes30 => (30, NotificationUnit::Minutes),
            Self::Hours1 => (1, NotificationUnit::Hours),
            Self::Hours2 => (2, NotificationUnit::Hours),
            Self::Hours3 => (3, NotificationUnit::Hours),
            Self::Hours6 => (6, NotificationUnit::Hours),
            Self::Hours12 => (12, NotificationUnit::Hours),
            Self::Days1 => (1, NotificationUnit::Days),
            Self::Days2 => (2, NotificationUnit::Days),
            Self::Days3 => (3, NotificationUnit::Days),
            Self::Days7 => (7, NotificationUnit::Days),
        };
        Notification::new(num, unit)
    }
}

#[cfg(test)]
mod tests {
    use poise::ChoiceParameter as _;

    use super::*;

    fn input() -> EventInput {
        EventInput {
            name: "定例".to_owned(),
            description: Some("  説明  ".to_owned()),
            start: DateTimeInput::new(2026, 8, 23, 10, 0),
            end: DateTimeInput::new(2026, 8, 23, 11, 30),
            is_all_day: false,
            color: Color::Red,
            notifications: vec![NotifyBefore::Minutes30, NotifyBefore::Days1],
        }
    }

    fn assert_user_error(result: Result<ValidatedEvent, BotError>, contains: &str) {
        match result {
            Err(BotError::User(message)) => {
                assert!(message.contains(contains), "{message}")
            }
            other => panic!("expected user error, got {other:?}"),
        }
    }

    #[test]
    fn validates_and_normalizes_input() {
        let v = input().validate().unwrap();
        assert_eq!(v.name, "定例");
        assert_eq!(v.description.as_deref(), Some("説明"));
        assert_eq!(v.start, "2026-08-23T10:00:00".parse().unwrap());
        assert_eq!(v.end, "2026-08-23T11:30:00".parse().unwrap());
        assert_eq!(
            v.notifications,
            vec![
                Notification::new(30, NotificationUnit::Minutes),
                Notification::new(1, NotificationUnit::Days),
            ]
        );
    }

    #[test]
    fn all_day_uses_midnight_and_ignores_time() {
        let mut i = input();
        i.is_all_day = true;
        i.end = DateTimeInput::new(2026, 8, 25, 0, 0);
        let v = i.validate().unwrap();
        assert_eq!(v.start, "2026-08-23T00:00:00".parse().unwrap());
        assert_eq!(v.end, "2026-08-25T00:00:00".parse().unwrap());
    }

    #[test]
    fn all_day_single_day_has_same_start_and_end() {
        let mut i = input();
        i.is_all_day = true;
        // 時刻上は終了が開始より前でも、終日なら同じ日として通る
        i.end = DateTimeInput::new(2026, 8, 23, 9, 0);
        let v = i.validate().unwrap();
        assert_eq!(v.start, v.end);
    }

    #[test]
    fn rejects_blank_name_and_too_long_fields() {
        let mut i = input();
        i.name = "   ".to_owned();
        assert_user_error(i.validate(), "名称");

        let mut i = input();
        i.name = "あ".repeat(NAME_MAX_CHARS + 1);
        assert_user_error(i.validate(), "32 文字");

        let mut i = input();
        i.name = "あ".repeat(NAME_MAX_CHARS);
        assert!(i.validate().is_ok());

        let mut i = input();
        i.description = Some("あ".repeat(DESCRIPTION_MAX_CHARS + 1));
        assert_user_error(i.validate(), "1000 文字");
    }

    #[test]
    fn empty_description_becomes_none() {
        let mut i = input();
        i.description = Some("   ".to_owned());
        assert_eq!(i.validate().unwrap().description, None);
    }

    #[test]
    fn rejects_invalid_dates_and_times() {
        let mut i = input();
        i.start = DateTimeInput::new(2026, 2, 30, 10, 0);
        assert_user_error(i.validate(), "日付が正しくありません: 2026/2/30");

        let mut i = input();
        i.start = DateTimeInput::new(2100, 1, 1, 10, 0);
        assert_user_error(i.validate(), "日付が正しくありません");

        let mut i = input();
        i.end = DateTimeInput::new(2026, 8, 23, 24, 0);
        assert_user_error(i.validate(), "時刻が正しくありません: 24:00");

        // うるう年は通る
        let mut i = input();
        i.start = DateTimeInput::new(2028, 2, 29, 0, 0);
        i.end = DateTimeInput::new(2028, 2, 29, 0, 0);
        assert!(i.validate().is_ok());
    }

    #[test]
    fn rejects_end_before_start_but_allows_same_time() {
        let mut i = input();
        i.end = DateTimeInput::new(2026, 8, 23, 9, 59);
        assert_user_error(i.validate(), "終了日時が開始日時より前");

        let mut i = input();
        i.end = i.start;
        assert!(i.validate().is_ok());
    }

    #[test]
    fn deduplicates_notifications() {
        let mut i = input();
        i.notifications = vec![
            NotifyBefore::Hours1,
            NotifyBefore::Minutes5,
            NotifyBefore::Hours1,
        ];
        assert_eq!(
            i.validate().unwrap().notifications,
            vec![
                Notification::new(1, NotificationUnit::Hours),
                Notification::new(5, NotificationUnit::Minutes),
            ]
        );
    }

    #[test]
    fn colors_are_hex_and_default_is_blue() {
        for color in Color::list() {
            let color = Color::from_name(&color.name).unwrap();
            let hex = color.hex();
            assert_eq!(hex.len(), 7, "{hex}");
            assert!(hex.starts_with('#'));
            assert!(hex[1..].chars().all(|c| c.is_ascii_hexdigit()), "{hex}");
        }
        assert_eq!(Color::default(), Color::Blue);
        assert_eq!(Color::Blue.rgb(), 0x2196F3);
    }

    #[test]
    fn notify_choice_names_match_their_notification() {
        // 選択肢の表示名 ("30分前") と保存される通知の表示 (`Notification: Display`) が一致する
        for choice in NotifyBefore::list() {
            let notify = NotifyBefore::from_name(&choice.name).unwrap();
            assert_eq!(notify.notification().to_string(), choice.name);
        }
    }
}
