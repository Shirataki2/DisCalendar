//! 永続 outbox の配信。Webhook ごとの先頭だけを処理し、再試行でも順序を保つ。
use crate::outbound_http;
use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use sqlx::{PgPool, Row};
use std::{fmt::Write as _, time::Duration};

pub const FAILURE_LIMIT: i32 = 10;
pub const MAX_ATTEMPTS: i32 = 3;

pub fn signature(secret: &str, body: &[u8]) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body);
    let mut result = String::from("sha256=");
    for byte in mac.finalize().into_bytes() {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}

pub fn discord_url(url: &reqwest::Url) -> bool {
    let segments: Vec<_> = url.path().split('/').collect();
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && matches!(
            url.host_str(),
            Some("discord.com" | "discordapp.com" | "canary.discord.com" | "ptb.discord.com")
        )
        && segments.len() == 5
        && segments[1] == "api"
        && segments[2] == "webhooks"
        && !segments[3].is_empty()
        && segments[3].bytes().all(|b| b.is_ascii_digit())
        && !segments[4].is_empty()
}

fn discord_payload(kind: &str, event: &Value, site: &str) -> Value {
    let title = match kind {
        "event.created" => "予定が作成されました",
        "event.updated" => "予定が変更されました",
        "event.deleted" => "予定が削除されました",
        _ => "Webhook のテスト送信",
    };
    if event.is_null() {
        return json!({"content": title, "allowed_mentions": {"parse": []}});
    }
    let color = event["color"]
        .as_str()
        .and_then(|s| u32::from_str_radix(s.trim_start_matches('#'), 16).ok())
        .unwrap_or(0x5865f2);
    let dates = event["start_at"]
        .as_str()
        .and_then(|v| v.parse().ok())
        .zip(event["end_at"].as_str().and_then(|v| v.parse().ok()))
        .map(|(start, end)| {
            crate::discord_datetime::format_date_range(event["is_all_day"] == true, start, end)
        })
        .unwrap_or_else(|| "日時を表示できません".into());
    json!({"allowed_mentions": {"parse": []}, "embeds": [{
        "title": event["name"], "description": event["description"], "color": color,
        "author": {"name": title},
        "url": format!("{}/dashboard/{}", site.trim_end_matches('/'), event["guild_id"].as_str().unwrap_or("")),
        "fields": [{"name": "日時", "value": dates}]
    }]})
}

