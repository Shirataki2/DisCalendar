//! JST の今日 / 投稿曜日から 7 日間の予定を通知先へ投稿する。
use std::sync::Arc;

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime};
use poise::serenity_prelude::{self as serenity, CreateEmbed, CreateEmbedFooter};
use sqlx::PgPool;

use crate::{
    commands::list::describe,
    data::Data,
    error::BotError,
    models::{event_settings, events, events::Event, now_jst},
};

#[derive(Clone, Copy, Debug)]
enum Kind {
    Daily,
    Weekly,
}

#[derive(Debug)]
struct Settings {
    daily_enabled: bool,
    daily_time: NaiveTime,
    weekly_enabled: bool,
    weekly_day: i16,
    weekly_time: NaiveTime,
    skip_empty: bool,
    last_daily_date: Option<NaiveDate>,
    last_weekly_date: Option<NaiveDate>,
}

impl Settings {
    fn due(&self, kind: Kind, now: NaiveDateTime) -> bool {
        let (enabled, time, last) = match kind {
            Kind::Daily => (self.daily_enabled, self.daily_time, self.last_daily_date),
            Kind::Weekly => (
                self.weekly_enabled
                    && i16::try_from(now.weekday().num_days_from_monday()).unwrap()
                        == self.weekly_day,
                self.weekly_time,
                self.last_weekly_date,
            ),
        };
        enabled && now.time() >= time && last.is_none_or(|date| date < now.date())
    }
}

pub async fn run_loop(ctx: serenity::Context, data: Data) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        if let Err(error) = tick(&ctx.http, &data).await {
            tracing::error!(%error, "failed to load digest settings");
        }
    }
}

async fn tick(http: &Arc<serenity::Http>, data: &Data) -> Result<(), BotError> {
    let now = now_jst();
    let guilds = sqlx::query!(r#"
        SELECT guild_id FROM guild_digest_settings
        WHERE (daily_enabled AND daily_time <= $1 AND (last_daily_date IS NULL OR last_daily_date < $2))
           OR (weekly_enabled AND weekly_day = $3 AND weekly_time <= $1
               AND (last_weekly_date IS NULL OR last_weekly_date < $2))
        ORDER BY guild_id
    "#, now.time(), now.date(), now.weekday().num_days_from_monday() as i16)
    .fetch_all(&data.pool)
    .await?;
    let mut tasks = tokio::task::JoinSet::new();
    for guild in guilds {
        // 遅いギルドが他のギルドの当日分を止めないよう、最大4ギルドを並行処理する。
        if tasks.len() >= 4
            && let Some(Err(error)) = tasks.join_next().await
        {
            tracing::error!(%error, "digest task failed");
        }
        let http = http.clone();
        let data = data.clone();
        tasks.spawn(async move {
            for kind in [Kind::Daily, Kind::Weekly] {
                // 1 ギルドの通信障害・レート制限が後続を無期限に止めない。
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    post(&http, &data.pool, &data.site_base_url, &guild.guild_id, kind),
                )
                .await;
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        tracing::warn!(%error, guild_id = %guild.guild_id, ?kind, "digest failed; retry next tick")
                    }
                    Err(_) => {
                        tracing::warn!(guild_id = %guild.guild_id, ?kind, "digest timed out; retry next tick")
                    }
                }
            }
        });
    }
    while let Some(result) = tasks.join_next().await {
        if let Err(error) = result {
            tracing::error!(%error, "digest task failed");
        }
    }
    Ok(())
}

