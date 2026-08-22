//! 予定の通知タスク (旧 `tasks/notify.rs` 相当)。
//!
//! 60秒ごとに全ギルド横断で未来の予定を取得し、各予定の通知設定 (「num unit 前」、
//! 予定開始そのものを含む) が今のタイミングに重なっていれば `event_settings` の通知先チャンネルへ
//! embed を送る。判定には分単位に丸めない生の現在時刻を使い、1分の判定窓でタスクの実行間隔の
//! ジッターを吸収する (旧実装と同じ設計)。

use std::time::Duration as StdDuration;

use chrono::{Duration, NaiveDateTime};
use poise::serenity_prelude::{self as serenity, ChannelId};

use crate::{
    data::Data,
    models::{
        event_settings,
        events::{self, Event},
        notifications::{Notification, NotificationUnit},
        now_jst,
    },
};

const INTERVAL: StdDuration = StdDuration::from_secs(60);

pub async fn run_loop(ctx: serenity::Context, data: Data) {
    loop {
        run_once(&ctx, &data).await;
        tokio::time::sleep(INTERVAL).await;
    }
}

async fn run_once(ctx: &serenity::Context, data: &Data) {
    let now = now_jst();
    let events = match events::list_all_future(&data.pool, now).await {
        Ok(events) => events,
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch upcoming events");
            return;
        }
    };
    for event in &events {
        notify_for_event(ctx, data, event, now).await;
    }
}

async fn notify_for_event(ctx: &serenity::Context, data: &Data, event: &Event, now: NaiveDateTime) {
    let setting = match event_settings::get(&data.pool, &event.guild_id).await {
        Ok(Some(setting)) => setting,
        Ok(None) => return,
        Err(e) => {
            tracing::error!(error = %e, guild_id = event.guild_id, "failed to fetch notification channel");
            return;
        }
    };
    let Ok(channel_id) = setting.channel_id.parse::<u64>().map(ChannelId::new) else {
        tracing::warn!(
            channel_id = setting.channel_id,
            "invalid channel id in event_settings"
        );
        return;
    };

    let (start, end) = effective_range(event);
    // 予定開始そのものへの通知を、他の「num unit 前」の通知と同じ仕組みで扱う
    let mut notifications = event.notifications();
    notifications.push(Notification::new(0, NotificationUnit::Minutes));

    for notification in notifications {
        if !is_due(start, notification.total_minutes(), now) {
            continue;
        }
        send_notification(ctx, channel_id, event, notification, start, end).await;
    }
}

/// 終日予定は開始日・終了日それぞれ 0:00 に丸めた範囲で判定・表示する (web / api と同じ規約)
fn effective_range(event: &Event) -> (NaiveDateTime, NaiveDateTime) {
    if event.is_all_day {
        let midnight = |dt: NaiveDateTime| {
            dt.date()
                .and_hms_opt(0, 0, 0)
                .expect("midnight is always valid")
        };
        (midnight(event.start_at), midnight(event.end_at))
    } else {
        (event.start_at, event.end_at)
    }
}

/// `minutes_before` 分前がちょうど今のタスク実行タイミングに重なるか。
/// 生の現在時刻 (秒を含む) を使って `[target - 1分, target)` の窓で見ることで、
/// 60秒間隔での実行タイミングのずれを吸収する
fn is_due(start: NaiveDateTime, minutes_before: i64, now: NaiveDateTime) -> bool {
    let target = now + Duration::minutes(minutes_before);
    let window_start = target - Duration::minutes(1);
    start >= window_start && start < target
}

async fn send_notification(
    ctx: &serenity::Context,
    channel_id: ChannelId,
    event: &Event,
    notification: Notification,
    start: NaiveDateTime,
    end: NaiveDateTime,
) {
    let embed = build_embed(event, notification, start, end);
    let result = channel_id
        .send_message(&ctx.http, serenity::CreateMessage::new().embed(embed))
        .await;
    if let Err(e) = result {
        tracing::warn!(
            error = %e,
            channel_id = channel_id.get(),
            "failed to send notification embed, falling back to plain text"
        );
        let content = build_plain_text(event, notification, start, end);
        if let Err(e) = channel_id
            .send_message(&ctx.http, serenity::CreateMessage::new().content(content))
            .await
        {
            tracing::error!(error = %e, channel_id = channel_id.get(), "failed to send notification");
        }
    }
}

/// 「以下の予定が開催されます」(開始時刻通知) / 「30分後に以下の予定が開催されます」(事前通知)
fn author_text(notification: Notification) -> String {
    if notification.num == 0 {
        "以下の予定が開催されます".to_owned()
    } else {
        format!(
            "{}に以下の予定が開催されます",
            notification.to_string().replace('前', "後")
        )
    }
}

fn format_date_range(is_all_day: bool, start: NaiveDateTime, end: NaiveDateTime) -> String {
    if is_all_day {
        if start == end {
            start.format("%Y/%m/%d").to_string()
        } else {
            format!("{} - {}", start.format("%Y/%m/%d"), end.format("%Y/%m/%d"))
        }
    } else if start.date() == end.date() {
        format!(
            "{} - {}",
            start.format("%Y/%m/%d %H:%M"),
            end.format("%H:%M")
        )
    } else {
        format!(
            "{} - {}",
            start.format("%Y/%m/%d %H:%M"),
            end.format("%Y/%m/%d %H:%M")
        )
    }
}