pub async fn run(pool: PgPool, site: String) {
    loop {
        match deliver_one(&pool, &site).await {
            Ok(true) => continue,
            Ok(false) => {}
            // DB のエラー本文には URL や本文が入る可能性があるため記録しない。
            Err(_) => tracing::warn!("webhook worker database operation failed"),
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

pub async fn deliver_one(pool: &PgPool, site: &str) -> sqlx::Result<bool> {
    let mut tx = pool.begin().await?;
    // 行ロックはクラッシュ時に解放される。別ワーカーは別 Webhook を処理できる。
    // ponytail: 1 ワーカーにつき同時配信 1 件。配信遅延が増えたらワーカー数を増やす。
    let row = sqlx::query("SELECT w.id AS webhook_id, w.url, w.kind AS mode, w.secret, w.consecutive_failures, w.enabled, w.generation AS current_generation, o.generation,
        o.id, o.kind, o.payload, o.actor_id, o.occurred_at, o.attempts
        FROM guild_webhooks w JOIN guild_webhook_outbox o ON o.webhook_id = w.id
        WHERE (o.next_attempt_at <= now() OR NOT w.enabled OR o.generation <> w.generation)
        AND NOT EXISTS (SELECT 1 FROM guild_webhook_outbox earlier WHERE earlier.webhook_id = w.id AND earlier.id < o.id)
        ORDER BY o.next_attempt_at, o.id FOR NO KEY UPDATE OF w SKIP LOCKED FOR UPDATE OF o SKIP LOCKED LIMIT 1")
        .fetch_optional(&mut *tx).await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let id: i64 = row.get("id");
    let webhook_id: i64 = row.get("webhook_id");
    // 無効化と予約が並行したとき、遅れて挿入された旧世代の予約も再開時に送らない。
    if !row.get::<bool, _>("enabled")
        || row.get::<i64, _>("generation") != row.get::<i64, _>("current_generation")
    {
        sqlx::query("DELETE FROM guild_webhook_outbox WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok(true);
    }
    let kind: String = row.get("kind");
    let mut event: Value = row.get("payload");
    if !event.is_null() {
        event["notifications"] = serde_json::to_value(
            crate::models::notifications::Notification::decode_all(&event["notifications"]),
        )
        .unwrap();
        let mentions: Vec<crate::models::notification_mentions::NotificationMention> =
            serde_json::from_value(event["notification_mentions"].clone()).unwrap_or_default();
        event["notification_mentions"] = serde_json::to_value(mentions).unwrap();
    }
    let payload = if row.get::<String, _>("mode") == "discord" {
        discord_payload(&kind, &event, site)
    } else {
        json!({"delivery_id": id.to_string(), "type": kind, "occurred_at": row.get::<DateTime<Utc>,_>("occurred_at"), "event": event, "actor_id": row.get::<String,_>("actor_id")})
    };
    let body = serde_json::to_vec(&payload).unwrap();
    let send = async {
        let url = outbound_http::parse_url(row.get::<&str, _>("url"))?;
        let client = outbound_http::client_for(&url).await?;
        let mut request = client
            .post(url)
            .header("Content-Type", "application/json")
            .header("X-DisCalendar-Delivery", id.to_string())
            .header("X-DisCalendar-Event", &kind);
        if row.get::<String, _>("mode") == "json" {
            request = request.header(
                "X-DisCalendar-Signature",
                signature(row.get("secret"), &body),
            );
        }
        // 本文を読み込まない。リダイレクトも成功にはしない。
        request
            .body(body)
            .send()
            .await
            .map(|r| i32::from(r.status().as_u16()))
            .map_err(|_| "接続または TLS 通信に失敗しました")
    };
    let result = tokio::time::timeout(Duration::from_secs(10), send)
        .await
        .unwrap_or(Err("送信がタイムアウトしました"));
    let (status, error) = match result {
        Ok(s) if (200..300).contains(&s) => (Some(s), None),
        Ok(s) => (
            Some(s),
            Some("送信先が成功以外の HTTP ステータスを返しました"),
        ),
        Err(e) => (None, Some(e)),
    };
    let failures = if error.is_none() {
        0
    } else {
        row.get::<i32, _>("consecutive_failures") + 1
    };
    let attempts: i32 = row.get::<i32, _>("attempts") + 1;
    sqlx::query("INSERT INTO guild_webhook_deliveries (webhook_id, delivery_id, kind, status, error) VALUES ($1,$2,$3,$4,$5)")
        .bind(webhook_id).bind(id.to_string()).bind(&kind).bind(status).bind(error).execute(&mut *tx).await?;
    sqlx::query("UPDATE guild_webhooks SET consecutive_failures=$2, enabled=$2 < $3, generation=generation + CASE WHEN $2 >= $3 THEN 1 ELSE 0 END, disabled_reason=CASE WHEN $2 >= $3 THEN '送信が10回連続で失敗しました' ELSE NULL END WHERE id=$1")
        .bind(webhook_id).bind(failures).bind(FAILURE_LIMIT).execute(&mut *tx).await?;
    if error.is_none() || attempts >= MAX_ATTEMPTS {
        sqlx::query("DELETE FROM guild_webhook_outbox WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("UPDATE guild_webhook_outbox SET attempts=$2, next_attempt_at=now() + $3 * interval '1 second' WHERE id=$1")
            .bind(id).bind(attempts).bind(if attempts == 1 {5} else {30}).execute(&mut *tx).await?;
    }
    if failures >= FAILURE_LIMIT {
        sqlx::query("DELETE FROM guild_webhook_outbox WHERE webhook_id=$1")
            .bind(webhook_id)
            .execute(&mut *tx)
            .await?;
        tracing::warn!(webhook_id, "webhook disabled after consecutive failures");
    }
    sqlx::query("DELETE FROM guild_webhook_deliveries WHERE webhook_id=$1 AND id NOT IN (SELECT id FROM guild_webhook_deliveries WHERE webhook_id=$1 ORDER BY id DESC LIMIT 20)")
        .bind(webhook_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signature_matches_known_vector_and_body_changes() {
        assert_eq!(
            signature("key", b"The quick brown fox jumps over the lazy dog"),
            "sha256=f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
        assert_ne!(signature("key", b"{}"), signature("key", b"{ }"));
        assert!(discord_url(
            &reqwest::Url::parse("https://discord.com/api/webhooks/123/token").unwrap()
        ));
        for url in [
            "https://discord.com.evil.test/api/webhooks/123/token",
            "http://discord.com/api/webhooks/123/token",
            "https://discord.com/api/users/@me",
        ] {
            assert!(!discord_url(&reqwest::Url::parse(url).unwrap()));
        }
    }
}
