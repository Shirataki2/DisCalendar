//! Webhook の閲覧・操作は restricted に関わらずサーバー管理権限を要求する。
use super::GuildMember;
use crate::{error::ApiError, models::feed_tokens::generate_token, outbound_http, state::AppState};
use actix_web::{HttpResponse, delete, get, post, put, web};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateInput {
    pub url: String,
    pub kind: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
pub struct EnabledInput {
    pub enabled: bool,
}
#[derive(Deserialize)]
pub struct Path {
    pub guild_id: String,
    pub webhook_id: i64,
}

fn require_manage(member: &GuildMember) -> Result<(), ApiError> {
    if member.permissions().can_manage_server() {
        Ok(())
    } else {
        Err(ApiError::Forbidden("サーバー管理権限が必要です".into()))
    }
}
fn response(value: Value) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(value)
}
async fn lock_hook<'a>(
    tx: &mut sqlx::Transaction<'a, sqlx::Postgres>,
    path: &Path,
) -> Result<(), ApiError> {
    sqlx::query("SELECT id FROM guild_webhooks WHERE guild_id=$1 AND id=$2 FOR UPDATE")
        .bind(&path.guild_id)
        .bind(path.webhook_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| ApiError::NotFound("Webhook がありません".into()))?;
    Ok(())
}

#[utoipa::path(tag="webhooks", params(("guild_id" = String, Path)), responses((status=200, body=Value)))]
#[get("/{guild_id}/webhooks")]
pub async fn list(
    member: GuildMember,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    require_manage(&member)?;
    let rows = sqlx::query("SELECT id, url, kind, enabled, consecutive_failures, disabled_reason, created_by, created_at FROM guild_webhooks WHERE guild_id=$1 ORDER BY id")
        .bind(member.guild_id()).fetch_all(&state.pool).await?;
    let mut items = Vec::new();
    for row in rows {
        let id: i64 = row.get("id");
        let logs: Vec<Value> = sqlx::query_scalar("SELECT to_jsonb(d) - 'webhook_id' - 'id' FROM (SELECT * FROM guild_webhook_deliveries WHERE webhook_id=$1 ORDER BY id DESC LIMIT 20) d")
            .bind(id).fetch_all(&state.pool).await?;
        // パス・クエリ全体が秘密になり得るため、origin 以外は返さない。
        let masked = outbound_http::parse_url(row.get("url"))
            .map(|u| format!("{}/…", u.origin().ascii_serialization()))
            .unwrap_or_else(|_| "非表示".into());
        items.push(json!({"id":id.to_string(), "url": masked, "kind":row.get::<String,_>("kind"), "enabled":row.get::<bool,_>("enabled"), "consecutive_failures":row.get::<i32,_>("consecutive_failures"), "disabled_reason":row.get::<Option<String>,_>("disabled_reason"), "created_by":row.get::<String,_>("created_by"), "created_at":row.get::<chrono::DateTime<chrono::Utc>,_>("created_at"), "deliveries":logs}));
    }
    Ok(response(json!(items)))
}

#[utoipa::path(tag="webhooks", params(("guild_id" = String, Path)), responses((status=200, body=Value)))]
#[post("/{guild_id}/webhooks")]
pub async fn create(
    member: GuildMember,
    state: web::Data<AppState>,
    body: web::Json<CreateInput>,
) -> Result<HttpResponse, ApiError> {
    require_manage(&member)?;
    let url = outbound_http::parse_url(&body.url).map_err(|e| ApiError::BadRequest(e.into()))?;
    if !matches!(body.kind.as_str(), "json" | "discord")
        || (body.kind == "discord" && !crate::webhooks::discord_url(&url))
    {
        return Err(ApiError::BadRequest(
            "種類、または Discord Webhook URL が正しくありません".into(),
        ));
    }
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        outbound_http::client_for(&url),
    )
    .await
    .map_err(|_| ApiError::BadRequest("URL の確認がタイムアウトしました".into()))?
    .map_err(|e| ApiError::BadRequest(e.into()))?;
    let mut tx = state.pool.begin().await?;
    // 登録をギルドごとに直列化して同時登録でも上限を守る。
    sqlx::query("SELECT guild_id FROM guilds WHERE guild_id=$1 FOR UPDATE")
        .bind(member.guild_id())
        .execute(&mut *tx)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM guild_webhooks WHERE guild_id=$1")
        .bind(member.guild_id())
        .fetch_one(&mut *tx)
        .await?;
    if count >= 5 {
        return Err(ApiError::BadRequest(
            "Webhook は1サーバー5件までです".into(),
        ));
    }
    let secret = generate_token();
    let id: i64 = sqlx::query_scalar("INSERT INTO guild_webhooks (guild_id,url,kind,secret,created_by) VALUES ($1,$2,$3,$4,$5) RETURNING id")
        .bind(member.guild_id()).bind(url.as_str()).bind(&body.kind).bind(&secret).bind(&member.user.discord_user_id).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(response(json!({"id":id.to_string(), "secret":secret})))
}

