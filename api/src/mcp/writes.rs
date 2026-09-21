//! 書き込みはDB確定後に外部反映する。外部応答が不明なら同じ操作を再実行しない。
use actix_web::{HttpResponse, web};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{McpUser, output_event, parse_time};
use crate::{
    error::ApiError,
    models::{
        events::{self, EventInput, EventRow},
        guilds, now_jst,
    },
    state::AppState,
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/events/create", web::post().to(create))
        .route("/events/update", web::post().to(update))
        .route("/events/delete", web::post().to(delete))
        .route("/events/operation", web::post().to(status));
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WriteInput {
    guild_id: String,
    idempotency_key: String,
    event_id: Option<i32>,
    expected_version: Option<String>,
    #[serde(default)]
    changes: serde_json::Map<String, Value>,
}

fn hash(value: &[u8]) -> String {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(value))
}

fn key(input: &WriteInput) -> Result<String, ApiError> {
    if input.idempotency_key.is_empty()
        || input.idempotency_key.len() > 128
        || !input.idempotency_key.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(ApiError::BadRequest(
            "idempotency_key must be 1..128 printable ASCII characters without spaces".into(),
        ));
    }
    Ok(hash(input.idempotency_key.as_bytes()))
}

async fn authorize(
    user: &McpUser,
    state: &AppState,
    guild: &str,
    action: &str,
) -> Result<(crate::discord::MemberAccess, bool), ApiError> {
    user.require_scope(&format!("events:{action}"))?;
    if !user.guild_ids.iter().any(|id| id == guild) {
        return Err(ApiError::Forbidden("guild access denied".into()));
    }
    let access = state
        .discord
        .fresh_write_access(guild, &user.discord_user_id)
        .await?
        .ok_or_else(|| ApiError::Forbidden("guild access denied".into()))?;
    let config = guilds::get_config(&state.pool, guild).await?;
    if !config.can_edit_events(access.0.permissions.can_manage_server(), &access.0.roles) {
        return Err(ApiError::Forbidden("event editing is restricted".into()));
    }
    Ok(access)
}

/// 省略とnullを区別して、完成した入力を既存バリデータへ渡す。
fn merge(
    changes: &serde_json::Map<String, Value>,
    old: Option<&EventRow>,
) -> Result<EventInput, ApiError> {
    let mut value = match old {
        Some(row) => {
            let mut value = output_event_ref(row)?;
            value["discord_scheduled_event"] = json!(row.discord_scheduled_event_id.is_some());
            value
        }
        None => {
            json!({"description":null,"notifications":[],"notification_mentions":[],"is_all_day":false,"discord_scheduled_event":false})
        }
    };
    for (name, supplied) in changes {
        if !matches!(
            name.as_str(),
            "name"
                | "description"
                | "notifications"
                | "notification_mentions"
                | "color"
                | "is_all_day"
                | "start_at"
                | "end_at"
                | "discord_scheduled_event"
        ) {
            return Err(ApiError::BadRequest("unknown event field".into()));
        }
        value[name] = if supplied.is_null()
            && matches!(name.as_str(), "notifications" | "notification_mentions")
        {
            json!([])
        } else if supplied.is_null() && name == "discord_scheduled_event" {
            json!(false)
        } else {
            supplied.clone()
        };
    }
    let all_day = value["is_all_day"]
        .as_bool()
        .ok_or_else(|| ApiError::BadRequest("is_all_day must be boolean".into()))?;
    for field in ["start_at", "end_at"] {
        let raw = value[field]
            .as_str()
            .ok_or_else(|| ApiError::BadRequest("start_at and end_at are required".into()))?;
        let time = if all_day {
            if raw.len() != 10 {
                return Err(ApiError::BadRequest(
                    "all-day events require YYYY-MM-DD".into(),
                ));
            }
            NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .ok_or_else(|| ApiError::BadRequest("invalid date".into()))?
        } else {
            parse_time(raw)?
        };
        value[field] = json!(time);
    }
    let input: EventInput = serde_json::from_value(value)
        .map_err(|_| ApiError::BadRequest("invalid event fields".into()))?;
    input.validate()?;
    Ok(input)
}

fn output_event_ref(row: &EventRow) -> Result<Value, ApiError> {
    // 生の通知JSONを保持しつつ、入力契約の日時形式へ戻す。
    let mut value = serde_json::to_value(row).map_err(anyhow::Error::from)?;
    for (field, time) in [("start_at", row.start_at), ("end_at", row.end_at)] {
        value[field] = if row.is_all_day {
            json!(time.date().to_string())
        } else {
            json!(format!("{time}+09:00").replace(' ', "T"))
        };
    }
    Ok(value)
}

#[derive(sqlx::FromRow)]
struct Operation {
    request_hash: String,
    action: String,
    result: Option<Value>,
    expired: bool,
    requires_discord_create: bool,
    requires_bot_create: bool,
}

async fn lookup(
    pool: &PgPool,
    user: &McpUser,
    guild: &str,
    key: &str,
) -> Result<Option<Operation>, ApiError> {
    Ok(sqlx::query_as("SELECT request_hash, action, result, requires_discord_create, requires_bot_create, created_at < now() - interval '24 hours' AS expired FROM mcp_event_operations WHERE user_id=$1 AND client_id=$2 AND guild_id=$3 AND key_hash=$4")
        .bind(&user.sub).bind(&user.client_id).bind(guild).bind(key).fetch_optional(pool).await?)
}

fn replay(op: Operation) -> Result<Value, ApiError> {
    if op.expired {
        return Err(ApiError::Conflict(
            "operation expired; this key cannot be reused".into(),
        ));
    }
    op.result.ok_or_else(|| {
        ApiError::Conflict("operation result is unavailable; do not use a new key".into())
    })
}

async fn create(
    user: McpUser,
    input: web::Json<WriteInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    write(user, input.into_inner(), state, "create").await
}
async fn update(
    user: McpUser,
    input: web::Json<WriteInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    write(user, input.into_inner(), state, "update").await
}
async fn delete(
    user: McpUser,
    input: web::Json<WriteInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    write(user, input.into_inner(), state, "delete").await
}

