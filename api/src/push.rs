//! Bot の通知待ちを端末ごとに配信する。時刻の判定は Bot だけが行う。
use crate::{
    models::{now_jst, push::validate_endpoint},
    state::AppState,
};
use actix_web::web;
use chrono::NaiveDateTime;
use sqlx::PgPool;
use std::time::Duration;
use web_push::{ContentEncoding, SubscriptionInfo, VapidSignatureBuilder, WebPushMessageBuilder};

#[derive(Clone)]
pub struct PushConfig {
    private_key: String,
    subject: String,
}
impl std::fmt::Debug for PushConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PushConfig { [redacted] }")
    }
}
impl PushConfig {
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        let private_key = std::env::var("VAPID_PRIVATE_KEY").unwrap_or_default();
        let subject = std::env::var("VAPID_SUBJECT").unwrap_or_default();
        if private_key.is_empty() && subject.is_empty() {
            return Ok(None);
        }
        anyhow::ensure!(
            VapidSignatureBuilder::from_base64_no_sub(&private_key).is_ok(),
            "VAPID_PRIVATE_KEY must be a base64url P-256 private key"
        );
        let url = reqwest::Url::parse(&subject)
            .map_err(|_| anyhow::anyhow!("VAPID_SUBJECT must be a mailto: or HTTPS URL"))?;
        anyhow::ensure!(
            matches!(url.scheme(), "mailto" | "https"),
            "VAPID_SUBJECT must be a mailto: or HTTPS URL"
        );
        Ok(Some(Self {
            private_key,
            subject,
        }))
    }
}

pub async fn run(state: web::Data<AppState>, config: PushConfig) {
    let client = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            tracing::error!("failed to create push HTTP client");
            return;
        }
    };
    let mut interval = tokio::time::interval(Duration::from_secs(15));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        interval.tick().await;
        if let Err(error) = expand(&state.pool).await {
            tracing::warn!(error = %error, "failed to expand push outbox");
            continue;
        }
        for _ in 0..100 {
            match deliver_one(&state, &config, &client).await {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => {
                    tracing::warn!(error = %error, "push delivery database error");
                    break;
                }
            }
        }
    }
}

/// 宛先は設定範囲・作成者・登録時刻で絞る。Discord の所属は送信直前に確認する。
pub async fn expand(pool: &PgPool) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM push_outbox WHERE fire_at < $1 - interval '1 day'")
        .bind(now_jst())
        .execute(&mut *tx)
        .await?;
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM push_outbox WHERE NOT expanded ORDER BY id LIMIT 100 FOR UPDATE SKIP LOCKED")
        .fetch_all(&mut *tx).await?;
    if ids.is_empty() {
        return tx.commit().await;
    }
    sqlx::query(
        r#"
        INSERT INTO push_deliveries (outbox_id, subscription_id)
        SELECT DISTINCT o.id, s.id FROM push_outbox o
        JOIN events e ON e.id = o.event_id
        JOIN guilds g ON g.guild_id = e.guild_id
        JOIN push_subscriptions s ON NOT s.disabled
        JOIN user_push_settings p ON p.user_id = s.user_id
        JOIN "account" a ON a."userId" = s.user_id AND a."providerId" = 'discord'
        WHERE o.id = ANY($1) AND o.fire_at >= $2 - interval '1 hour'
          AND s.created_at <= o.fire_at AT TIME ZONE 'Asia/Tokyo'
          AND (p.scope = 'all' OR (p.scope = 'created' AND e.created_by = a."accountId"))
        ON CONFLICT DO NOTHING
    "#,
    )
    .bind(&ids)
    .bind(now_jst())
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE push_outbox SET expanded = true WHERE id = ANY($1)")
        .bind(&ids)
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

#[derive(sqlx::FromRow)]
struct Delivery {
    outbox_id: i64,
    subscription_id: i32,
    attempts: i32,
    endpoint: String,
    p256dh: String,
    auth: String,
    guild_id: String,
    guild_name: String,
    name: String,
    event_id: i32,
    start_at: NaiveDateTime,
    queued_start: NaiveDateTime,
    fire_at: NaiveDateTime,
    is_all_day: bool,
    notifications: serde_json::Value,
    notify_at_start: bool,
    discord_id: String,
}

