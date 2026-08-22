//! 予定の通知タスク (旧 `tasks/notify.rs` 相当)。
//!
//! 60秒ごとに全ギルド横断で未来の予定を取得し、各予定の通知設定 (「num unit 前」、
//! 予定開始そのものを含む) の発火時刻 (`start - num unit`) が前回チェックした時刻から
//! 今回までの間に入っていれば `event_settings` の通知先チャンネルへ embed を送る。
//! 固定長の判定窓ではなく前回時刻を引き継ぐ可変長の窓を使うことで、1回の実行が
//! 60秒を超えても (Discord API が遅い、対象が多いなど) 未判定区間が生まれず取りこぼさない。

use std::{
    collections::{HashMap, HashSet},
    time::Duration as StdDuration,
};

use chrono::{Duration, NaiveDateTime};
use poise::serenity_prelude::{self as serenity, ChannelId, HttpError};

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
/// `is_permanent_discord_error` の許可リストに無い未知の恒久エラーに対する安全網。
/// この回数 (= 分) 送信を試みても成功しなければ諦めて処理済み扱いにし、
/// last_checked が凍結され続けて全ギルドの通知処理が長期停滞するのを防ぐ
const MAX_SEND_ATTEMPTS: u32 = 5;

pub async fn run_loop(ctx: serenity::Context, data: Data) {
    // sleep 方式だと実際の周期が「60秒 + 前回の処理時間」になるので、処理時間を含まない
    // 一定周期で tick できる interval を使う。判定窓自体は last_checked を引き継ぐので
    // tick が多少遅れても (Burst で連続 tick になっても) 取りこぼしは起きない
    let mut interval = tokio::time::interval(INTERVAL);
    let mut last_checked = now_jst();
    // event_settings の取得などが一時的に失敗して last_checked を進められなかった tick の
    // 再試行時に、同じ判定窓で既に送信済みの (event_id, 発火時刻) を重複送信しないための記録。
    // キーに発火分数ではなく実際の発火時刻を使うのは、待機中にユーザーが予定の開始時刻を
    // 変更すると同じ (event_id, 分数) でも発火時刻が変わり、変更後の新しい発火時刻への通知まで
    // 「送信済み」と誤判定してしまうため (last_checked が確定した頃には既に判定窓の外になり、
    // その通知が永久に送られなくなる)。last_checked が確定したら次の窓では意味を持たないのでクリアする
    let mut sent_in_window: HashSet<(i32, NaiveDateTime)> = HashSet::new();
    // is_permanent_discord_error の許可リストに無い未知の恒久エラーで送信が失敗し続けたときの
    // 試行回数。MAX_SEND_ATTEMPTS に達したら諦めて処理済み扱いにする (sent_in_window と同様、
    // last_checked が確定したら次の窓では意味を持たないのでクリアする)
    let mut failure_counts: HashMap<(i32, NaiveDateTime), u32> = HashMap::new();
    loop {
        interval.tick().await;
        let now = now_jst();
        // DB 取得に失敗した tick で last_checked を進めてしまうと、その窓で発火するはずだった
        // 通知が二度と `is_due` の判定範囲に入らず永久に失われる。成功した時だけ確定させ、
        // 失敗した窓は次の tick でも last_checked はそのままにして再試行する
        if run_once(
            &ctx,
            &data,
            last_checked,
            now,
            &mut sent_in_window,
            &mut failure_counts,
        )
        .await
        {
            last_checked = now;
            sent_in_window.clear();
            failure_counts.clear();
        }
    }
}

/// 予定の取得と全予定への通知処理がすべて成功したら true。
/// 呼び出し側はこれを見て `last_checked` を更新するかどうか決める
async fn run_once(
    ctx: &serenity::Context,
    data: &Data,
    last_checked: NaiveDateTime,
    now: NaiveDateTime,
    sent_in_window: &mut HashSet<(i32, NaiveDateTime)>,
    failure_counts: &mut HashMap<(i32, NaiveDateTime), u32>,
) -> bool {
    // 開始時刻通知 (0分前) の発火時刻は start そのものなので、start >= last_checked の予定を
    // 取得すれば、事前通知 (start は未来) と開始時刻通知 (start は前回チェック以降) の両方を拾える
    let events = match events::list_all_future(&data.pool, last_checked).await {
        Ok(events) => events,
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch upcoming events");
            return false;
        }
    };
    let mut all_ok = true;
    for event in &events {
        if !notify_for_event(
            ctx,
            data,
            event,
            last_checked,
            now,
            sent_in_window,
            failure_counts,
        )
        .await
        {
            all_ok = false;
        }
    }
    all_ok
}