async fn status(
    user: McpUser,
    input: web::Json<WriteInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    if !user.guild_ids.contains(&input.guild_id)
        || !user.scopes.iter().any(|scope| {
            matches!(
                scope.as_str(),
                "events:create" | "events:update" | "events:delete"
            )
        })
    {
        return Err(ApiError::Forbidden("operation access denied".into()));
    }
    let key = key(&input)?;
    let op = lookup(&state.pool, &user, &input.guild_id, &key)
        .await?
        .ok_or_else(|| ApiError::NotFound("operation not found".into()))?;
    let (access, bot) = authorize(&user, &state, &input.guild_id, &op.action).await?;
    check_discord_permissions(
        &access,
        bot,
        op.requires_discord_create,
        op.requires_bot_create,
    )?;
    Ok(HttpResponse::Ok().json(replay(op)?))
}

async fn write(
    user: McpUser,
    input: WriteInput,
    state: web::Data<AppState>,
    action: &str,
) -> Result<HttpResponse, ApiError> {
    let key = key(&input)?;
    let (access, bot_can_create) = authorize(&user, &state, &input.guild_id, action).await?;
    let fingerprint = hash(&serde_json::to_vec(&json!({"action":action,"event_id":input.event_id,"expected_version":input.expected_version,"changes":input.changes})).map_err(anyhow::Error::from)?);
    let _guard = crate::models::event_links::lock_writer(&state.pool, &input.guild_id).await?;
    if let Some(op) = lookup(&state.pool, &user, &input.guild_id, &key).await? {
        if op.request_hash != fingerprint {
            return Err(ApiError::Conflict(
                "idempotency key was used with different content".into(),
            ));
        }
        check_discord_permissions(
            &access,
            bot_can_create,
            op.requires_discord_create,
            op.requires_bot_create,
        )?;
        return Ok(HttpResponse::Ok().json(replay(op)?));
    }
    let mut tx = state.pool.begin().await?;
    crate::models::event_links::lock_guild(&mut *tx, &input.guild_id).await?;
    let old = if action == "create" {
        if input.event_id.is_some() || input.expected_version.is_some() {
            return Err(ApiError::BadRequest(
                "create does not accept event_id or expected_version".into(),
            ));
        }
        None
    } else {
        let id = input
            .event_id
            .filter(|id| *id > 0)
            .ok_or_else(|| ApiError::BadRequest("positive event_id required".into()))?;
        let version = input
            .expected_version
            .as_deref()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| ApiError::BadRequest("expected_version required".into()))?;
        ensure_resolved(&mut *tx, &input.guild_id, id).await?;
        let row = events::find_by_id_for_update(&mut tx, &input.guild_id, id)
            .await?
            .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
        if super::event_version(&row)? != version {
            return Err(ApiError::Conflict(
                "event version changed; read it again".into(),
            ));
        }
        Some(row)
    };
    if action == "delete" && !input.changes.is_empty() {
        return Err(ApiError::BadRequest(
            "delete does not accept changes".into(),
        ));
    }
    let body = if action != "delete" {
        Some(merge(&input.changes, old.as_ref())?)
    } else {
        None
    };
    if let Some(old) = &old
        && crate::recurring_events::info(&mut tx, &input.guild_id, old.id)
            .await?
            .is_some()
    {
        if let Some(body) = &body {
            crate::recurring::validate_recurring_fields(body)?;
        }
        crate::recurring_events::record_exception(
            &mut tx,
            &input.guild_id,
            old.id,
            action == "delete",
        )
        .await?;
    }
    let linked = old
        .as_ref()
        .and_then(|r| r.discord_scheduled_event_id.clone());
    let desired = body
        .as_ref()
        .is_some_and(|b| b.discord_scheduled_event == Some(true));
    if let Some(body) = &body {
        events::validate_discord_flag(body, desired, now_jst())?;
        crate::models::notification_mentions::validate_roles(
            body.notification_mentions.as_deref().unwrap_or_default(),
            &access.guild,
            access
                .permissions
                .has(crate::discord::Permissions::MENTION_EVERYONE),
        )?;
        crate::models::notification_mentions::validate_targets(
            &state.discord,
            &input.guild_id,
            Some(&user.discord_user_id),
            body.notification_mentions.as_deref(),
        )
        .await?;
    }
    check_discord_permissions(
        &access,
        bot_can_create,
        desired && linked.is_none(),
        desired,
    )?;
    let row = match (action, &body) {
        ("create", Some(body)) => {
            events::create(
                &mut *tx,
                &input.guild_id,
                body,
                now_jst(),
                &user.discord_user_id,
            )
            .await?
        }
        ("update", Some(body)) => events::update(
            &mut *tx,
            &input.guild_id,
            input.event_id.unwrap(),
            body,
            &user.discord_user_id,
            now_jst(),
        )
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?,
        _ => {
            let row = old.unwrap();
            crate::webhook_outbox::enqueue(
                &mut tx,
                &input.guild_id,
                row.id,
                "event.deleted",
                &user.discord_user_id,
            )
            .await?;
            events::delete(&mut *tx, &input.guild_id, row.id).await?;
            row
        }
    };
    let event_id = row.id;
    if action != "delete" && !desired && linked.is_some() {
        crate::models::event_links::delete(&mut *tx, &input.guild_id, event_id).await?;
    }
    if action != "delete" {
        crate::webhook_outbox::enqueue(
            &mut tx,
            &input.guild_id,
            event_id,
            if action == "create" {
                "event.created"
            } else {
                "event.updated"
            },
            &user.discord_user_id,
        )
        .await?;
    }
    let mut row = row;
    row.discord_scheduled_event_id = if action == "delete" || desired {
        linked.clone()
    } else {
        None
    };
    let mut result = json!({"request_id":user.request_id,"discord_event_id":linked,"database":"succeeded","discord":if desired || linked.is_some() { "unknown" } else { "not_required" },"event_id":event_id,"event":output_event(row)?,"url":format!("{}/dashboard/{}", state.site_base_url, input.guild_id)});
    if action != "delete" {
        result["event"]["recurrence"] = serde_json::to_value(
            crate::recurring_events::info(&mut tx, &input.guild_id, event_id).await?,
        )
        .map_err(anyhow::Error::from)?;
    }
    // 外部呼び出しの前にunknownを永続化。プロセス停止・応答喪失後の再送は呼び出さない。
    sqlx::query("INSERT INTO mcp_event_operations (user_id,client_id,guild_id,key_hash,request_hash,action,event_id,result,discord_state,requires_discord_create,requires_bot_create,discord_event_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)")
        .bind(&user.sub).bind(&user.client_id).bind(&input.guild_id).bind(&key).bind(fingerprint).bind(action).bind(event_id).bind(&result).bind(result["discord"].as_str().unwrap()).bind(desired && linked.is_none()).bind(desired).bind(&linked).execute(&mut *tx).await?;
    audit(&mut tx, &user, &input.guild_id, event_id, action, &result).await?;
    tx.commit().await?;
    if desired || linked.is_some() {
        let outcome = reflect(
            &state,
            &user,
            &input,
            event_id,
            body.as_ref(),
            linked.as_deref(),
            access.permissions.create_events(),
        )
        .await;
        result["discord"] = json!(side_effect_status(&outcome));
        // 対応付けの保存に失敗しても、POST応答から判明した新しいIDを保持する。
        result["discord_event_id"] = sqlx::query_scalar::<_, Option<String>>("SELECT discord_event_id FROM mcp_event_operations WHERE user_id=$1 AND client_id=$2 AND guild_id=$3 AND key_hash=$4")
            .bind(&user.sub).bind(&user.client_id).bind(&input.guild_id).bind(&key).fetch_one(&state.pool).await?.into();
        if action != "delete"
            && let Some(row) = events::find_by_id(&state.pool, &input.guild_id, event_id).await?
        {
            result["event"] = output_event(row)?;
        }
        let mut tx = state.pool.begin().await?;
        sqlx::query("UPDATE mcp_event_operations SET result=$5, discord_state=$5->>'discord', discord_event_id=$5->>'discord_event_id' WHERE user_id=$1 AND client_id=$2 AND guild_id=$3 AND key_hash=$4")
            .bind(&user.sub).bind(&user.client_id).bind(&input.guild_id).bind(&key).bind(&result).execute(&mut *tx).await?;
        audit(&mut tx, &user, &input.guild_id, event_id, action, &result).await?;
        tx.commit().await?;
    }
    tracing::info!(guild_id = %input.guild_id, event_id, action, discord = %result["discord"], "MCP event operation completed");
    Ok(HttpResponse::Ok().json(result))
}

