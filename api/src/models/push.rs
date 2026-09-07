//! 本人の端末と、端末間で共有する通知範囲。
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::error::ApiError;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum PushScope {
    All,
    Created,
    Off,
}
impl PushScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Created => "created",
            Self::Off => "off",
        }
    }
}
#[derive(Deserialize, ToSchema)]
pub struct SubscriptionInput {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub device_name: String,
}
#[derive(Serialize, sqlx::FromRow, ToSchema)]
pub struct Subscription {
    pub id: i32,
    pub device_name: String,
    pub created_at: DateTime<Utc>,
    pub last_sent_at: Option<DateTime<Utc>>,
    pub failure_count: i32,
    pub disabled: bool,
}
#[derive(Serialize, ToSchema)]
pub struct PushSettings {
    pub scope: String,
    pub subscriptions: Vec<Subscription>,
}

/// 任意 URL への送信を防ぐ。主要ブラウザのプッシュサービスだけを受け付け、転送も追わない。
pub fn validate_endpoint(endpoint: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(endpoint) else {
        return false;
    };
    let host = url.host_str().unwrap_or_default();
    endpoint.len() <= 2048
        && url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.fragment().is_none()
        && (host == "fcm.googleapis.com"
            || host == "updates.push.services.mozilla.com"
            || host == "web.push.apple.com"
            || host.ends_with(".push.apple.com")
            || host == "wns.windows.com"
            || host.ends_with(".notify.windows.com"))
}
impl SubscriptionInput {
    pub fn validate(&self) -> Result<(), ApiError> {
        let public = URL_SAFE_NO_PAD.decode(&self.p256dh).unwrap_or_default();
        let auth = URL_SAFE_NO_PAD.decode(&self.auth).unwrap_or_default();
        if !validate_endpoint(&self.endpoint)
            || public.len() != 65
            || public.first() != Some(&4)
            || auth.len() != 16
            || self.device_name.trim().is_empty()
            || self.device_name.chars().count() > 80
        {
            return Err(ApiError::BadRequest(
                "購読 URL・暗号鍵・端末名を確認してください".into(),
            ));
        }
        Ok(())
    }
}

pub async fn get(pool: &PgPool, user: &str) -> sqlx::Result<PushSettings> {
    let scope = sqlx::query_scalar("SELECT scope FROM user_push_settings WHERE user_id = $1")
        .bind(user)
        .fetch_optional(pool)
        .await?
        .unwrap_or_else(|| "off".into());
    let subscriptions = sqlx::query_as("SELECT id, device_name, created_at, last_sent_at, failure_count, disabled FROM push_subscriptions WHERE user_id = $1 ORDER BY created_at, id")
        .bind(user).fetch_all(pool).await?;
    Ok(PushSettings {
        scope,
        subscriptions,
    })
}

pub async fn set_scope(pool: &PgPool, user: &str, scope: &PushScope) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO user_push_settings (user_id, scope) VALUES ($1, $2) ON CONFLICT (user_id) DO UPDATE SET scope = EXCLUDED.scope")
        .bind(user).bind(scope.as_str()).execute(pool).await?;
    Ok(())
}

pub async fn subscribe(
    pool: &PgPool,
    user: &str,
    input: &SubscriptionInput,
) -> Result<(), ApiError> {
    input.validate()?;
    let mut tx = pool.begin().await?;
    // 端末上限の確認と登録をユーザー単位で直列化する。
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 176))")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    let owner: Option<String> =
        sqlx::query_scalar("SELECT user_id FROM push_subscriptions WHERE endpoint = $1")
            .bind(&input.endpoint)
            .fetch_optional(&mut *tx)
            .await?;
    if owner.as_deref().is_some_and(|owner| owner != user) {
        return Err(ApiError::Conflict(
            "この端末の購読を解除してから登録し直してください".into(),
        ));
    }
    if owner.is_none() {
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM push_subscriptions WHERE user_id = $1")
                .bind(user)
                .fetch_one(&mut *tx)
                .await?;
        if count >= 10 {
            return Err(ApiError::BadRequest("端末は10台まで登録できます".into()));
        }
    }
    let saved = sqlx::query("INSERT INTO push_subscriptions (user_id, endpoint, p256dh, auth, device_name) VALUES ($1,$2,$3,$4,$5) ON CONFLICT (endpoint) DO UPDATE SET p256dh = EXCLUDED.p256dh, auth = EXCLUDED.auth, device_name = EXCLUDED.device_name, failure_count = 0, disabled = false WHERE push_subscriptions.user_id = EXCLUDED.user_id")
        .bind(user).bind(&input.endpoint).bind(&input.p256dh).bind(&input.auth).bind(input.device_name.trim()).execute(&mut *tx).await?;
    if saved.rows_affected() == 0 {
        return Err(ApiError::Conflict(
            "この端末の購読を解除してから登録し直してください".into(),
        ));
    }
    tx.commit().await?;
    Ok(())
}
pub async fn remove(pool: &PgPool, user: &str, id: i32) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM push_subscriptions WHERE user_id = $1 AND id = $2")
        .bind(user)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
pub async fn remove_endpoint(pool: &PgPool, user: &str, endpoint: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM push_subscriptions WHERE user_id = $1 AND endpoint = $2")
        .bind(user)
        .bind(endpoint)
        .execute(pool)
        .await?;
    Ok(())
}