/// この予定の通知処理が (再試行可能な失敗なく) 完了したら true
async fn notify_for_event(
    ctx: &serenity::Context,
    data: &Data,
    event: &Event,
    last_checked: NaiveDateTime,
    now: NaiveDateTime,
    sent_in_window: &mut HashSet<(i32, NaiveDateTime)>,
    failure_counts: &mut HashMap<(i32, NaiveDateTime), u32>,
) -> bool {
    let (start, end) = effective_range(event);
    // 予定開始そのものへの通知を、他の「num unit 前」の通知と同じ仕組みで扱う
    let mut notifications = event.notifications();
    notifications.push(Notification::new(0, NotificationUnit::Minutes));
    // web のフォームや API は同じ通知の重複を弾かないので、送信前に一度だけに絞る
    // (0分前の開始時刻通知が DB に保存されていた場合もここで一本化される)
    let notifications = dedup_notifications(notifications);

    // 先に今回送る通知を絞ってから event_settings を引く。全未来予定に対して毎 tick
    // SELECT すると、予定が増えるほど DB 負荷とタスクの所要時間が際限なく伸びてしまう。
    // 同じ判定窓の再試行で既に送信済みの発火時刻は除く (event_settings 取得の失敗などで
    // last_checked が進まなかった場合に、他の予定の通知まで重複送信しないため)
    let due: Vec<(Notification, NaiveDateTime)> = notifications
        .into_iter()
        .filter_map(|notification| {
            let minutes = notification.total_minutes();
            let fire = fire_at(start, minutes)?;
            (is_due(start, minutes, last_checked, now)
                && !sent_in_window.contains(&(event.id, fire)))
            .then_some((notification, fire))
        })
        .collect();
    if due.is_empty() {
        return true;
    }

    let setting = match event_settings::get(&data.pool, &event.guild_id).await {
        Ok(Some(setting)) => setting,
        Ok(None) => return true,
        Err(e) => {
            tracing::error!(error = %e, guild_id = event.guild_id, "failed to fetch notification channel");
            // 一時的な DB エラーの可能性があるので、次の tick で同じ判定窓のまま再試行できるよう false を返す
            return false;
        }
    };
    // event_settings.channel_id は制約のない TEXT なので、旧データや手動修正で "0" が
    // 入っている可能性がある。ChannelId::new(0) は panic するので、u64 ではなく
    // NonZeroU64 としてパースし、0 も不正値として弾く
    let Ok(channel_id) = setting
        .channel_id
        .parse::<std::num::NonZeroU64>()
        .map(ChannelId::from)
    else {
        tracing::warn!(
            channel_id = setting.channel_id,
            "invalid channel id in event_settings"
        );
        // 値そのものが不正なので再試行しても直らない。失敗扱いにしない
        return true;
    };

    let mut all_sent = true;
    for (notification, fire) in due {
        let key = (event.id, fire);
        if send_notification(ctx, channel_id, event, notification, start, end).await {
            sent_in_window.insert(key);
            failure_counts.remove(&key);
            continue;
        }
        // Discord API やネットワークの一時障害で embed もフォールバックも失敗した場合、
        // 送信済みとして記録すると次の判定窓で二度と再試行されず失われてしまう。
        // ただし is_permanent_discord_error の許可リストに無い未知の恒久エラーだと
        // この失敗が延々と繰り返され、last_checked が凍結されたまま他の全ギルドの
        // 通知まで巻き込んで長期停滞し得るので、一定回数を超えたら諦めて処理済み扱いにする
        let attempts = failure_counts.entry(key).or_insert(0);
        *attempts += 1;
        if *attempts >= MAX_SEND_ATTEMPTS {
            tracing::error!(
                event_id = event.id,
                attempts = *attempts,
                "giving up on notification after repeated failures"
            );
            sent_in_window.insert(key);
            failure_counts.remove(&key);
        } else {
            all_sent = false;
        }
    }
    all_sent
}