fn check_discord_permissions(
    access: &crate::discord::MemberAccess,
    bot: bool,
    needs_user: bool,
    needs_bot: bool,
) -> Result<(), ApiError> {
    if (needs_user && !access.permissions.create_events()) || (needs_bot && !bot) {
        return Err(ApiError::Forbidden(
            "Discord Create Events permission required".into(),
        ));
    }
    Ok(())
}

/// 不明な外部操作を、新しいキーやWeb編集で作り直して重複させない。
pub(crate) async fn ensure_resolved<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    guild_id: &str,
    event_id: impl Into<Option<i32>>,
) -> Result<(), ApiError> {
    let unknown: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM mcp_event_operations WHERE ($1::integer IS NULL OR event_id=$1) AND guild_id=$2 AND (discord_state='unknown' OR (discord_state='failed' AND NOT requires_bot_create AND discord_event_id IS NOT NULL)))")
        .bind(event_id.into()).bind(guild_id).fetch_one(executor).await?;
    if unknown {
        return Err(ApiError::Conflict(
            "Discord outcome is unknown or cleanup failed; an operator must reconcile it before editing".into(),
        ));
    }
    Ok(())
}

fn side_effect_status(outcome: &Result<(), ApiError>) -> &'static str {
    use crate::discord::DiscordError;
    match outcome {
        Ok(()) => "succeeded",
        Err(ApiError::RateLimited | ApiError::Conflict(_) | ApiError::Forbidden(_)) => "failed",
        Err(ApiError::Discord(DiscordError::GuildGone)) => "failed",
        Err(ApiError::Discord(DiscordError::Status { status, .. })) if status.is_client_error() => {
            "failed"
        }
        // 5xx、接続喪失、不正応答、外部成功後の対応付け保存失敗は成功を断定しない。
        Err(_) => "unknown",
    }
}

async fn audit(
    conn: &mut sqlx::PgConnection,
    user: &McpUser,
    guild: &str,
    id: i32,
    action: &str,
    result: &Value,
) -> Result<(), ApiError> {
    sqlx::query("INSERT INTO admin_audit_logs (actor_user_id,actor_discord_user_id,action,target_type,target_id,detail) VALUES ($1,$2,$3,'event',$4,$5)")
        .bind(&user.sub).bind(&user.discord_user_id).bind(format!("mcp.event.{action}"))
        .bind(id.to_string()).bind(json!({"request_id":user.request_id,"guild_id":guild,"client_id":user.client_id,"connection_id":user.connection_id,"database":result["database"],"discord":result["discord"]}))
        .execute(conn).await?;
    Ok(())
}

/// 結果本文を24時間で失効させ、毎時消去する。キーの墓標は消さない。
pub(super) async fn retention(pool: PgPool) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
    loop {
        interval.tick().await;
        if let Err(error) = purge_expired(&pool).await {
            tracing::error!(kind = error.kind(), "MCP operation retention failed");
        }
    }
}

async fn purge_expired(pool: &PgPool) -> Result<(), ApiError> {
    sqlx::query("UPDATE mcp_event_operations SET result=NULL WHERE result IS NOT NULL AND created_at < now() - interval '24 hours'").execute(pool).await?;
    sqlx::query("DELETE FROM admin_audit_logs WHERE action LIKE 'mcp.event.%' AND created_at < now() - interval '30 days'").execute(pool).await?;
    Ok(())
}