async fn post(
    http: &serenity::Http,
    pool: &PgPool,
    site: &str,
    guild: &str,
    kind: Kind,
) -> Result<(), BotError> {
    let now = now_jst();
    let Some(channel) = event_settings::get(pool, guild).await? else {
        return Ok(());
    };
    let start = now.date().and_time(NaiveTime::MIN);
    let end = start + Duration::days(if matches!(kind, Kind::Daily) { 1 } else { 7 });
    // 展開は自身でトランザクションを開くため、投稿用の接続を確保する前に終える。
    // プール上限が 1 でも接続の取り合いで停止しない。
    crate::recurring_events::ensure_range(pool, guild, start, end).await?;
    let events = events::list_period(pool, guild, start, end).await?;
    let mut tx = pool.begin().await?;
    // 送信から履歴の確定まで排他する。別プロセスは待たずにこのギルドを飛ばす。
    let Some(settings) = sqlx::query_as!(
        Settings,
        r#"
        SELECT daily_enabled, daily_time, weekly_enabled, weekly_day, weekly_time,
               skip_empty, last_daily_date, last_weekly_date
        FROM guild_digest_settings WHERE guild_id = $1 FOR UPDATE SKIP LOCKED
    "#,
        guild
    )
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(());
    };
    // 準備中の設定変更・別プロセスの投稿・JST の日付またぎを確認する。
    let current = now_jst();
    if current.date() != now.date() || !settings.due(kind, current) {
        return Ok(());
    }
    if !settings.skip_empty || !events.is_empty() {
        if let (Ok(channel_id), Ok(guild_id)) = (
            channel.channel_id.parse::<serenity::ChannelId>(),
            guild.parse::<u64>(),
        ) {
            // Discord の nonce は直近数分のみ有効。送信直後の DB 障害や短い再起動時の重複を抑止する。
            // 送信成功と DB commit は分散トランザクションではないため、長い障害時の exactly-once は保証しない。
            let nonce = format!(
                "{:x}-{:x}-{}",
                guild_id,
                now.date().num_days_from_ce(),
                if matches!(kind, Kind::Daily) {
                    "d"
                } else {
                    "w"
                }
            );
            let message = serenity::CreateMessage::new()
                .embed(build_embed(
                    kind,
                    start.date(),
                    &events,
                    &format!("{}/dashboard/{guild}", site.trim_end_matches('/')),
                ))
                .allowed_mentions(serenity::CreateAllowedMentions::new())
                .nonce(serenity::Nonce::String(nonce))
                .enforce_nonce(true);
            if let Err(error) = channel_id.send_message(http, message).await {
                if !super::notify::is_permanent_discord_error(&error) {
                    return Err(error.into());
                }
                tracing::warn!(%error, guild_id = guild, ?kind, "digest channel permanently unreachable; marking processed");
            }
        } else {
            tracing::warn!(
                guild_id = guild,
                ?kind,
                "invalid digest destination; marking processed"
            );
        }
    }
    sqlx::query!(
        r#"
        UPDATE guild_digest_settings SET
            last_daily_date = CASE WHEN $2 THEN $3 ELSE last_daily_date END,
            last_weekly_date = CASE WHEN NOT $2 THEN $3 ELSE last_weekly_date END
        WHERE guild_id = $1
    "#,
        guild,
        matches!(kind, Kind::Daily),
        now.date()
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Discord の制限を UTF-16 単位で保守的に数え、長い旧データでも投稿を止めない。
fn truncate(value: &str, limit: usize) -> String {
    let mut length = 0;
    value
        .chars()
        .take_while(|c| {
            length += c.len_utf16();
            length <= limit
        })
        .collect()
}

fn build_embed(kind: Kind, date: NaiveDate, events: &[Event], calendar: &str) -> CreateEmbed {
    let title = match kind {
        Kind::Daily => format!("今日の予定 ({}月{}日)", date.month(), date.day()),
        Kind::Weekly => {
            let last = date + Duration::days(6);
            format!(
                "今週の予定 ({}/{}〜{}/{})",
                date.month(),
                date.day(),
                last.month(),
                last.day()
            )
        }
    };
    let description = if events.is_empty() {
        format!(
            "{}の予定はありません\n[カレンダーを開く]({calendar})",
            if matches!(kind, Kind::Daily) {
                "今日"
            } else {
                "今週"
            }
        )
    } else {
        format!("[カレンダーを開く]({calendar})")
    };
    let mut length = title.encode_utf16().count() + description.encode_utf16().count() + 64;
    let mut embed = CreateEmbed::new()
        .title(title)
        .description(description)
        .colour(0x0000ff);
    let mut shown = 0;
    for event in events.iter().take(25) {
        let name = truncate(&event.name, 256);
        let name = if name.is_empty() {
            "（名称なし）".into()
        } else {
            name
        };
        let value = truncate(&describe(event), 1024);
        length += name.encode_utf16().count() + value.encode_utf16().count();
        if length > 6000 {
            break;
        }
        embed = embed.field(name, value, false);
        shown += 1;
    }
    if shown < events.len() {
        embed = embed.footer(CreateEmbedFooter::new(format!(
            "ほか {} 件・カレンダーで全件を確認できます",
            events.len() - shown
        )));
    }
    embed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "../api/migrations")]
    async fn locks_skip_concurrent_ticks_and_empty_periods_are_recorded(pool: PgPool) {
        let now = now_jst();
        sqlx::query("INSERT INTO guild_digest_settings (guild_id, daily_enabled, daily_time, weekly_enabled, weekly_day, weekly_time) VALUES ('1', true, '00:00', true, $1, '00:00')")
            .bind(now.weekday().num_days_from_monday() as i16).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO event_settings (guild_id, channel_id) VALUES ('1', '2')")
            .execute(&pool)
            .await
            .unwrap();
        // 空の期間は送信しない。誤って送信すると接続失敗でテストが落ちる。
        let http = serenity::HttpBuilder::new("test")
            .proxy("http://127.0.0.1:1")
            .ratelimiter_disabled(true)
            .build();
        let mut lock = pool.begin().await.unwrap();
        sqlx::query("SELECT guild_id FROM guild_digest_settings WHERE guild_id='1' FOR UPDATE")
            .execute(&mut *lock)
            .await
            .unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            post(&http, &pool, "https://example.com", "1", Kind::Daily),
        )
        .await
        .unwrap()
        .unwrap();
        lock.rollback().await.unwrap();
        let single = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with((*pool.connect_options()).clone())
            .await
            .unwrap();
        for kind in [Kind::Daily, Kind::Daily, Kind::Weekly, Kind::Weekly] {
            tokio::time::timeout(
                std::time::Duration::from_secs(2),
                post(&http, &single, "https://example.com", "1", kind),
            )
            .await
            .unwrap()
            .unwrap();
        }
        single.close().await;
        let dates: (NaiveDate, NaiveDate) = sqlx::query_as("SELECT last_daily_date, last_weekly_date FROM guild_digest_settings WHERE guild_id='1'").fetch_one(&pool).await.unwrap();
        assert_eq!(dates, (now.date(), now.date()));
    }

    #[sqlx::test(migrations = "../api/migrations")]
    async fn invalid_destination_is_processed_without_retry(pool: PgPool) {
        sqlx::query("INSERT INTO guild_digest_settings (guild_id, daily_enabled, daily_time, skip_empty) VALUES ('1', true, '00:00', false)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO event_settings (guild_id, channel_id) VALUES ('1', 'invalid')")
            .execute(&pool)
            .await
            .unwrap();
        let http = serenity::HttpBuilder::new("test")
            .proxy("http://127.0.0.1:1")
            .ratelimiter_disabled(true)
            .build();
        post(&http, &pool, "https://example.com", "1", Kind::Daily)
            .await
            .unwrap();
        let date: NaiveDate = sqlx::query_scalar(
            "SELECT last_daily_date FROM guild_digest_settings WHERE guild_id='1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(date, now_jst().date());
        post(&http, &pool, "https://example.com", "1", Kind::Daily)
            .await
            .unwrap();
    }

    #[sqlx::test(migrations = "../api/migrations")]
    async fn slow_discord_request_does_not_block_other_guilds(pool: PgPool) {
        sqlx::query("INSERT INTO guild_digest_settings (guild_id, daily_enabled, daily_time, skip_empty) VALUES ('1', true, '00:00', false), ('2', true, '00:00', true)")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO event_settings (guild_id, channel_id) VALUES ('1', '11'), ('2', '22')",
        )
        .execute(&pool)
        .await
        .unwrap();
        // 先頭ギルドへの HTTP 応答を保留し、後続の空期間が先に処理済みになることを確認する。
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let http = Arc::new(
            serenity::HttpBuilder::new("test")
                .proxy(format!("http://{}", listener.local_addr().unwrap()))
                .ratelimiter_disabled(true)
                .build(),
        );
        let data = Data {
            pool: pool.clone(),
            site_base_url: "https://example.com".into(),
            log_channel_id: None,
            invite_url: String::new(),
            support_guild_id: None,
            guild_sync: Arc::new(tokio::sync::Mutex::new(())),
            tasks_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            presence_tasks: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        };
        let task = tokio::spawn(async move { tick(&http, &data).await });
        let _connection =
            tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
                .await
                .unwrap()
                .unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let done: bool = sqlx::query_scalar("SELECT last_daily_date IS NOT NULL FROM guild_digest_settings WHERE guild_id='2'").fetch_one(&pool).await.unwrap();
                if done { break }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        }).await;
        task.abort();
        let _ = task.await;
        assert!(result.is_ok(), "送信待ちのギルドが後続をブロックしている");
    }

    #[test]
    fn due_uses_jst_time_weekday_and_separate_persisted_dates() {
        let monday: NaiveDateTime = "2026-09-28T08:00:00".parse().unwrap();
        let mut settings = Settings {
            daily_enabled: true,
            daily_time: "08:00:00".parse().unwrap(),
            weekly_enabled: true,
            weekly_day: 0,
            weekly_time: "08:00:00".parse().unwrap(),
            skip_empty: true,
            last_daily_date: None,
            last_weekly_date: None,
        };
        for kind in [Kind::Daily, Kind::Weekly] {
            assert!(!settings.due(kind, monday - Duration::seconds(1)));
            assert!(settings.due(kind, monday));
            assert!(settings.due(kind, monday + Duration::hours(15)));
        }
        settings.last_daily_date = Some(monday.date());
        assert!(!settings.due(Kind::Daily, monday));
        assert!(settings.due(Kind::Weekly, monday));
        settings.last_weekly_date = Some(monday.date());
        assert!(!settings.due(Kind::Weekly, monday));
        assert!(!settings.due(Kind::Weekly, monday + Duration::days(1)));
        assert!(settings.due(Kind::Weekly, monday + Duration::days(7)));
        assert!(!settings.due(Kind::Daily, monday + Duration::hours(16)));
        assert!(settings.due(Kind::Daily, monday + Duration::days(1)));
        settings.daily_enabled = false;
        settings.weekly_enabled = false;
        assert!(!settings.due(Kind::Daily, monday + Duration::days(7)));
        assert!(!settings.due(Kind::Weekly, monday + Duration::days(7)));
    }

    #[test]
    fn embeds_include_dates_empty_message_and_fit_discord_limits() {
        let date = "2026-12-28".parse().unwrap();
        let empty = serde_json::to_value(build_embed(
            Kind::Weekly,
            date,
            &[],
            "https://discalendar.app/dashboard/1",
        ))
        .unwrap();
        assert_eq!(empty["title"], "今週の予定 (12/28〜1/3)");
        assert!(
            empty["description"]
                .as_str()
                .unwrap()
                .contains("今週の予定はありません")
        );
        let event = Event {
            id: 1,
            guild_id: "1".into(),
            name: "定例".into(),
            description: None,
            location: None,
            notifications: serde_json::json!([{ "num": 30, "unit": "minutes" }]),
            notification_mentions: serde_json::json!([]),
            color: "#0000ff".into(),
            is_all_day: false,
            start_at: "2026-12-28T10:00:00".parse().unwrap(),
            end_at: "2026-12-28T11:00:00".parse().unwrap(),
            created_at: "2026-12-01T00:00:00".parse().unwrap(),
            created_by: None,
            updated_by: None,
            updated_at: None,
        };
        for location in [None, Some("😀".repeat(1000))] {
            let mut event = event.clone();
            event.location = location;
            let value = serde_json::to_value(build_embed(
                Kind::Daily,
                date,
                &vec![event; 30],
                "https://discalendar.app/dashboard/1",
            ))
            .unwrap();
            assert_eq!(value["title"], "今日の予定 (12月28日)");
            let fields = value["fields"].as_array().unwrap();
            assert!(fields.len() <= 25);
            assert!(fields[0]["value"].as_str().unwrap().contains("<t:"));
            assert!(
                value["footer"]["text"]
                    .as_str()
                    .unwrap()
                    .contains(&format!("ほか {} 件", 30 - fields.len()))
            );
            let mut total = value["title"].as_str().unwrap().encode_utf16().count()
                + value["description"]
                    .as_str()
                    .unwrap()
                    .encode_utf16()
                    .count()
                + value["footer"]["text"]
                    .as_str()
                    .unwrap()
                    .encode_utf16()
                    .count();
            for field in fields {
                let name = field["name"].as_str().unwrap().encode_utf16().count();
                let body = field["value"].as_str().unwrap().encode_utf16().count();
                assert!(name <= 256 && body <= 1024);
                total += name + body;
            }
            assert!(total <= 6000);
        }
    }
}