/// 同じ発火時刻 (分換算値) を持つ通知の重複を除く (最初に現れた1件だけを残す)。
/// 「60分前」と「1時間前」のように `Notification` の構造体としては異なっても
/// `total_minutes()` が一致するものは同じタイミングで二重に通知してしまうため、
/// キーには構造体ではなく換算後の分数を使う
fn dedup_notifications(notifications: Vec<Notification>) -> Vec<Notification> {
    let mut seen = HashSet::new();
    notifications
        .into_iter()
        .filter(|n| seen.insert(n.total_minutes()))
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

/// `start` の `minutes_before` 分前の時刻 (通知の発火時刻)。
///
/// `num` は API 側で値域を検証していない `u32` なので、事前通知の分数換算値が巨大になり
/// 日時の演算がオーバーフローし得る。checked 演算にして、オーバーフロー時は
/// (どのみち計算不能な通知として) None を返す。ここで panic すると `run_loop` の
/// tokio タスクごと停止し、`Data::mark_tasks_started` のガードで再起動もされず
/// 全ギルドの通知が止まってしまうため、必ず素通りできない形にしておく
fn fire_at(start: NaiveDateTime, minutes_before: i64) -> Option<NaiveDateTime> {
    let offset = Duration::try_minutes(minutes_before)?;
    start.checked_sub_signed(offset)
}

/// 通知の発火時刻が、前回チェックした時刻から今回のチェックまでの間 (`[last_checked, now)`)
/// に入っているか。固定長の窓ではなく実際に経過した時間で判定することで、1回の `run_once` が
/// (対象が多い・Discord API が遅いなどで) 60秒を超えても、その間に発火時刻を迎えた通知を
/// 取りこぼさない
fn is_due(
    start: NaiveDateTime,
    minutes_before: i64,
    last_checked: NaiveDateTime,
    now: NaiveDateTime,
) -> bool {
    let Some(fire) = fire_at(start, minutes_before) else {
        return false;
    };
    fire >= last_checked && fire < now
}

/// embed かフォールバックのプレーンテキストのいずれかが実際に届いたら true。
/// チャンネル削除・権限剥奪など再試行しても直らない Discord API エラーの場合も、
/// これ以上 last_checked を止めて他ギルドの通知まで巻き込まないよう true (処理済み) を返す
async fn send_notification(
    ctx: &serenity::Context,
    channel_id: ChannelId,
    event: &Event,
    notification: Notification,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> bool {
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
            if is_permanent_discord_error(&e) {
                tracing::error!(
                    error = %e,
                    channel_id = channel_id.get(),
                    "notification channel is permanently unreachable, giving up"
                );
                return true;
            }
            tracing::error!(error = %e, channel_id = channel_id.get(), "failed to send notification");
            return false;
        }
    }
    true
}

/// Unknown Channel / Missing Access / Missing Permissions / Archived Thread は
/// 再試行しても直らないことが明確な Discord のエラーコード
/// (`/init` はスレッドも通知先チャンネルとして保存できるので、そのスレッドが後から
/// ロック・アーカイブされると Archived Thread で恒久的に送信できなくなる)。
/// HTTP ステータスコード (4xx かどうか) だけで判定すると、408 (Request Timeout) のような
/// 一時的なエラーまで巻き込んでしまうため、Discord 固有のエラーコードのホワイトリストで絞り込む。
/// これにより未知のエラーコードは安全側に倒れて一時的な障害として再試行対象のままになる
const PERMANENT_DISCORD_ERROR_CODES: [isize; 4] = [
    10003, // Unknown Channel
    50001, // Missing Access
    50013, // Missing Permissions
    50083, // Thread is archived
];

fn is_permanent_discord_error(error: &serenity::Error) -> bool {
    let serenity::Error::Http(http_error) = error else {
        return false;
    };
    let HttpError::UnsuccessfulRequest(response) = http_error else {
        return false;
    };
    is_permanent_error_code(response.error.code)
}

/// `serenity::ErrorResponse` (`#[non_exhaustive]`) を経由せずテストできるよう、
/// Discord のエラーコードだけを見る判定をここに切り出す
fn is_permanent_error_code(code: isize) -> bool {
    PERMANENT_DISCORD_ERROR_CODES.contains(&code)
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
    fn fire_at_computes_minutes_before_start() {
        let start = dt("2026-08-23T10:00:00");
        assert_eq!(fire_at(start, 30), Some(dt("2026-08-23T09:30:00")));
        assert_eq!(fire_at(start, 0), Some(start));
    }

    #[test]
    fn fire_at_returns_none_on_overflow() {
        let start = dt("2026-08-23T10:00:00");
        assert_eq!(fire_at(start, i64::MAX), None);
        assert_eq!(fire_at(start, i64::MIN), None);
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
    fn dedup_notifications_treats_equal_minute_values_as_duplicates() {
        // 「60分前」と「1時間前」、「24時間前」と「1日前」は Notification としては別要素でも
        // total_minutes() が一致するので、どちらか片方だけ残す
        let notifications = vec![
            Notification::new(60, NotificationUnit::Minutes),
            Notification::new(1, NotificationUnit::Hours),
            Notification::new(24, NotificationUnit::Hours),
            Notification::new(1, NotificationUnit::Days),
        ];
        assert_eq!(
            dedup_notifications(notifications),
            vec![
                Notification::new(60, NotificationUnit::Minutes),
                Notification::new(24, NotificationUnit::Hours),
            ]
        );
    }

    #[test]
    fn permanent_error_codes_are_a_specific_discord_code_allowlist() {
        // Unknown Channel / Missing Access / Missing Permissions / Archived Thread は
        // 再試行しても直らない (Archived Thread は /init でスレッドを通知先にしたケース)
        assert!(is_permanent_error_code(10003));
        assert!(is_permanent_error_code(50001));
        assert!(is_permanent_error_code(50013));
        assert!(is_permanent_error_code(50083));
        // 未知のエラーコード (408 相当の一時的なものを含む) は安全側に倒し、再試行対象のままにする
        assert!(!is_permanent_error_code(0));
        assert!(!is_permanent_error_code(50035)); // Invalid Form Body
        assert!(!is_permanent_error_code(20028)); // レート制限系
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