async fn reflect(
    state: &AppState,
    user: &McpUser,
    input: &WriteInput,
    id: i32,
    body: Option<&EventInput>,
    linked: Option<&str>,
    user_can_create: bool,
) -> Result<(), ApiError> {
    use crate::{discord::scheduled_events::ScheduledEventPayload, models::event_links};
    let guild = &input.guild_id;
    if let Some(body) = body.filter(|b| b.discord_scheduled_event == Some(true)) {
        let payload = ScheduledEventPayload::new(
            &state.site_base_url,
            guild,
            &body.name,
            body.description.as_deref(),
            body.is_all_day,
            body.start_at,
            body.end_at,
        );
        if let Some(sid) = linked
            && state
                .discord
                .modify_scheduled_event(guild, sid, &payload)
                .await?
        {
            return Ok(());
        }
        // 404後の再作成にも本人の作成権限が必要。再送時の認可にも残す。
        if !user_can_create {
            return Err(ApiError::Forbidden(
                "Discord Create Events permission required".into(),
            ));
        }
        if linked.is_some() {
            sqlx::query("UPDATE mcp_event_operations SET requires_discord_create=true WHERE user_id=$1 AND client_id=$2 AND guild_id=$3 AND key_hash=$4")
                .bind(&user.sub).bind(&user.client_id).bind(guild).bind(key(input)?).execute(&state.pool).await?;
        }
        let sid = state
            .discord
            .create_scheduled_event(guild, &payload)
            .await?;
        // リンク保存とは別に確定し、リンクの失敗・ロールバックでも照合用IDを残す。
        sqlx::query("UPDATE mcp_event_operations SET discord_event_id=$5, result=jsonb_set(result,'{discord_event_id}',to_jsonb($5::text)) WHERE user_id=$1 AND client_id=$2 AND guild_id=$3 AND key_hash=$4")
            .bind(&user.sub).bind(&user.client_id).bind(guild).bind(key(input)?).bind(&sid).execute(&state.pool).await?;
        let mut tx = state.pool.begin().await?;
        if linked.is_some() {
            event_links::set_scheduled_event_id(&mut *tx, guild, id, &sid).await?;
        } else {
            event_links::insert(&mut *tx, guild, id, &sid, now_jst()).await?;
        }
        // 保存時の通知とは別に、確定した連携IDを同じトランザクションで通知する。
        crate::webhook_outbox::enqueue(&mut tx, guild, id, "event.updated", &user.discord_user_id)
            .await?;
        tx.commit().await?;
    } else if let Some(sid) = linked
        && !state.discord.delete_scheduled_event(guild, sid).await?
    {
        return Err(ApiError::Unavailable(
            "Discord deletion could not be confirmed".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, HttpServer};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn user() -> McpUser {
        McpUser {
            request_id: "test-request".into(),
            sub: "u".into(),
            client_id: "client".into(),
            connection_id: "c".into(),
            discord_user_id: "333".into(),
            scopes: vec![
                "events:create".into(),
                "events:update".into(),
                "events:delete".into(),
            ],
            guild_ids: vec!["111".into()],
        }
    }
    fn input(key: &str, changes: Value) -> WriteInput {
        serde_json::from_value(json!({"guild_id":"111","idempotency_key":key,"changes":changes}))
            .unwrap()
    }
    async fn result(response: HttpResponse) -> Value {
        serde_json::from_slice(
            &actix_web::body::to_bytes(response.into_body())
                .await
                .unwrap(),
        )
        .unwrap()
    }

    fn test_state(pool: PgPool, origin: &str) -> web::Data<AppState> {
        web::Data::new(AppState {
            pool: pool.clone(),
            sql_console_pool: pool.clone(),
            sql_known_words: tokio::sync::Mutex::new(None),
            discord: crate::discord::DiscordClient::new("test", origin).unwrap(),
            site_base_url: origin.to_owned(),
            event_update_locks: Default::default(),
            auth: crate::auth::AuthConfig {
                secret: "test".into(),
                cookie_names: vec![],
            },
            activity_days: moka::future::Cache::new(1),
            admin: Default::default(),
            started_at: chrono::Utc::now(),
        })
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn writes_preserve_fields_versions_replays_and_uncertain_effects(pool: PgPool) {
        tokio::task::LocalSet::new()
            .run_until(exercise_writes(pool))
            .await;
    }

    async fn exercise_writes(pool: PgPool) {
        let mode = Arc::new(AtomicUsize::new(0));
        let sent = Arc::new(AtomicUsize::new(0));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (m, count) = (mode.clone(), sent.clone());
        let server = HttpServer::new(move || {
            App::new().app_data(web::Data::new((m.clone(), count.clone())))
                .default_service(web::to(|req: actix_web::HttpRequest| async move {
                    let mock = req.app_data::<web::Data<(Arc<AtomicUsize>, Arc<AtomicUsize>)>>().unwrap();
                    let mode = mock.0.load(Ordering::SeqCst);
                    let path = req.path();
                    if path == "/users/@me" { return HttpResponse::Ok().json(json!({"id":"444"})); }
                    if path.contains("scheduled-events") {
                        mock.1.fetch_add(1, Ordering::SeqCst);
                        if mode >= 10 && req.method() == actix_web::http::Method::PATCH { return HttpResponse::NotFound().finish(); }
                        if mode == 11 { return HttpResponse::Ok().body("invalid response"); }
                        if mode == 7 { return HttpResponse::Forbidden().finish(); }
                        if mode == 8 { return HttpResponse::Ok().body("invalid response"); }
                        if mode == 9 { std::future::pending::<()>().await; }
                        return HttpResponse::Ok().json(json!({"id":"777"}));
                    }
                    if mode == 1 { return HttpResponse::Forbidden().finish(); }
                    if mode == 2 && path.ends_with("/333") { return HttpResponse::NotFound().finish(); }
                    if mode == 3 && path.ends_with("/444") { return HttpResponse::NotFound().finish(); }
                    if mode == 4 { return HttpResponse::TooManyRequests().insert_header(("Retry-After", "30")).finish(); }
                    if path == "/guilds/111" { return HttpResponse::Ok().json(json!({"id":"111","name":"guild","owner_id":if mode == 5 || mode == 6 { "999" } else { "333" },"roles":[{"id":"111","name":"everyone","permissions":"17592186044416","position":0,"color":0,"managed":false}]})); }
                    HttpResponse::Ok().json(json!({"roles":if mode == 6 { vec!["555"] } else { vec![] }}))
                }))
        }).workers(1).listen(listener).unwrap().run();
        let handle = server.handle();
        tokio::spawn(server);
        let state = test_state(pool.clone(), &origin);
        sqlx::query("INSERT INTO guilds (guild_id,name,locale) VALUES ('111','Webhook','ja')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO guild_webhooks (guild_id,url,kind,secret,created_by) VALUES ('111','http://127.0.0.1','json','test-only','333')").execute(&pool).await.unwrap();
        let changes = json!({"name":"元の予定","color":"#123456","description":"保持する説明","notifications":[{"num":30,"unit":"minutes"}],"start_at":"2099-09-13T01:00:00Z","end_at":"2099-09-13T11:00:00+09:00"});
        let first = result(
            write(
                user(),
                input("create", changes.clone()),
                state.clone(),
                "create",
            )
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(first["database"], "succeeded");
        assert_eq!(first["discord"], "not_required");
        assert_eq!(first["event"]["start_at"], "2099-09-13T10:00:00+09:00");
        let id = first["event_id"].as_i64().unwrap() as i32;
        assert_eq!(
            first,
            result(
                write(
                    user(),
                    input("create", changes.clone()),
                    state.clone(),
                    "create"
                )
                .await
                .unwrap()
            )
            .await
        );
        let mut different = changes.clone();
        different["name"] = json!("別の内容");
        assert!(matches!(
            write(user(), input("create", different), state.clone(), "create").await,
            Err(ApiError::Conflict(_))
        ));
        let (left, right) = tokio::join!(
            write(
                user(),
                input("concurrent", changes.clone()),
                state.clone(),
                "create"
            ),
            write(
                user(),
                input("concurrent", changes.clone()),
                state.clone(),
                "create"
            ),
        );
        assert!(left.is_ok() || right.is_ok());
        for outcome in [left, right] {
            assert!(outcome.is_ok() || matches!(outcome, Err(ApiError::Conflict(_))));
        }
        let concurrent = result(
            write(
                user(),
                input("concurrent", changes.clone()),
                state.clone(),
                "create",
            )
            .await
            .unwrap(),
        )
        .await;
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM mcp_event_operations WHERE key_hash=$1")
                .bind(hash(b"concurrent"))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 1);
        let concurrent_id = concurrent["event_id"].as_i64().unwrap() as i32;
        let patch = |key: &str| {
            let mut patch = input(key, json!({"name":key}));
            patch.event_id = Some(concurrent_id);
            patch.expected_version = concurrent["event"]["version"].as_str().map(str::to_owned);
            patch
        };
        let (left, right) = tokio::join!(
            write(user(), patch("left"), state.clone(), "update"),
            write(user(), patch("right"), state.clone(), "update")
        );
        assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
        assert!(
            matches!(left, Err(ApiError::Conflict(_)))
                || matches!(right, Err(ApiError::Conflict(_)))
        );
        // メンバー・Bot・権限のキャッシュが温まっていても再送は毎回Discordを検証する。
        for denied in 1..=4 {
            mode.store(denied, Ordering::SeqCst);
            assert!(
                write(
                    user(),
                    input("create", changes.clone()),
                    state.clone(),
                    "create"
                )
                .await
                .is_err()
            );
        }
        mode.store(0, Ordering::SeqCst);
        sqlx::query("INSERT INTO guild_config (guild_id,restricted,editor_role_ids) VALUES ('111',true,ARRAY['555'])").execute(&pool).await.unwrap();
        mode.store(5, Ordering::SeqCst);
        assert!(matches!(
            write(
                user(),
                input("restricted", changes.clone()),
                state.clone(),
                "create"
            )
            .await,
            Err(ApiError::Forbidden(_))
        ));
        mode.store(6, Ordering::SeqCst);
        assert!(
            write(
                user(),
                input("editor", changes.clone()),
                state.clone(),
                "create"
            )
            .await
            .is_ok()
        );
        sqlx::query("DELETE FROM guild_config WHERE guild_id='111'")
            .execute(&pool)
            .await
            .unwrap();
        mode.store(0, Ordering::SeqCst);
        let mut read_only = user();
        read_only.scopes = vec!["events:read".into()];
        assert!(matches!(
            write(
                read_only,
                input("other", changes.clone()),
                state.clone(),
                "create"
            )
            .await,
            Err(ApiError::Forbidden(_))
        ));
        let mut outside = input("other", changes.clone());
        outside.guild_id = "222".into();
        assert!(matches!(
            write(user(), outside, state.clone(), "create").await,
            Err(ApiError::Forbidden(_))
        ));
        let mut patch = input("update", json!({"name":"変更"}));
        patch.event_id = Some(id);
        patch.expected_version = first["event"]["version"].as_str().map(str::to_owned);
        let updated = result(write(user(), patch, state.clone(), "update").await.unwrap()).await;
        assert_eq!(updated["event"]["description"], changes["description"]);
        assert_eq!(updated["event"]["notifications"], changes["notifications"]);
        assert_ne!(updated["event"]["version"], first["event"]["version"]);
        let mut stale = input("stale", json!({}));
        stale.event_id = Some(id);
        stale.expected_version = first["event"]["version"].as_str().map(str::to_owned);
        assert!(matches!(
            write(user(), stale, state.clone(), "delete").await,
            Err(ApiError::Conflict(_))
        ));
        // 管理・Webなど別writerの保存と対応付けもハッシュを変える。
        sqlx::query("UPDATE events SET description='管理画面' WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        let mut stale = input("stale2", json!({"name":"上書き不可"}));
        stale.event_id = Some(id);
        stale.expected_version = updated["event"]["version"].as_str().map(str::to_owned);
        assert!(matches!(
            write(user(), stale, state.clone(), "update").await,
            Err(ApiError::Conflict(_))
        ));
        let row = events::find_by_id(&pool, "111", id).await.unwrap().unwrap();
        let clear = merge(json!({"description":null,"notifications":null,"notification_mentions":null,"discord_scheduled_event":null}).as_object().unwrap(), Some(&row)).unwrap();
        assert!(clear.description.is_none());
        assert!(clear.notifications.is_empty());
        assert_eq!(clear.notification_mentions, Some(vec![]));
        for bad in [
            json!({"start_at":"2099-09-13T00:00:00"}),
            json!({"name":"x".repeat(33)}),
            json!({"description":"x".repeat(1001)}),
            json!({"guild_id":"222"}),
            json!({"name":null}),
            json!({"end_at":"2099-01-01T00:00:00Z"}),
        ] {
            assert!(merge(bad.as_object().unwrap(), Some(&row)).is_err());
        }
        let day = merge(
            json!({"is_all_day":true,"start_at":"2099-09-14","end_at":"2099-09-15"})
                .as_object()
                .unwrap(),
            Some(&row),
        )
        .unwrap();
        assert_eq!(day.end_at.date().to_string(), "2099-09-15");
        // 保存済みunknownはプロセス再起動後も外部操作を再実行しない。
        sqlx::query("UPDATE mcp_event_operations SET result=jsonb_set(result,'{discord}','\"unknown\"') WHERE key_hash=$1").bind(hash(b"create")).execute(&pool).await.unwrap();
        let replayed = result(
            write(
                user(),
                input("create", changes.clone()),
                state.clone(),
                "create",
            )
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(replayed["discord"], "unknown");
        assert_eq!(sent.load(Ordering::SeqCst), 0);
        let mut linked = changes.clone();
        linked["discord_scheduled_event"] = json!(true);
        let linked_result = result(
            write(
                user(),
                input("linked-success", linked),
                state.clone(),
                "create",
            )
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(linked_result["discord"], "succeeded");
        assert_eq!(linked_result["event"]["discord_scheduled_event_id"], "777");
        let linked_id = linked_result["event_id"].as_i64().unwrap() as i32;
        // 作成時のnullとは別に、確定したIDの更新通知が永続化される。
        let snapshot: Value = sqlx::query_scalar(
            "SELECT payload FROM guild_webhook_outbox WHERE event_id=$1 ORDER BY id DESC LIMIT 1",
        )
        .bind(linked_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(snapshot["discord_scheduled_event_id"], "777");
        let mut patch = input("linked-patch", json!({"name":"連携を保持"}));
        patch.event_id = Some(linked_id);
        patch.expected_version = linked_result["event"]["version"]
            .as_str()
            .map(str::to_owned);
        let linked_updated =
            result(write(user(), patch, state.clone(), "update").await.unwrap()).await;
        assert_eq!(linked_updated["event"]["discord_scheduled_event_id"], "777");
        mode.store(10, Ordering::SeqCst);
        let mut patch = input("recreate", json!({"name":"再作成"}));
        patch.event_id = Some(linked_id);
        patch.expected_version = linked_updated["event"]["version"]
            .as_str()
            .map(str::to_owned);
        // 古いIDを別値にして、リンク差し替えとWebhookを検証する。
        crate::models::event_links::set_scheduled_event_id(&pool, "111", linked_id, "778")
            .await
            .unwrap();
        let recreation_row = events::find_by_id(&pool, "111", linked_id)
            .await
            .unwrap()
            .unwrap();
        patch.expected_version = Some(super::super::event_version(&recreation_row).unwrap());
        let denied_body = merge(&patch.changes, Some(&recreation_row)).unwrap();
        let calls = sent.load(Ordering::SeqCst);
        assert!(matches!(
            reflect(
                &state,
                &user(),
                &patch,
                linked_id,
                Some(&denied_body),
                Some("778"),
                false
            )
            .await,
            Err(ApiError::Forbidden(_))
        ));
        assert_eq!(
            sent.load(Ordering::SeqCst),
            calls + 1,
            "作成権限がなければPATCHの404後にPOSTしない"
        );
        let original: WriteInput = serde_json::from_value(json!({"guild_id":"111","idempotency_key":"recreate","event_id":linked_id,"expected_version":patch.expected_version,"changes":patch.changes})).unwrap();
        let calls = sent.load(Ordering::SeqCst);
        let linked_updated =
            result(write(user(), patch, state.clone(), "update").await.unwrap()).await;
        assert_eq!(linked_updated["discord"], "succeeded");
        assert_eq!(linked_updated["event"]["discord_scheduled_event_id"], "777");
        assert_eq!(sent.load(Ordering::SeqCst), calls + 2);
        assert_eq!(
            result(
                write(user(), original, state.clone(), "update")
                    .await
                    .unwrap()
            )
            .await,
            linked_updated
        );
        assert_eq!(sent.load(Ordering::SeqCst), calls + 2);
        let snapshot: Value = sqlx::query_scalar(
            "SELECT payload FROM guild_webhook_outbox WHERE event_id=$1 ORDER BY id DESC LIMIT 1",
        )
        .bind(linked_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(snapshot["discord_scheduled_event_id"], "777");
        assert!(
            lookup(&pool, &user(), "111", &hash(b"recreate"))
                .await
                .unwrap()
                .unwrap()
                .requires_discord_create
        );
        mode.store(0, Ordering::SeqCst);
        let mut patch = input("unlink", json!({"discord_scheduled_event":null}));
        patch.event_id = Some(linked_id);
        patch.expected_version = linked_updated["event"]["version"]
            .as_str()
            .map(str::to_owned);
        let unlinked = result(write(user(), patch, state.clone(), "update").await.unwrap()).await;
        assert_eq!(unlinked["discord"], "succeeded");
        assert!(unlinked["event"]["discord_scheduled_event_id"].is_null());
        let mut patch = input("relink", json!({"discord_scheduled_event":true}));
        patch.event_id = Some(linked_id);
        patch.expected_version = unlinked["event"]["version"].as_str().map(str::to_owned);
        let relinked = result(write(user(), patch, state.clone(), "update").await.unwrap()).await;
        let snapshot: Value = sqlx::query_scalar(
            "SELECT payload FROM guild_webhook_outbox WHERE event_id=$1 ORDER BY id DESC LIMIT 1",
        )
        .bind(linked_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(snapshot["discord_scheduled_event_id"], "777");
        // 再作成のPOST応答が不正でも、同じキーの再送でPOSTし直さない。
        mode.store(11, Ordering::SeqCst);
        let make_patch = || {
            let mut patch = input("recreate-unknown", json!({"name":"応答不明"}));
            patch.event_id = Some(linked_id);
            patch.expected_version = relinked["event"]["version"].as_str().map(str::to_owned);
            patch
        };
        let calls = sent.load(Ordering::SeqCst);
        let unknown = result(
            write(user(), make_patch(), state.clone(), "update")
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(unknown["discord"], "unknown");
        assert_eq!(
            result(
                write(user(), make_patch(), state.clone(), "update")
                    .await
                    .unwrap()
            )
            .await,
            unknown
        );
        assert_eq!(sent.load(Ordering::SeqCst), calls + 2);
        assert!(ensure_resolved(&pool, "111", linked_id).await.is_err());
        mode.store(0, Ordering::SeqCst);
        let mut linked = changes.clone();
        linked["discord_scheduled_event"] = json!(true);
        let linked_result = result(
            write(
                user(),
                input("cleanup-source", linked),
                state.clone(),
                "create",
            )
            .await
            .unwrap(),
        )
        .await;
        let cleanup_id = linked_result["event_id"].as_i64().unwrap() as i32;
        let mut patch = input("cleanup-failed", json!({"discord_scheduled_event":null}));
        patch.event_id = Some(cleanup_id);
        patch.expected_version = linked_result["event"]["version"]
            .as_str()
            .map(str::to_owned);
        mode.store(7, Ordering::SeqCst);
        let failed = result(write(user(), patch, state.clone(), "update").await.unwrap()).await;
        assert_eq!(failed["database"], "succeeded");
        assert_eq!(failed["discord"], "failed");
        assert_eq!(failed["discord_event_id"], "777");
        assert!(failed["event"]["discord_scheduled_event_id"].is_null());
        assert!(
            crate::models::event_links::get(&pool, "111", cleanup_id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(ensure_resolved(&pool, "111", cleanup_id).await.is_err());
        mode.store(0, Ordering::SeqCst);
        // 確定拒否と不正応答を区別する。再送は外部POSTを増やさない。
        for (failure, expected) in [(7, "failed"), (8, "unknown")] {
            mode.store(failure, Ordering::SeqCst);
            let mut linked = changes.clone();
            linked["discord_scheduled_event"] = json!(true);
            let key = format!("linked-{failure}");
            let response = result(
                write(user(), input(&key, linked.clone()), state.clone(), "create")
                    .await
                    .unwrap(),
            )
            .await;
            assert_eq!(response["database"], "succeeded");
            assert_eq!(response["discord"], expected);
            let calls = sent.load(Ordering::SeqCst);
            assert_eq!(
                response,
                result(
                    write(user(), input(&key, linked), state.clone(), "create")
                        .await
                        .unwrap()
                )
                .await
            );
            assert_eq!(sent.load(Ordering::SeqCst), calls);
        }
        // リンク保存をDB側で拒否しても、判明したDiscord IDは別に永続化される。
        mode.store(0, Ordering::SeqCst);
        sqlx::raw_sql("CREATE FUNCTION reject_test_link() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test link failure'; END $$; CREATE TRIGGER reject_test_link BEFORE INSERT OR UPDATE ON event_discord_links FOR EACH ROW EXECUTE FUNCTION reject_test_link()")
            .execute(&pool).await.unwrap();
        let mut linked = changes.clone();
        linked["discord_scheduled_event"] = json!(true);
        let failed_link = result(
            write(
                user(),
                input("link-save-failed", linked.clone()),
                state.clone(),
                "create",
            )
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(failed_link["discord"], "unknown");
        assert_eq!(failed_link["discord_event_id"], "777");
        assert!(failed_link["event"]["discord_scheduled_event_id"].is_null());
        let calls = sent.load(Ordering::SeqCst);
        assert_eq!(
            result(
                write(
                    user(),
                    input("link-save-failed", linked),
                    state.clone(),
                    "create"
                )
                .await
                .unwrap()
            )
            .await,
            failed_link
        );
        assert_eq!(sent.load(Ordering::SeqCst), calls);
        // 再作成でも古いIDで新しいIDを上書きしない。
        sqlx::raw_sql("DROP TRIGGER reject_test_link ON event_discord_links")
            .execute(&pool)
            .await
            .unwrap();
        let mut linked = changes.clone();
        linked["discord_scheduled_event"] = json!(true);
        let source = result(
            write(
                user(),
                input("recreate-save-source", linked),
                state.clone(),
                "create",
            )
            .await
            .unwrap(),
        )
        .await;
        let source_id = source["event_id"].as_i64().unwrap() as i32;
        crate::models::event_links::set_scheduled_event_id(&pool, "111", source_id, "778")
            .await
            .unwrap();
        sqlx::raw_sql("CREATE TRIGGER reject_test_link BEFORE INSERT OR UPDATE ON event_discord_links FOR EACH ROW EXECUTE FUNCTION reject_test_link()")
            .execute(&pool).await.unwrap();
        let source_row = events::find_by_id(&pool, "111", source_id)
            .await
            .unwrap()
            .unwrap();
        let mut patch = input("recreate-save-failed", json!({"name":"再作成の保存失敗"}));
        patch.event_id = Some(source_id);
        patch.expected_version = Some(super::super::event_version(&source_row).unwrap());
        mode.store(10, Ordering::SeqCst);
        let failed_link =
            result(write(user(), patch, state.clone(), "update").await.unwrap()).await;
        assert_eq!(failed_link["discord"], "unknown");
        assert_eq!(failed_link["discord_event_id"], "777");
        assert_eq!(failed_link["event"]["discord_scheduled_event_id"], "778");
        let persisted: String = sqlx::query_scalar(
            "SELECT discord_event_id FROM mcp_event_operations WHERE key_hash=$1",
        )
        .bind(hash(b"recreate-save-failed"))
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(persisted, "777");
        sqlx::raw_sql("DROP TRIGGER reject_test_link ON event_discord_links; DROP FUNCTION reject_test_link()")
            .execute(&pool).await.unwrap();
        // ギルド全体の削除も、対応付けのないunknownと後始末失敗を見逃さない。
        assert!(ensure_resolved(&pool, "111", None).await.is_err());
        assert!(ensure_resolved(&pool, "222", None).await.is_ok());
        // Discord操作中にハンドラを停止する。DBとunknownが残り、再実行しない。
        mode.store(9, Ordering::SeqCst);
        let mut linked = changes.clone();
        linked["discord_scheduled_event"] = json!(true);
        let background_state = state.clone();
        let background_input = input("interrupted", linked.clone());
        let calls = sent.load(Ordering::SeqCst);
        let task = tokio::task::spawn_local(async move {
            write(user(), background_input, background_state, "create")
                .await
                .map(|_| ())
        });
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while sent.load(Ordering::SeqCst) == calls {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        task.abort();
        let _ = task.await;
        mode.store(0, Ordering::SeqCst);
        // 接続dropによるPostgres側のロック解放を待つ。
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let replayed = result(
            write(
                user(),
                input("interrupted", linked),
                state.clone(),
                "create",
            )
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(replayed["discord"], "unknown");
        assert_eq!(sent.load(Ordering::SeqCst), calls + 1);
        let unknown_id = replayed["event_id"].as_i64().unwrap() as i32;
        assert!(ensure_resolved(&pool, "111", unknown_id).await.is_err());
        assert!(ensure_resolved(&pool, "222", unknown_id).await.is_ok());
        let mut blocked = input("new-key", json!({"discord_scheduled_event":true}));
        blocked.event_id = Some(unknown_id);
        blocked.expected_version = replayed["event"]["version"].as_str().map(str::to_owned);
        assert!(matches!(
            write(user(), blocked, state.clone(), "update").await,
            Err(ApiError::Conflict(_))
        ));
        assert_eq!(sent.load(Ordering::SeqCst), calls + 1);
        let mut deletion = input("delete", json!({}));
        deletion.event_id = Some(id);
        deletion.expected_version = Some(super::super::event_version(&row).unwrap());
        assert_eq!(
            result(
                write(user(), deletion, state.clone(), "delete")
                    .await
                    .unwrap()
            )
            .await["database"],
            "succeeded"
        );
        assert!(
            events::find_by_id(&pool, "111", id)
                .await
                .unwrap()
                .is_none()
        );
        // PostgreSQLはCOMMIT済みだが、その応答フレームだけを捨てる実際の通信障害。
        let (proxy_pool, proxy) = commit_loss_proxy(&pool).await;
        let calls = sent.load(Ordering::SeqCst);
        let mut linked = changes.clone();
        linked["discord_scheduled_event"] = json!(true);
        assert!(matches!(
            write(
                user(),
                input("commit-lost", linked.clone()),
                test_state(proxy_pool.clone(), &state.site_base_url),
                "create"
            )
            .await,
            Err(ApiError::Database(_))
        ));
        assert_eq!(sent.load(Ordering::SeqCst), calls);
        let persisted = lookup(&pool, &user(), "111", &hash(b"commit-lost"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(replay(persisted).unwrap()["discord"], "unknown");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(
            result(
                write(
                    user(),
                    input("commit-lost", linked),
                    state.clone(),
                    "create"
                )
                .await
                .unwrap()
            )
            .await["discord"],
            "unknown"
        );
        assert_eq!(sent.load(Ordering::SeqCst), calls);
        proxy_pool.close().await;
        proxy.abort();
        sqlx::query("UPDATE mcp_event_operations SET created_at=now()-interval '25 hours'")
            .execute(&pool)
            .await
            .unwrap();
        purge_expired(&pool).await.unwrap();
        assert!(matches!(
            write(user(), input("create", changes), state.clone(), "create").await,
            Err(ApiError::Conflict(_))
        ));
        let retained: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM mcp_event_operations WHERE result IS NOT NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(retained, 0);
        let audit: Vec<Value> = sqlx::query_scalar(
            "SELECT detail FROM admin_audit_logs WHERE action LIKE 'mcp.event.%'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(!audit.is_empty());
        assert!(
            audit
                .iter()
                .all(|v| !v.to_string().contains("保持する説明"))
        );
        // Webが作ったシリーズをMCPで変更・中止しても、補充で元に戻らない。
        let recurring_start = (now_jst() + chrono::Duration::days(1))
            .date()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let recurring: EventInput = serde_json::from_value(json!({
            "name":"MCP定例","color":"#123456","start_at":recurring_start,
            "end_at":recurring_start + chrono::Duration::hours(1),"recurrence_rule":{"frequency":"daily","end":{"type":"count","count":3}}
        })).unwrap();
        let mut tx = pool.begin().await.unwrap();
        let event = events::create(&mut *tx, "111", &recurring, now_jst(), "333")
            .await
            .unwrap();
        crate::recurring::attach_created(&mut tx, "111", event.id, &recurring, "333")
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let mut patch = input("recurring-update", json!({"name":"MCPの個別変更"}));
        patch.event_id = Some(event.id);
        patch.expected_version = Some(super::super::event_version(&event).unwrap());
        let changed = result(write(user(), patch, state.clone(), "update").await.unwrap()).await;
        assert_eq!(changed["event"]["recurrence"]["is_exception"], true);
        let mut deletion = input("recurring-delete", json!({}));
        deletion.event_id = Some(event.id);
        deletion.expected_version = changed["event"]["version"].as_str().map(str::to_owned);
        write(user(), deletion, state.clone(), "delete")
            .await
            .unwrap();
        crate::recurring_events::ensure_range(
            &pool,
            "111",
            recurring.start_at,
            recurring.start_at + chrono::Duration::days(3),
        )
        .await
        .unwrap();
        assert!(
            events::find_by_id(&pool, "111", event.id)
                .await
                .unwrap()
                .is_none()
        );
        let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM events WHERE name='MCP定例'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining, 2);
        handle.stop(false).await;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../../rollback/20260913090000_mcp_event_operations.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let after: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, after);
    }

    /// テスト専用のPostgreSQLプロキシ。本文・資格情報を出力せずCOMMIT応答だけ切断する。
    async fn commit_loss_proxy(pool: &PgPool) -> (PgPool, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let options = pool.connect_options();
        let target = (options.get_host().to_owned(), options.get_port());
        let task = tokio::spawn(async move {
            while let Ok((client, _)) = listener.accept().await {
                let target = target.clone();
                tokio::spawn(async move {
                    let server = tokio::net::TcpStream::connect(target).await.unwrap();
                    let (mut client_read, mut client_write) = client.into_split();
                    let (mut server_read, mut server_write) = server.into_split();
                    let responses = async {
                        loop {
                            let tag = server_read.read_u8().await?;
                            let length = server_read.read_u32().await?;
                            let mut body = vec![0; (length - 4) as usize];
                            server_read.read_exact(&mut body).await?;
                            if tag == b'C' && body == b"COMMIT\0" {
                                return Ok::<(), std::io::Error>(());
                            }
                            client_write.write_u8(tag).await?;
                            client_write.write_u32(length).await?;
                            client_write.write_all(&body).await?;
                        }
                    };
                    tokio::select! {
                        _ = tokio::io::copy(&mut client_read, &mut server_write) => {},
                        _ = responses => {},
                    }
                });
            }
        });
        let options = options
            .as_ref()
            .clone()
            .host("127.0.0.1")
            .port(port)
            .ssl_mode(sqlx::postgres::PgSslMode::Disable);
        (
            sqlx::postgres::PgPoolOptions::new()
                .connect_with(options)
                .await
                .unwrap(),
            task,
        )
    }

    #[test]
    fn distinguishes_rejections_from_unknown_effects() {
        use crate::discord::DiscordError;
        assert_eq!(side_effect_status(&Ok(())), "succeeded");
        assert_eq!(side_effect_status(&Err(ApiError::RateLimited)), "failed");
        for (status, expected) in [
            (reqwest::StatusCode::FORBIDDEN, "failed"),
            (reqwest::StatusCode::INTERNAL_SERVER_ERROR, "unknown"),
        ] {
            assert_eq!(
                side_effect_status(&Err(ApiError::Discord(DiscordError::Status {
                    status,
                    body: String::new()
                }))),
                expected
            );
        }
        assert_eq!(
            side_effect_status(&Err(ApiError::Discord(DiscordError::Unexpected(
                "response lost"
            )))),
            "unknown"
        );
    }
}