#[utoipa::path(tag="webhooks", params(("guild_id" = String, Path), ("webhook_id" = i64, Path)), responses((status=204)))]
#[put("/{guild_id}/webhooks/{webhook_id}")]
pub async fn set_enabled(
    member: GuildMember,
    state: web::Data<AppState>,
    path: web::Path<Path>,
    body: web::Json<EnabledInput>,
) -> Result<HttpResponse, ApiError> {
    require_manage(&member)?;
    let mut tx = state.pool.begin().await?;
    lock_hook(&mut tx, &path).await?;
    sqlx::query("UPDATE guild_webhooks SET enabled=$2, generation=generation + CASE WHEN $2 THEN 0 ELSE 1 END, consecutive_failures=0, disabled_reason=NULL WHERE id=$1")
        .bind(path.webhook_id).bind(body.enabled).execute(&mut *tx).await?;
    if !body.enabled {
        sqlx::query("DELETE FROM guild_webhook_outbox WHERE webhook_id=$1")
            .bind(path.webhook_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(HttpResponse::NoContent().finish())
}

#[utoipa::path(tag="webhooks", params(("guild_id" = String, Path), ("webhook_id" = i64, Path)), responses((status=204)))]
#[delete("/{guild_id}/webhooks/{webhook_id}")]
pub async fn remove(
    member: GuildMember,
    state: web::Data<AppState>,
    path: web::Path<Path>,
) -> Result<HttpResponse, ApiError> {
    require_manage(&member)?;
    let deleted = sqlx::query("DELETE FROM guild_webhooks WHERE guild_id=$1 AND id=$2")
        .bind(&path.guild_id)
        .bind(path.webhook_id)
        .execute(&state.pool)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(ApiError::NotFound("Webhook がありません".into()));
    }
    Ok(HttpResponse::NoContent().finish())
}

#[utoipa::path(tag="webhooks", params(("guild_id" = String, Path), ("webhook_id" = i64, Path)), responses((status=200, body=Value)))]
#[post("/{guild_id}/webhooks/{webhook_id}/secret")]
pub async fn rotate(
    member: GuildMember,
    state: web::Data<AppState>,
    path: web::Path<Path>,
) -> Result<HttpResponse, ApiError> {
    require_manage(&member)?;
    let secret = generate_token();
    let changed = sqlx::query("UPDATE guild_webhooks SET secret=$3 WHERE guild_id=$1 AND id=$2")
        .bind(&path.guild_id)
        .bind(path.webhook_id)
        .bind(&secret)
        .execute(&state.pool)
        .await?;
    if changed.rows_affected() == 0 {
        return Err(ApiError::NotFound("Webhook がありません".into()));
    }
    Ok(response(json!({"secret":secret})))
}

#[utoipa::path(tag="webhooks", params(("guild_id" = String, Path), ("webhook_id" = i64, Path)), responses((status=202)))]
#[post("/{guild_id}/webhooks/{webhook_id}/test")]
pub async fn test_send(
    member: GuildMember,
    state: web::Data<AppState>,
    path: web::Path<Path>,
) -> Result<HttpResponse, ApiError> {
    require_manage(&member)?;
    let mut tx = state.pool.begin().await?;
    lock_hook(&mut tx, &path).await?;
    // テスト連打によるキュー増大を防ぐ。同じ Webhook では1分に1回まで。
    let allowed: bool = sqlx::query_scalar("SELECT enabled AND NOT EXISTS (SELECT 1 FROM guild_webhook_outbox WHERE webhook_id=$1 AND kind='webhook.test') AND NOT EXISTS (SELECT 1 FROM guild_webhook_deliveries WHERE webhook_id=$1 AND kind='webhook.test' AND attempted_at > now()-interval '1 minute') FROM guild_webhooks WHERE id=$1")
        .bind(path.webhook_id).fetch_one(&mut *tx).await?;
    if !allowed {
        return Err(ApiError::BadRequest(
            "有効な Webhook で1分ほど間隔を空けてお試しください".into(),
        ));
    }
    sqlx::query("INSERT INTO guild_webhook_outbox (webhook_id,kind,payload,actor_id,generation) SELECT id,'webhook.test','null'::jsonb,$2,generation FROM guild_webhooks WHERE id=$1")
        .bind(path.webhook_id).bind(&member.user.discord_user_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(HttpResponse::Accepted().json(json!({"queued":true})))
}
