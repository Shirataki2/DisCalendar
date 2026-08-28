//! Better Auth (web 側) が発行するセッションの検証。
//!
//! Better Auth はセッション cookie を `<token>.<base64(HMAC-SHA256(secret, token))>` の形で
//! 署名付きで発行する (better-call の `signCookieValue`)。API は同じ secret で署名を検証し、
//! 共有 DB の `session` テーブルから有効なセッションを引いて、連携済み Discord アカウントの
//! ユーザー ID を得る。これにより Discord のユーザートークンを API に渡す必要がなくなる。

use std::{future::Future, pin::Pin};

use actix_web::{FromRequest, HttpMessage as _, HttpRequest, dev::Payload, http::header, web};
use anyhow::anyhow;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, KeyInit as _, Mac as _};
use sha2::Sha256;
use sqlx::PgPool;

use crate::{error::ApiError, state::AppState};

/// セッション検証に必要な設定
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// `BETTER_AUTH_SECRET`
    pub secret: String,
    /// 受け付けるセッション cookie 名 (`__Secure-` 付き / なし)
    pub cookie_names: Vec<String>,
}

/// Better Auth のセッションから復元した認証済みユーザー。
/// ハンドラの引数に書くだけで、未認証なら 401 が返る
#[derive(Debug, Clone)]
pub struct AuthUser {
    /// Better Auth の `user.id`
    pub id: String,
    pub name: String,
    /// 連携済み Discord アカウントのユーザー ID (Snowflake、文字列)
    pub discord_user_id: String,
}

impl FromRequest for AuthUser {
    type Error = ApiError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, ApiError>>>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();
        Box::pin(async move {
            // 同一リクエスト内で複数の extractor から使われても DB を引くのは 1 回にする
            if let Some(user) = req.extensions().get::<AuthUser>() {
                return Ok(user.clone());
            }
            let state = req
                .app_data::<web::Data<AppState>>()
                .ok_or_else(|| anyhow!("AppState is not registered"))?;
            let token =
                session_token_from_request(&req, &state.auth).ok_or(ApiError::Unauthorized)?;
            let user = load_user_by_session(&state.pool, &token)
                .await?
                .ok_or(ApiError::Unauthorized)?;
            req.extensions_mut().insert(user.clone());
            // 利用の記録 (#81)。運営者の閲覧が指標に混ざらないよう管理コンソールは数えない
            if !req.path().starts_with("/admin") {
                record_daily_activity(state.get_ref(), &user.id).await;
            }
            Ok(user)
        })
    }
}

/// 日次アクティビティ (#81) を記録する。1 人 1 日 1 行で、同じ日の 2 回目以降は
/// キャッシュで DB への往復ごとスキップする。記録は本来のリクエストの付随処理なので、
/// 失敗しても警告ログだけ残してリクエスト自体は通す
async fn record_daily_activity(state: &AppState, user_id: &str) {
    let day = crate::models::now_jst().date();
    if state.activity_days.get(user_id).await == Some(day) {
        return;
    }
    match crate::models::user_activity::record(&state.pool, user_id, day).await {
        Ok(()) => state.activity_days.insert(user_id.to_owned(), day).await,
        Err(error) => tracing::warn!(%error, "failed to record daily user activity"),
    }
}

/// リクエストから署名検証済みのセッショントークンを取り出す。
/// 1. Better Auth の cookie (ブラウザ / Next.js rewrites 経由)
/// 2. `Authorization: Bearer <cookie と同じ署名付き値>` (curl やサーバー間呼び出し用)
fn session_token_from_request(req: &HttpRequest, auth: &AuthConfig) -> Option<String> {
    for name in &auth.cookie_names {
        if let Some(cookie) = req.cookie(name) {
            // cookie 名が一致したのに署名が不正なら、別の手段にフォールバックせず拒否する
            return verify_signed_value(cookie.value(), &auth.secret).map(str::to_owned);
        }
    }
    let value = req
        .headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?
        .trim();
    verify_signed_value(value, &auth.secret).map(str::to_owned)
}

/// better-call 形式の署名付き値 `<value>.<base64(HMAC-SHA256(secret, value))>` を検証し、
/// 正しければ `value` を返す
pub fn verify_signed_value<'a>(signed: &'a str, secret: &str) -> Option<&'a str> {
    let (value, signature) = signed.rsplit_once('.')?;
    if value.is_empty() {
        return None;
    }
    let signature = STANDARD.decode(signature).ok()?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(value.as_bytes());
    // verify_slice は定数時間比較
    mac.verify_slice(&signature).ok()?;
    Some(value)
}

/// Better Auth のスキーマ (session / user / account) から有効なセッションのユーザーを引く
async fn load_user_by_session(pool: &PgPool, token: &str) -> Result<Option<AuthUser>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT u.id AS "id!", u.name AS "name!", a."accountId" AS "discord_user_id!"
        FROM "session" s
        JOIN "user" u ON u.id = s."userId"
        JOIN "account" a ON a."userId" = u.id AND a."providerId" = 'discord'
        WHERE s.token = $1 AND s."expiresAt" > now()
        LIMIT 1
        "#,
        token
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| AuthUser {
        id: r.id,
        name: r.name,
        discord_user_id: r.discord_user_id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // node -e 'require("crypto").createHmac("sha256","test-secret").update("abc").digest("base64")'
    const SIGNED: &str = "abc.5MTa3WVx+Bp7AZ0hbbHvOHewyI+qvFamb4iX7jKFdZM=";

    #[test]
    fn accepts_value_signed_by_better_auth() {
        assert_eq!(verify_signed_value(SIGNED, "test-secret"), Some("abc"));
    }

    #[test]
    fn rejects_wrong_secret() {
        assert_eq!(verify_signed_value(SIGNED, "other-secret"), None);
    }

    #[test]
    fn rejects_tampered_value() {
        let tampered = SIGNED.replacen("abc", "abd", 1);
        assert_eq!(verify_signed_value(&tampered, "test-secret"), None);
    }

    #[test]
    fn rejects_malformed_input() {
        assert_eq!(verify_signed_value("no-signature", "test-secret"), None);
        assert_eq!(verify_signed_value(".sig", "test-secret"), None);
        assert_eq!(verify_signed_value("abc.not-base64!!", "test-secret"), None);
    }

    #[test]
    fn value_may_contain_dots() {
        // rsplit なので value 側に '.' があっても最後の '.' で分割される
        let mut mac = Hmac::<Sha256>::new_from_slice(b"s").unwrap();
        mac.update(b"a.b");
        let sig = STANDARD.encode(mac.finalize().into_bytes());
        assert_eq!(verify_signed_value(&format!("a.b.{sig}"), "s"), Some("a.b"));
    }
}
