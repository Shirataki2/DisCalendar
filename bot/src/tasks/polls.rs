//! 日程調整の投票メッセージを投稿・更新する。Web回答・候補編集・締切も反映する。
use crate::{data::Data, error::BotError, models::now_jst, polls::custom_id};
use chrono::NaiveDateTime;
use poise::serenity_prelude::{
    self as serenity, ButtonStyle, ChannelId, CreateActionRow, CreateAllowedMentions, CreateButton,
    CreateEmbed, CreateMessage, EditMessage, MessageId, Nonce,
};
use sqlx::{Connection, FromRow, PgPool};
use std::time::Duration;

#[derive(FromRow)]
struct Poll {
    id: i32,
    guild_id: String,
    title: String,
    description: Option<String>,
    deadline: Option<NaiveDateTime>,
    status: String,
    discord_revision: i64,
}
#[derive(FromRow)]
struct OptionRow {
    id: i32,
    start_at: NaiveDateTime,
    end_at: NaiveDateTime,
    is_all_day: bool,
    yes: i64,
    maybe: i64,
    no: i64,
}
#[derive(FromRow)]
struct Post {
    channel_id: String,
    message_id: Option<String>,
    synced_revision: i64,
    closed: bool,
    next_attempt_at: NaiveDateTime,
}

