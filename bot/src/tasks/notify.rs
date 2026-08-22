//! 予定の通知タスク (旧 `tasks/notify.rs` 相当)。
//!
//! 60秒ごとに全ギルド横断で未来の予定を取得し、各予定の通知設定 (「num unit 前」、
//! 予定開始そのものを含む) の発火時刻 (`start - num unit`) が前回チェックした時刻から
//! 今回までの間に入っていれば `event_settings` の通知先チャンネルへ embed を送る。
//! 固定長の判定窓ではなく前回時刻を引き継ぐ可変長の窓を使うことで、1回の実行が
//! 60秒を超えても (Discord API が遅い、対象が多いなど) 未判定区間が生まれず取りこぼさない。

use std::{collections::HashSet, time::Duration as StdDuration};

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
    // sleep 方式だと実際の周期が「60秒 + 前回の処理時間」になるので、処理時間を含まない
    // 一定周期で tick できる interval を使う。判定窓自体は last_checked を引き継ぐので
    // tick が多少遅れても (Burst で連続 tick になっても) 取りこぼしは起きない
    let mut interval = tokio::time::interval(INTERVAL);
    let mut last_checked = now_jst();
    loop {
        interval.tick().await;
        let now = now_jst();
        run_once(&ctx, &data, last_checked, now).await;
        last_checked = now;
    }
}

async fn run_once(
    ctx: &serenity::Context,
    data: &Data,
    last_checked: NaiveDateTime,
    now: NaiveDateTime,
) {
    // 開始時刻通知 (0分前) の発火時刻は start そのものなので、start >= last_checked の予定を
    // 取得すれば、事前通知 (start は未来) と開始時刻通知 (start は前回チェック以降) の両方を拾える
    let events = match events::list_all_future(&data.pool, last_checked).await {
        Ok(events) => events,
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch upcoming events");
            return;
        }
    };
    for event in &events {
        notify_for_event(ctx, data, event, last_checked, now).await;
    }
}

async fn notify_for_event(
    ctx: &serenity::Context,
    data: &Data,
    event: &Event,
    last_checked: NaiveDateTime,
    now: NaiveDateTime,
) {
    let (start, end) = effective_range(event);
    // 予定開始そのものへの通知を、他の「num unit 前」の通知と同じ仕組みで扱う
    let mut notifications = event.notifications();
    notifications.push(Notification::new(0, NotificationUnit::Minutes));
    // web のフォームや API は同じ通知の重複を弾かないので、送信前に一度だけに絞る
    // (0分前の開始時刻通知が DB に保存されていた場合もここで一本化される)
    let notifications = dedup_notifications(notifications);

    // 先に今回送る通知を絞ってから event_settings を引く。全未来予定に対して毎 tick
    // SELECT すると、予定が増えるほど DB 負荷とタスクの所要時間が際限なく伸びてしまう
    let due: Vec<Notification> = notifications
        .into_iter()
        .filter(|notification| is_due(start, notification.total_minutes(), last_checked, now))
        .collect();
    if due.is_empty() {
        return;
    }

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

    for notification in due {
        send_notification(ctx, channel_id, event, notification, start, end).await;
    }
}