fn build_embed(
    event: &Event,
    notification: Notification,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> serenity::CreateEmbed {
    let color = event.color.trim_start_matches('#');
    let color = u32::from_str_radix(color, 16).unwrap_or(0xff0000);
    let mut embed = serenity::CreateEmbed::new()
        .title(&event.name)
        .colour(color)
        .author(serenity::CreateEmbedAuthor::new(author_text(notification)))
        .field(
            "日時",
            format_date_range(event.is_all_day, start, end),
            false,
        );
    if let Some(description) = &event.description {
        embed = embed.description(description);
    }
    embed
}

/// embed の送信に失敗したとき用のプレーンテキスト版
fn build_plain_text(
    event: &Event,
    notification: Notification,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> String {
    use std::fmt::Write as _;

    let mut content = String::new();
    let _ = writeln!(content, ":bell: {}\n", author_text(notification));
    let _ = writeln!(content, "**{}**", event.name);
    if let Some(description) = &event.description {
        let _ = writeln!(content, "{description}\n");
    }
    let _ = writeln!(
        content,
        "**日時**\n　{}",
        format_date_range(event.is_all_day, start, end)
    );
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        s.parse().unwrap()
    }

    #[test]
    fn is_due_when_start_falls_in_the_one_minute_window_before_target() {
        let start = dt("2026-08-23T10:00:00");
        // 30分前通知: 現在時刻が 09:29:45 なら target = 09:59:45、窓は [09:58:45, 09:59:45) で start は入らない
        assert!(!is_due(start, 30, dt("2026-08-23T09:29:45")));
        // 現在時刻が 09:30:45 なら target = 10:00:45、窓は [09:59:45, 10:00:45) に start が入る
        assert!(is_due(start, 30, dt("2026-08-23T09:30:45")));
        // 次の tick (60秒後、10:31:45) では target = 10:01:45、窓は [10:00:45, 10:01:45) で start は入らない (1回きり)
        assert!(!is_due(start, 30, dt("2026-08-23T09:31:45")));
    }

    #[test]
    fn is_due_for_start_time_notification() {
        let start = dt("2026-08-23T10:00:00");
        assert!(is_due(start, 0, dt("2026-08-23T10:00:30")));
        assert!(!is_due(start, 0, dt("2026-08-23T09:59:00")));
        assert!(!is_due(start, 0, dt("2026-08-23T10:01:30")));
    }

    #[test]
    fn effective_range_rounds_all_day_events_to_midnight() {
        let event = Event {
            id: 1,
            guild_id: "1".to_owned(),
            name: "終日".to_owned(),
            description: None,
            notifications: vec![],
            color: "#0000ff".to_owned(),
            is_all_day: true,
            start_at: dt("2026-08-23T15:30:00"),
            end_at: dt("2026-08-24T09:00:00"),
            created_at: dt("2026-08-01T00:00:00"),
        };
        assert_eq!(
            effective_range(&event),
            (dt("2026-08-23T00:00:00"), dt("2026-08-24T00:00:00"))
        );
    }

    #[test]
    fn effective_range_keeps_exact_time_for_normal_events() {
        let event = Event {
            id: 1,
            guild_id: "1".to_owned(),
            name: "通常".to_owned(),
            description: None,
            notifications: vec![],
            color: "#0000ff".to_owned(),
            is_all_day: false,
            start_at: dt("2026-08-23T10:00:00"),
            end_at: dt("2026-08-23T11:00:00"),
            created_at: dt("2026-08-01T00:00:00"),
        };
        assert_eq!(
            effective_range(&event),
            (dt("2026-08-23T10:00:00"), dt("2026-08-23T11:00:00"))
        );
    }

    #[test]
    fn author_text_distinguishes_start_time_from_advance_notice() {
        assert_eq!(
            author_text(Notification::new(0, NotificationUnit::Minutes)),
            "以下の予定が開催されます"
        );
        assert_eq!(
            author_text(Notification::new(30, NotificationUnit::Minutes)),
            "30分後に以下の予定が開催されます"
        );
        assert_eq!(
            author_text(Notification::new(1, NotificationUnit::Days)),
            "1日後に以下の予定が開催されます"
        );
    }

    #[test]
    fn formats_date_range_for_all_day_single_and_multi_day() {
        assert_eq!(
            format_date_range(true, dt("2026-08-23T00:00:00"), dt("2026-08-23T00:00:00")),
            "2026/08/23"
        );
        assert_eq!(
            format_date_range(true, dt("2026-08-23T00:00:00"), dt("2026-08-25T00:00:00")),
            "2026/08/23 - 2026/08/25"
        );
        assert_eq!(
            format_date_range(false, dt("2026-08-23T10:00:00"), dt("2026-08-23T11:30:00")),
            "2026/08/23 10:00 - 11:30"
        );
        assert_eq!(
            format_date_range(false, dt("2026-08-23T22:00:00"), dt("2026-08-24T01:00:00")),
            "2026/08/23 22:00 - 2026/08/24 01:00"
        );
    }
}
