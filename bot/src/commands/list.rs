use chrono::{Datelike, Duration, NaiveDateTime};
use poise::serenity_prelude::CreateEmbed;

use super::format_datetime;
use crate::{
    data::Context,
    error::BotError,
    models::{
        events::{self, Event},
        now_jst,
    },
    paginator::Paginator,
};

/// 1 ページに表示する予定の数
const PER_PAGE: usize = 4;

/// 予定の一覧を表示します
#[poise::command(slash_command, guild_only)]
pub async fn list(
    ctx: Context<'_>,
    #[description = "表示する予定の範囲 (省略時は未来)"] range: Option<EventRange>,
) -> Result<(), BotError> {
    let range = range.unwrap_or_default();
    let Some(guild_id) = ctx.guild_id() else {
        return Err(BotError::user("このコマンドはサーバー内でのみ実行できます"));
    };
    let guild_id = guild_id.to_string();
    let pool = &ctx.data().pool;

    // DB を読む前に「処理中」を返し、3 秒の初回応答期限を過ぎて interaction が失敗扱いになるのを防ぐ
    ctx.defer().await?;

    let now = now_jst();
    let events = match range {
        EventRange::Past => events::list_past(pool, &guild_id, now).await?,
        EventRange::Future => events::list_future(pool, &guild_id, now).await?,
        EventRange::Today | EventRange::Week => {
            let (start, end) = range.period(now);
            events::list_period(pool, &guild_id, start, end).await?
        }
        EventRange::Next => events::list_next(pool, &guild_id, now).await?,
        EventRange::All => events::list_all(pool, &guild_id).await?,
    };
    if events.is_empty() {
        ctx.say(range.empty_message()).await?;
        return Ok(());
    }

    let template = CreateEmbed::new().title("予定一覧").colour(0x0000ff);
    let mut paginator = Paginator::new(PER_PAGE, template);
    for event in &events {
        paginator.add(&event.name, describe(event), false);
    }
    paginator.start(ctx).await
}

/// 一覧の 1 件分の本文
fn describe(event: &Event) -> String {
    let notifications = event.notifications();
    let notifications = if notifications.is_empty() {
        "なし".to_owned()
    } else {
        notifications
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "`開始時刻`: {}\n`終了時刻`: {}\n`　通知　`: {}",
        format_datetime(event.start_at, event.is_all_day),
        format_datetime(event.end_at, event.is_all_day),
        notifications
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, poise::ChoiceParameter)]
pub enum EventRange {
    #[name = "過去"]
    Past,
    #[default]
    #[name = "未来"]
    Future,
    #[name = "全て"]
    All,
    #[name = "今日"]
    Today,
    #[name = "今週"]
    Week,
    #[name = "次の予定"]
    Next,
}

impl EventRange {
    /// JST の月曜始まり。終了境界は翌日または翌週月曜の 0 時。
    fn period(self, now: NaiveDateTime) -> (NaiveDateTime, NaiveDateTime) {
        let midnight = now.date().and_time(chrono::NaiveTime::MIN);
        if self == Self::Week {
            let start = midnight - Duration::days(now.weekday().num_days_from_monday().into());
            (start, start + Duration::days(7))
        } else {
            (midnight, midnight + Duration::days(1))
        }
    }

    fn empty_message(self) -> &'static str {
        match self {
            Self::Today => "今日の予定はありません",
            Self::Week => "今週の予定はありません",
            Self::Next => "これから開催される予定はありません",
            Self::Past => "過去の予定はありません",
            Self::Future => "これから開催される予定はありません",
            Self::All => "登録されている予定はありません",
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn periods_use_jst_midnight_and_monday_across_years() {
        for now in ["2026-12-28T00:00:00", "2027-01-03T23:59:59"] {
            let (start, end) = EventRange::Week.period(now.parse().unwrap());
            assert_eq!(start, "2026-12-28T00:00:00".parse().unwrap());
            assert_eq!(end, "2027-01-04T00:00:00".parse().unwrap());
        }
        let (start, end) = EventRange::Today.period("2026-12-31T23:59:59".parse().unwrap());
        assert_eq!(start, "2026-12-31T00:00:00".parse().unwrap());
        assert_eq!(end, "2027-01-01T00:00:00".parse().unwrap());
        for range in [EventRange::Today, EventRange::Week, EventRange::Next] {
            assert!(range.empty_message().contains("予定はありません"));
        }
    }

    fn event(notifications: serde_json::Value, all_day: bool) -> Event {
        Event {
            id: 1,
            guild_id: "1".to_owned(),
            name: "定例".to_owned(),
            description: None,
            notifications,
            color: "#2196F3".to_owned(),
            is_all_day: all_day,
            start_at: "2026-08-23T10:00:00".parse().unwrap(),
            end_at: "2026-08-24T11:30:00".parse().unwrap(),
            created_at: "2026-08-01T00:00:00".parse().unwrap(),
            created_by: None,
            updated_by: None,
            updated_at: None,
        }
    }

    #[test]
    fn describes_timed_event_with_notifications() {
        let e = event(
            json!([
                { "num": 30, "unit": "minutes" },
                { "num": 1, "unit": "days" },
            ]),
            false,
        );
        assert_eq!(
            describe(&e),
            "`開始時刻`: <t:1787446800:F> (<t:1787446800:R>)\n`終了時刻`: <t:1787538600:F> (<t:1787538600:R>)\n`　通知　`: 30分前, 1日前"
        );
    }

    #[test]
    fn describes_all_day_event_without_notifications() {
        let e = event(json!([]), true);
        assert_eq!(
            describe(&e),
            "`開始時刻`: 2026/08/23\n`終了時刻`: 2026/08/24\n`　通知　`: なし"
        );
    }
}