/// 同じ「num unit 前」の重複を除く (最初に現れた1件だけを残す)
fn dedup_notifications(notifications: Vec<Notification>) -> Vec<Notification> {
    let mut seen = HashSet::new();
    notifications
        .into_iter()
        .filter(|n| seen.insert(*n))
        .collect()
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

/// 通知の発火時刻 (`start` の `minutes_before` 分前) が、前回チェックした時刻から
/// 今回のチェックまでの間 (`[last_checked, now)`) に入っているか。
///
/// 固定長の窓ではなく実際に経過した時間で判定することで、1回の `run_once` が
/// (対象が多い・Discord API が遅いなどで) 60秒を超えても、その間に発火時刻を
/// 迎えた通知を取りこぼさない。
///
/// `num` は API 側で値域を検証していない `u32` なので、事前通知の分数換算値が巨大になり
/// 日時の演算がオーバーフローし得る。checked 演算にして、オーバーフロー時は
/// (どのみち計算不能な通知として) 対象外にする。ここで panic すると `run_loop` の
/// tokio タスクごと停止し、`Data::mark_tasks_started` のガードで再起動もされず
/// 全ギルドの通知が止まってしまうため、必ず素通りできない形にしておく
fn is_due(
    start: NaiveDateTime,
    minutes_before: i64,
    last_checked: NaiveDateTime,
    now: NaiveDateTime,
) -> bool {
    let Some(offset) = Duration::try_minutes(minutes_before) else {
        return false;
    };
    let Some(fire_at) = start.checked_sub_signed(offset) else {
        return false;
    };
    fire_at >= last_checked && fire_at < now
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
        // embed と違い、プレーンテキストは予定名・説明中の @everyone やロール/ユーザーメンションを
        // そのまま解釈してしまうので、明示的に許可したメンションを空にして無効化する
        if let Err(e) = channel_id
            .send_message(
                &ctx.http,
                serenity::CreateMessage::new()
                    .content(content)
                    .allowed_mentions(serenity::CreateAllowedMentions::new()),
            )
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
    fn is_due_when_fire_time_falls_within_the_checked_window() {
        let start = dt("2026-08-23T10:00:00");
        // 30分前通知の発火時刻は 09:30:00。前回チェックが 09:29:00、今回が 09:30:00 の tick では
        // まだ発火時刻に届いていない ([09:29:00, 09:30:00) は 09:30:00 を含まない)
        assert!(!is_due(
            start,
            30,
            dt("2026-08-23T09:29:00"),
            dt("2026-08-23T09:30:00")
        ));
        // 次の tick (09:30:00 → 09:31:00) の間に発火時刻が入るので通知する
        assert!(is_due(
            start,
            30,
            dt("2026-08-23T09:30:00"),
            dt("2026-08-23T09:31:00")
        ));
        // さらに次の tick では既に過ぎているので通知しない (1回きり)
        assert!(!is_due(
            start,
            30,
            dt("2026-08-23T09:31:00"),
            dt("2026-08-23T09:32:00")
        ));
    }

    #[test]
    fn is_due_for_start_time_notification() {
        let start = dt("2026-08-23T10:00:00");
        assert!(is_due(
            start,
            0,
            dt("2026-08-23T09:59:30"),
            dt("2026-08-23T10:00:30")
        ));
        assert!(!is_due(
            start,
            0,
            dt("2026-08-23T09:58:30"),
            dt("2026-08-23T09:59:30")
        ));
    }

    #[test]
    fn is_due_covers_gaps_caused_by_slow_processing() {
        // 60秒 tick のはずが処理に時間がかかり、前回チェックから70秒空いたケース。
        // 固定長の1分窓なら取りこぼし得るが、可変長の窓なのでその間の発火時刻を確実に拾える
        let start = dt("2026-08-23T10:00:00");
        assert!(is_due(
            start,
            0,
            dt("2026-08-23T09:59:50"),
            dt("2026-08-23T10:01:00")
        ));
    }

    #[test]
    fn is_due_does_not_panic_on_overflowing_notification_values() {
        // num: u32 は API 側で値域を検証していないので、巨大な「N週間前」が保存され得る。
        // 日時演算がオーバーフローしても panic せず、単に対象外として扱う
        let start = dt("2026-08-23T10:00:00");
        let last_checked = dt("2026-08-23T09:59:00");
        let now = dt("2026-08-23T10:00:00");
        assert!(!is_due(
            start,
            i64::from(u32::MAX) * 10_080,
            last_checked,
            now
        ));
        assert!(!is_due(start, i64::MAX, last_checked, now));
        assert!(!is_due(start, i64::MIN, last_checked, now));
    }

    #[test]
    fn dedup_notifications_keeps_only_the_first_occurrence() {
        let notifications = vec![
            Notification::new(30, NotificationUnit::Minutes),
            Notification::new(1, NotificationUnit::Days),
            Notification::new(30, NotificationUnit::Minutes),
            Notification::new(0, NotificationUnit::Minutes),
        ];
        assert_eq!(
            dedup_notifications(notifications),
            vec![
                Notification::new(30, NotificationUnit::Minutes),
                Notification::new(1, NotificationUnit::Days),
                Notification::new(0, NotificationUnit::Minutes),
            ]
        );
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