pub async fn run_loop(ctx: serenity::Context, data: Data) {
    let mut interval = tokio::time::interval(Duration::from_secs(15));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        if let Err(error) = tick(&ctx.http, &data).await {
            tracing::warn!(%error,"poll messages could not be refreshed");
        }
    }
}
async fn tick(http: &serenity::Http, data: &Data) -> Result<(), BotError> {
    let ids:Vec<i32>=sqlx::query_scalar(r#"
        SELECT p.id FROM schedule_polls p JOIN guilds g ON g.guild_id=p.guild_id
        LEFT JOIN schedule_poll_posts s ON s.poll_id=p.id
        WHERE COALESCE(s.next_attempt_at,'epoch') <= $1
        AND (s.poll_id IS NOT NULL OR (p.status='open' AND (p.deadline IS NULL OR p.deadline>$1)))
        AND (s.poll_id IS NULL OR s.synced_revision<>p.discord_revision
             OR (NOT s.closed AND (p.status<>'open' OR p.deadline<=$1)))
        AND (s.message_id IS NOT NULL OR EXISTS (SELECT 1 FROM event_settings es WHERE es.guild_id=p.guild_id))
        ORDER BY COALESCE(s.next_attempt_at,'epoch'),p.id LIMIT 20
    "#).bind(now_jst()).fetch_all(&data.pool).await?;
    // ponytail: 15秒ごとに最大20件を逐次処理。滞留が観測されたら同時送信を最大4件にする。
    for id in ids {
        let result = tokio::time::timeout(
            Duration::from_secs(20),
            sync_one(http, &data.pool, &data.site_base_url, id),
        )
        .await;
        if !matches!(result, Ok(Ok(()))) {
            tracing::warn!(poll_id=id,error=?result,"poll message failed; retry later");
            sqlx::query("UPDATE schedule_poll_posts SET next_attempt_at=$2 WHERE poll_id=$1")
                .bind(id)
                .bind(now_jst() + chrono::Duration::minutes(1))
                .execute(&data.pool)
                .await?;
        }
    }
    Ok(())
}
async fn sync_one(
    http: &serenity::Http,
    pool: &PgPool,
    base: &str,
    id: i32,
) -> Result<(), BotError> {
    // セッションロックは専用接続を閉じて解放する。APIの通常プールを外部通信中に占有しない。
    let mut guard = sqlx::PgConnection::connect_with(pool.connect_options().as_ref()).await?;
    let locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtext('poll_message'),$1)")
            .bind(id)
            .fetch_one(&mut guard)
            .await?;
    if !locked {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    let poll:Option<Poll>=sqlx::query_as("SELECT p.* FROM schedule_polls p JOIN guilds g ON g.guild_id=p.guild_id WHERE p.id=$1 FOR SHARE OF p")
        .bind(id).fetch_optional(&mut *tx).await?;
    let Some(poll) = poll else {
        return Ok(());
    };
    let closed =
        poll.status != "open" || poll.deadline.is_some_and(|deadline| deadline <= now_jst());
    let post: Option<Post> = sqlx::query_as("SELECT * FROM schedule_poll_posts WHERE poll_id=$1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    if post.as_ref().is_some_and(|s| {
        s.next_attempt_at > now_jst()
            || (s.synced_revision == poll.discord_revision && s.closed == closed)
    }) {
        return Ok(());
    }
    let channel = if let Some(ref post) = post {
        post.channel_id.clone()
    } else {
        let channel: Option<String> = sqlx::query_scalar(
            "SELECT channel_id FROM event_settings WHERE guild_id=$1 ORDER BY id LIMIT 1",
        )
        .bind(&poll.guild_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(channel) = channel else {
            return Ok(());
        };
        sqlx::query("INSERT INTO schedule_poll_posts(poll_id,channel_id) VALUES ($1,$2)")
            .bind(id)
            .bind(&channel)
            .execute(&mut *tx)
            .await?;
        channel
    };
    let options:Vec<OptionRow>=sqlx::query_as("SELECT o.id,o.start_at,o.end_at,o.is_all_day,count(*) FILTER(WHERE v.answer='yes') AS yes,count(*) FILTER(WHERE v.answer='maybe') AS maybe,count(*) FILTER(WHERE v.answer='no') AS no FROM schedule_poll_options o LEFT JOIN schedule_poll_votes v ON v.option_id=o.id WHERE o.poll_id=$1 GROUP BY o.id ORDER BY o.position")
        .bind(id).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let channel = ChannelId::new(
        channel
            .parse::<u64>()
            .ok()
            .filter(|id| *id > 0)
            .ok_or_else(|| BotError::user("invalid poll channel"))?,
    );
    let (embed, components) = message(&poll, &options, base, closed);
    let message_id = if let Some(message_id) = post.and_then(|p| p.message_id) {
        let message_id = MessageId::new(
            message_id
                .parse::<u64>()
                .map_err(|_| BotError::user("invalid poll message"))?,
        );
        match channel
            .edit_message(
                http,
                message_id,
                EditMessage::new()
                    .embed(embed)
                    .components(components)
                    .allowed_mentions(CreateAllowedMentions::new()),
            )
            .await
        {
            Ok(_) => message_id,
            // 管理者が投稿を削除した場合は復活させず、次の変更でも再投稿しない。
            Err(serenity::Error::Http(ref error))
                if error.status_code() == Some(serenity::http::StatusCode::NOT_FOUND) =>
            {
                sqlx::query(
                    "UPDATE schedule_poll_posts SET next_attempt_at='9999-12-31' WHERE poll_id=$1",
                )
                .bind(id)
                .execute(pool)
                .await?;
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        }
    } else {
        // 同じnonceを短時間の再送に使う。Discordが受理してDB保存だけ失敗した場合も重複を抑える。
        channel
            .send_message(
                http,
                CreateMessage::new()
                    .embed(embed)
                    .components(components)
                    .allowed_mentions(CreateAllowedMentions::new())
                    .nonce(Nonce::String(format!("poll:{id}")))
                    .enforce_nonce(true),
            )
            .await?
            .id
    };
    let updated=sqlx::query("UPDATE schedule_poll_posts SET message_id=$2,synced_revision=$3,closed=$4,next_attempt_at=$5 WHERE poll_id=$1")
        .bind(id).bind(message_id.to_string()).bind(poll.discord_revision).bind(closed).bind(now_jst()).execute(pool).await?;
    if updated.rows_affected() == 0 {
        // 送信中に日程調整が削除された場合は作った投稿も片付ける。
        channel.delete_message(http, message_id).await?;
    }
    Ok(())
}
fn message(
    poll: &Poll,
    options: &[OptionRow],
    base: &str,
    closed: bool,
) -> (CreateEmbed, Vec<CreateActionRow>) {
    let status = if poll.status == "confirmed" {
        "確定済み"
    } else if closed {
        "締切済み"
    } else {
        "回答受付中"
    };
    let deadline = poll
        .deadline
        .map(|d| d.format("%Y/%m/%d %H:%M JST").to_string())
        .unwrap_or_else(|| "なし".into());
    let mut embed = CreateEmbed::new()
        .title(format!("日程調整: {}", poll.title))
        .url(format!(
            "{base}/dashboard/{}/polls/{}",
            poll.guild_id, poll.id
        ))
        .description(format!(
            "{status}・締切: {deadline}\n{}",
            poll.description.as_deref().unwrap_or("")
        ))
        .colour(0x5865f2);
    let mut rows = Vec::new();
    for (index, option) in options.iter().enumerate() {
        let time = if option.is_all_day {
            format!(
                "{} 〜 {}（終日）",
                option.start_at.format("%Y/%m/%d"),
                option.end_at.format("%Y/%m/%d")
            )
        } else {
            format!(
                "{} 〜 {} JST",
                option.start_at.format("%Y/%m/%d %H:%M"),
                option.end_at.format("%Y/%m/%d %H:%M")
            )
        };
        embed = embed.field(
            format!("候補 {}: {time}", index + 1),
            format!(
                "○ {}人 / △ {}人 / × {}人",
                option.yes, option.maybe, option.no
            ),
            false,
        );
        rows.push(CreateActionRow::Buttons(
            [
                ("yes", "○", ButtonStyle::Success),
                ("maybe", "△", ButtonStyle::Secondary),
                ("no", "×", ButtonStyle::Danger),
            ]
            .into_iter()
            .map(|(answer, label, style)| {
                CreateButton::new(custom_id(poll.id, option.id, answer))
                    .label(format!("候補{} {label}", index + 1))
                    .style(style)
                    .disabled(closed)
            })
            .collect(),
        ));
    }
    (embed, rows)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn five_candidates_fit_and_closed_buttons_are_disabled() {
        let poll = Poll {
            id: 1,
            guild_id: "123".into(),
            title: "調整".into(),
            description: None,
            deadline: None,
            status: "open".into(),
            discord_revision: 1,
        };
        let options = (1..=5)
            .map(|id| OptionRow {
                id,
                start_at: "2099-01-01T10:00:00".parse().unwrap(),
                end_at: "2099-01-01T11:00:00".parse().unwrap(),
                is_all_day: false,
                yes: 2,
                maybe: 1,
                no: 0,
            })
            .collect::<Vec<_>>();
        let (embed, rows) = message(&poll, &options, "https://example.com", true);
        let json = serde_json::to_value(rows).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 5);
        for row in json.as_array().unwrap() {
            assert_eq!(row["components"].as_array().unwrap().len(), 3);
            for button in row["components"].as_array().unwrap() {
                assert_eq!(button["disabled"], true);
                assert!(
                    crate::polls::parse_custom_id(button["custom_id"].as_str().unwrap()).is_some()
                );
            }
        }
        assert!(
            serde_json::to_string(&embed)
                .unwrap()
                .contains("○ 2人 / △ 1人 / × 0人")
        );
    }
}

#[cfg(test)]
mod db_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[sqlx::test(migrations = "../api/migrations")]
    async fn post_once_refresh_votes_and_disable_after_deadline(pool: PgPool) {
        sqlx::query("INSERT INTO guilds(guild_id,name) VALUES ('1','test')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO event_settings(guild_id,channel_id) VALUES ('1','2')")
            .execute(&pool)
            .await
            .unwrap();
        let poll:i32=sqlx::query_scalar("INSERT INTO schedule_polls(guild_id,title,created_by,created_at) VALUES ('1','日程調整','3',now()) RETURNING id").fetch_one(&pool).await.unwrap();
        let option:i32=sqlx::query_scalar("INSERT INTO schedule_poll_options(poll_id,start_at,end_at,is_all_day,position) VALUES ($1,'2099-01-01 10:00','2099-01-01 11:00',false,0) RETURNING id").bind(poll).fetch_one(&pool).await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let http = serenity::HttpBuilder::new("test")
            .proxy(format!("http://{}", listener.local_addr().unwrap()))
            .ratelimiter_disabled(true)
            .build();
        let (sent, mut received) = tokio::sync::mpsc::unbounded_channel();
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let (header_end, content_length) = loop {
                    let mut buffer = [0; 4096];
                    let size = socket.read(&mut buffer).await.unwrap();
                    assert!(size > 0);
                    request.extend_from_slice(&buffer[..size]);
                    if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&request[..end]);
                        let length = header
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .map(|v| v.parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        break (end + 4, length);
                    }
                };
                while request.len() < header_end + content_length {
                    let mut buffer = [0; 4096];
                    let size = socket.read(&mut buffer).await.unwrap();
                    assert!(size > 0);
                    request.extend_from_slice(&buffer[..size]);
                }
                let method = String::from_utf8_lossy(&request[..header_end])
                    .split_whitespace()
                    .next()
                    .unwrap()
                    .to_owned();
                let body: serde_json::Value =
                    serde_json::from_slice(&request[header_end..header_end + content_length])
                        .unwrap();
                sent.send((method, body)).unwrap();
                let mut message = serenity::Message::default();
                message.id = MessageId::new(4);
                message.channel_id = ChannelId::new(2);
                let body = serde_json::to_string(&message).unwrap();
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
        });
        let (a, b) = tokio::join!(
            sync_one(&http, &pool, "https://example.com", poll),
            sync_one(&http, &pool, "https://example.com", poll)
        );
        a.unwrap();
        b.unwrap();
        let (method, body) = received.recv().await.unwrap();
        assert_eq!(method, "POST");
        assert_eq!(body["enforce_nonce"], true);
        assert_eq!(body["allowed_mentions"]["parse"], serde_json::json!([]));
        assert!(received.try_recv().is_err());
        crate::poll_votes::vote(&pool, "1", poll, option, "3", "yes")
            .await
            .unwrap();
        sync_one(&http, &pool, "https://example.com", poll)
            .await
            .unwrap();
        let (method, body) = received.recv().await.unwrap();
        assert_eq!(method, "PATCH");
        assert_eq!(
            body["embeds"][0]["fields"][0]["value"],
            "○ 1人 / △ 0人 / × 0人"
        );
        sqlx::query("UPDATE schedule_polls SET deadline='2000-01-01' WHERE id=$1")
            .bind(poll)
            .execute(&pool)
            .await
            .unwrap();
        // 期限だけが過ぎた場合にも、revision差分なしでボタンを無効化する。
        sqlx::query("UPDATE schedule_poll_posts SET synced_revision=(SELECT discord_revision FROM schedule_polls WHERE id=$1) WHERE poll_id=$1").bind(poll).execute(&pool).await.unwrap();
        sync_one(&http, &pool, "https://example.com", poll)
            .await
            .unwrap();
        let (method, body) = received.recv().await.unwrap();
        assert_eq!(method, "PATCH");
        assert_eq!(body["components"][0]["components"][0]["disabled"], true);
        sync_one(&http, &pool, "https://example.com", poll)
            .await
            .unwrap();
        assert!(received.try_recv().is_err());
        server.abort();
    }
    #[sqlx::test(migrations = "../api/migrations")]
    async fn queue_skips_unconfigured_and_closed_polls_and_retries_failed_posts(pool: PgPool) {
        use std::{
            collections::HashMap,
            sync::{Arc, atomic::AtomicBool},
        };
        sqlx::query(
            "INSERT INTO guilds(guild_id,name) VALUES ('1','configured'),('3','unconfigured')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO event_settings(guild_id,channel_id) VALUES ('1','2')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO schedule_polls(guild_id,title,created_by,created_at,status) VALUES ('1','open','5',now(),'open'),('1','closed','5',now(),'closed'),('3','unconfigured','5',now(),'open')").execute(&pool).await.unwrap();
        let data = Data {
            pool: pool.clone(),
            site_base_url: "https://example.com".into(),
            log_channel_id: None,
            invite_url: String::new(),
            support_guild_id: None,
            guild_sync: Arc::new(tokio::sync::Mutex::new(())),
            tasks_started: Arc::new(AtomicBool::new(false)),
            presence_tasks: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        };
        let http = serenity::HttpBuilder::new("test")
            .proxy("http://127.0.0.1:1")
            .ratelimiter_disabled(true)
            .build();
        tick(&http, &data).await.unwrap();
        let rows: Vec<(i32, NaiveDateTime)> =
            sqlx::query_as("SELECT poll_id,next_attempt_at FROM schedule_poll_posts")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, 1);
        assert!(rows[0].1 > now_jst());
        tick(&http, &data).await.unwrap();
        let after: NaiveDateTime =
            sqlx::query_scalar("SELECT next_attempt_at FROM schedule_poll_posts WHERE poll_id=1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(after, rows[0].1);
    }
}