async fn deliver_one(
    state: &AppState,
    config: &PushConfig,
    client: &reqwest::Client,
) -> sqlx::Result<bool> {
    let mut tx = state.pool.begin().await?;
    // 解除済み・オフ・作成者限定の範囲外・Bot 退出・古すぎる通知は送らない。
    sqlx::query(
        r#"
        UPDATE push_deliveries d SET done = true FROM push_outbox o
        WHERE d.outbox_id = o.id AND NOT d.done AND (
            o.fire_at < $1 - interval '1 hour' OR NOT EXISTS (
                SELECT 1 FROM push_subscriptions s
                JOIN user_push_settings p ON p.user_id = s.user_id
                JOIN "account" a ON a."userId" = s.user_id AND a."providerId" = 'discord'
                JOIN events e ON e.id = o.event_id JOIN guilds g ON g.guild_id = e.guild_id
                WHERE s.id = d.subscription_id AND NOT s.disabled
                AND (p.scope = 'all' OR (p.scope = 'created' AND e.created_by = a."accountId"))
            ))
    "#,
    )
    .bind(now_jst())
    .execute(&mut *tx)
    .await?;
    let row: Option<Delivery> = sqlx::query_as(
        r#"
        SELECT d.outbox_id, d.subscription_id, d.attempts, s.endpoint, s.p256dh, s.auth,
            e.guild_id, g.name AS guild_name, e.name, e.id AS event_id,
            e.start_at, o.start_at AS queued_start, o.fire_at, e.is_all_day, e.notifications,
            COALESCE(c.notify_at_start, true) AS notify_at_start, a."accountId" AS discord_id
        FROM push_deliveries d JOIN push_outbox o ON o.id = d.outbox_id
        JOIN push_subscriptions s ON s.id = d.subscription_id
        JOIN user_push_settings p ON p.user_id = s.user_id
        JOIN "account" a ON a."userId" = s.user_id AND a."providerId" = 'discord'
        JOIN events e ON e.id = o.event_id JOIN guilds g ON g.guild_id = e.guild_id
        LEFT JOIN guild_config c ON c.guild_id = e.guild_id
        WHERE NOT d.done AND d.next_attempt_at <= now() AND NOT s.disabled
          AND (p.scope = 'all' OR (p.scope = 'created' AND e.created_by = a."accountId"))
          AND o.fire_at >= $1 - interval '1 hour'
        ORDER BY d.next_attempt_at, d.outbox_id, d.subscription_id LIMIT 1
        FOR UPDATE OF d, s, p SKIP LOCKED
    "#,
    )
    .bind(now_jst())
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok(false);
    };
    let start = if row.is_all_day {
        row.start_at.date().and_time(chrono::NaiveTime::MIN)
    } else {
        row.start_at
    };
    let still_due = crate::models::notifications::Notification::fire_minutes(
        &row.notifications,
        row.notify_at_start,
    )
    .into_iter()
    .any(|minutes| {
        start.checked_sub_signed(chrono::Duration::minutes(minutes)) == Some(row.fire_at)
    });
    let outcome = if start != row.queued_start || !still_due || !validate_endpoint(&row.endpoint) {
        Outcome::Skip
    } else {
        match state
            .discord
            .is_current_member(&row.guild_id, &row.discord_id)
            .await
        {
            Ok(false) => Outcome::Skip,
            Err(_) => Outcome::Retry,
            Ok(true) => send(client, config, &row).await,
        }
    };
    match outcome {
        Outcome::Gone => {
            sqlx::query("DELETE FROM push_subscriptions WHERE id = $1")
                .bind(row.subscription_id)
                .execute(&mut *tx)
                .await?;
        }
        _ => {
            if matches!(outcome, Outcome::Sent) {
                sqlx::query("UPDATE push_subscriptions SET last_sent_at = now(), failure_count = 0 WHERE id = $1").bind(row.subscription_id).execute(&mut *tx).await?;
            } else if matches!(outcome, Outcome::Failed) && row.attempts + 1 >= 5 {
                sqlx::query("UPDATE push_subscriptions SET failure_count = failure_count + 1, disabled = failure_count + 1 >= 5 WHERE id = $1").bind(row.subscription_id).execute(&mut *tx).await?;
            }
            let done = matches!(outcome, Outcome::Sent | Outcome::Skip) || row.attempts + 1 >= 5;
            sqlx::query("UPDATE push_deliveries SET attempts = attempts + 1, done = $3, next_attempt_at = now() + interval '1 minute' WHERE outbox_id = $1 AND subscription_id = $2")
                .bind(row.outbox_id).bind(row.subscription_id).bind(done).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(true)
}
enum Outcome {
    Sent,
    Gone,
    Failed,
    Retry,
    Skip,
}

async fn send(client: &reqwest::Client, config: &PushConfig, row: &Delivery) -> Outcome {
    let info = SubscriptionInfo::new(&row.endpoint, &row.p256dh, &row.auth);
    let message = (|| {
        let mut signature = VapidSignatureBuilder::from_base64(&config.private_key, &info)?;
        signature.add_claim("sub", config.subject.as_str());
        let mut builder = WebPushMessageBuilder::new(&info);
        builder.set_vapid_signature(signature.build()?);
        builder.set_ttl(3600);
        let time = if row.is_all_day {
            format!("{} 終日", row.start_at.format("%m/%d"))
        } else {
            format!("{} (JST)", row.start_at.format("%m/%d %H:%M"))
        };
        let payload = serde_json::json!({
            "title": row.name, "body": format!("{}\n{}", row.guild_name, time),
            "url": format!("/dashboard/{}?date={}", row.guild_id, row.start_at.date()),
            // 同じ予定の再試行・近接通知は端末上で1枚にまとめる。
            "tag": format!("event-{}", row.event_id)
        })
        .to_string();
        builder.set_payload(ContentEncoding::Aes128Gcm, payload.as_bytes());
        builder.build()
    })();
    let Ok(message) = message else {
        return Outcome::Failed;
    };
    let request = web_push::request_builder::build_request::<Vec<u8>>(message);
    let (parts, body) = request.into_parts();
    let mut request = client.post(parts.uri.to_string()).body(body);
    for (key, value) in &parts.headers {
        request = request.header(key.as_str(), value.as_bytes());
    }
    // URL、鍵、プッシュサービスの本文はログに残さない。
    match request.send().await {
        Ok(response) if response.status().is_success() => Outcome::Sent,
        Ok(response) if matches!(response.status().as_u16(), 404 | 410) => Outcome::Gone,
        _ => Outcome::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode};
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicU16, Ordering},
    };

    fn config_and_public() -> (PushConfig, String) {
        // 実行時だけの鍵。秘密鍵のリテラルをテストにも保存しない。
        let mut bytes: [u8; 32] = rand::random();
        bytes[0] = 1;
        let private_key = URL_SAFE_NO_PAD.encode(bytes);
        let key = VapidSignatureBuilder::from_base64_no_sub(&private_key).unwrap();
        let public = URL_SAFE_NO_PAD.encode(key.get_public_key());
        (
            PushConfig {
                private_key,
                subject: "mailto:test@example.com".into(),
            },
            public,
        )
    }
    fn delivery(endpoint: String, public: String) -> Delivery {
        let start = now_jst();
        Delivery {
            outbox_id: 1,
            subscription_id: 1,
            attempts: 0,
            endpoint,
            p256dh: public,
            auth: URL_SAFE_NO_PAD.encode([1; 16]),
            guild_id: "111".into(),
            guild_name: "サーバー".into(),
            name: "予定".into(),
            event_id: 1,
            start_at: start,
            queued_start: start,
            fire_at: start,
            is_all_day: false,
            notifications: serde_json::json!([]),
            notify_at_start: true,
            discord_id: "222".into(),
        }
    }
    #[tokio::test]
    async fn encrypts_payload_and_classifies_push_service_responses_without_redirecting() {
        let status = Arc::new(AtomicU16::new(201));
        let server_status = status.clone();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = HttpServer::new(move || {
            let status = server_status.clone();
            App::new().route(
                "/push",
                web::post().to(move |request: HttpRequest, bytes: web::Bytes| {
                    let code = status.load(Ordering::SeqCst);
                    async move {
                        assert_eq!(
                            request.headers().get("content-encoding").unwrap(),
                            "aes128gcm"
                        );
                        assert!(
                            request
                                .headers()
                                .get("authorization")
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .starts_with("vapid ")
                        );
                        assert!(!bytes.is_empty());
                        assert!(!bytes.windows("予定".len()).any(|w| w == "予定".as_bytes()));
                        HttpResponse::build(StatusCode::from_u16(code).unwrap())
                            .insert_header(("location", "/never-follow"))
                            .finish()
                    }
                }),
            )
        })
        .listen(listener)
        .unwrap()
        .run();
        let handle = server.handle();
        tokio::spawn(server);
        let (config, public) = config_and_public();
        let row = delivery(format!("http://{address}/push"), public);
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        assert!(matches!(send(&client, &config, &row).await, Outcome::Sent));
        for code in [404, 410] {
            status.store(code, Ordering::SeqCst);
            assert!(matches!(send(&client, &config, &row).await, Outcome::Gone));
        }
        for code in [307, 429, 500] {
            status.store(code, Ordering::SeqCst);
            assert!(matches!(
                send(&client, &config, &row).await,
                Outcome::Failed
            ));
        }
        handle.stop(true).await;
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn rechecks_membership_and_disables_only_after_five_exhausted_deliveries(pool: PgPool) {
        let present = Arc::new(AtomicBool::new(false));
        let server_present = present.clone();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = HttpServer::new(move || {
            let present = server_present.clone();
            App::new().route(
                "/guilds/111/members/222",
                web::get().to(move || {
                    let present = present.load(Ordering::SeqCst);
                    async move {
                        if present {
                            HttpResponse::Ok().json(serde_json::json!({"roles":[]}))
                        } else {
                            HttpResponse::NotFound().finish()
                        }
                    }
                }),
            )
        })
        .listen(listener)
        .unwrap()
        .run();
        let handle = server.handle();
        tokio::spawn(server);
        sqlx::raw_sql(r#"
            CREATE TABLE "account" ("userId" TEXT, "providerId" TEXT, "accountId" TEXT);
            INSERT INTO "account" VALUES ('u1','discord','222');
            INSERT INTO guilds (guild_id,name,locale) VALUES ('111','サーバー','ja');
            INSERT INTO events (guild_id,name,color,is_all_day,start_at,end_at,created_by)
                VALUES ('111','予定','#5865F2',false,now() AT TIME ZONE 'Asia/Tokyo',now() AT TIME ZONE 'Asia/Tokyo','222');
            INSERT INTO push_outbox (event_id,fire_at,start_at) SELECT id,start_at,start_at FROM events;
        "#).execute(&pool).await.unwrap();
        let (config, public) = config_and_public();
        crate::models::push::subscribe(
            &pool,
            "u1",
            &crate::models::push::SubscriptionInput {
                endpoint: "https://fcm.googleapis.com/fcm/send/test".into(),
                p256dh: public,
                auth: URL_SAFE_NO_PAD.encode([1; 16]),
                device_name: "端末".into(),
            },
        )
        .await
        .unwrap();
        crate::models::push::set_scope(&pool, "u1", &crate::models::push::PushScope::All)
            .await
            .unwrap();
        sqlx::query("UPDATE push_subscriptions SET created_at = now() - interval '1 hour'")
            .execute(&pool)
            .await
            .unwrap();
        expand(&pool).await.unwrap();
        let state = AppState {
            pool: pool.clone(),
            sql_console_pool: pool.clone(),
            sql_known_words: Default::default(),
            discord: crate::discord::DiscordClient::new("test", &format!("http://{address}"))
                .unwrap(),
            site_base_url: "https://discalendar.app".into(),
            event_update_locks: Default::default(),
            auth: crate::auth::AuthConfig {
                secret: "test".into(),
                cookie_names: vec![],
            },
            activity_days: moka::future::Cache::new(1),
            admin: Default::default(),
            started_at: chrono::Utc::now(),
        };
        // プッシュサービスには接続せず、ローカルの未使用ポート経由で通信失敗を再現する。
        let client = reqwest::Client::builder()
            .proxy(reqwest::Proxy::all("http://127.0.0.1:1").unwrap())
            .timeout(Duration::from_secs(1))
            .build()
            .unwrap();
        assert!(deliver_one(&state, &config, &client).await.unwrap());
        let (done,failures):(bool,i32)=sqlx::query_as("SELECT d.done,s.failure_count FROM push_deliveries d JOIN push_subscriptions s ON s.id=d.subscription_id").fetch_one(&pool).await.unwrap();
        assert!(done);
        assert_eq!(failures, 0);
        present.store(true, Ordering::SeqCst);
        sqlx::query("UPDATE push_deliveries SET done=false,attempts=0,next_attempt_at=now()")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(r#"
            INSERT INTO events (guild_id,name,color,is_all_day,start_at,end_at,created_by)
                SELECT '111','予定','#5865F2',false,now() AT TIME ZONE 'Asia/Tokyo',now() AT TIME ZONE 'Asia/Tokyo','222' FROM generate_series(1,4);
            INSERT INTO push_outbox (event_id,fire_at,start_at) SELECT id,start_at,start_at FROM events ON CONFLICT DO NOTHING;
        "#).execute(&pool).await.unwrap();
        expand(&pool).await.unwrap();
        // 各通知の初回失敗が同じ tick に重なっても端末を停止しない。
        for _ in 0..5 {
            assert!(deliver_one(&state, &config, &client).await.unwrap());
        }
        let (disabled, failures): (bool, i32) =
            sqlx::query_as("SELECT disabled,failure_count FROM push_subscriptions")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!disabled);
        assert_eq!(failures, 0);
        for _ in 0..20 {
            sqlx::query("UPDATE push_deliveries SET next_attempt_at=now() - interval '1 second'")
                .execute(&pool)
                .await
                .unwrap();
            assert!(deliver_one(&state, &config, &client).await.unwrap());
        }
        let (disabled, failures): (bool, i32) =
            sqlx::query_as("SELECT disabled,failure_count FROM push_subscriptions")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(disabled);
        assert_eq!(failures, 5);
        assert!(!deliver_one(&state, &config, &client).await.unwrap());
        handle.stop(true).await;
    }
}
