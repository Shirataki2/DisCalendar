//! サーバー用 API トークン (#405) を使った外部公開エンドポイント (`/hooks/guilds/{guild_id}/events`)。
//!
//! 認証は `Authorization: Bearer <token>`。web の `/hooks/*` リライト経由で公開する。
//! 入力の検証・通知・Webhook・Discord イベント連携は通常の予定 API (`routes/events.rs`) と揃える。

use actix_web::{
    HttpRequest, HttpResponse, ResponseError as _, delete,
    http::{StatusCode, header::AUTHORIZATION},
    post, put, web,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use utoipa::{IntoParams, ToSchema};

use super::events::{
    UPDATE_SYNC_ATTEMPTS, cleanup_scheduled_event, cleanup_unsaved_scheduled_event, cover_image,
    describe_create_error, describe_scheduled_event_error, payload_for,
    undo_scheduled_event_changes, with_befores,
};
use crate::{
    attachments::AttachmentStorage,
    discord::is_snowflake,
    error::{ApiError, ErrorBody},
    event_history::{self, Action, Source},
    models::{
        api_tokens::{
            self, AUTH_FAILURE_LIMIT_PER_MINUTE, AuthenticatedToken, RATE_LIMIT_PER_MINUTE,
        },
        event_links,
        events::{self, Event, EventInput},
        notification_mentions, now_jst,
    },
    recurring::{self, ChangeScope, DeleteOptions},
    recurring_events,
    state::AppState,
};

#[derive(Debug, Deserialize, IntoParams)]
pub struct HookGuildPath {
    /// ギルド ID
    pub guild_id: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct HookEventPath {
    /// ギルド ID
    pub guild_id: String,
    /// 予定 ID
    pub event_id: i32,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct HookExternalPath {
    /// ギルド ID
    pub guild_id: String,
    /// 外部システム側の ID (`1..=128` 文字、`[A-Za-z0-9._:-]`)
    pub external_id: String,
}

/// API トークン経由での予定作成入力 (`EventInput` + 任意の `external_id`)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HookEventInput {
    #[serde(flatten)]
    pub event: EventInput,
    /// 外部システム側で一意な ID (あとから同じ ID で更新・削除するときに指定)
    #[serde(default)]
    #[schema(example = "yt:video-123", max_length = 128)]
    pub external_id: Option<String>,
}

/// API トークン経由の予定レスポンス
#[derive(Debug, Serialize, ToSchema)]
pub struct HookEvent {
    #[serde(flatten)]
    pub event: Event,
    /// 紐づけられた外部 ID (未設定なら null)
    #[schema(example = "yt:video-123")]
    pub external_id: Option<String>,
}

struct IdempotencyMeta {
    key_hash: String,
    request_hash: String,
}

fn client_ip(req: &HttpRequest) -> String {
    if let Some(ip) = req
        .headers()
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        return ip.to_owned();
    }
    req.connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_owned()
}

fn extract_bearer_token(req: &HttpRequest) -> Option<&str> {
    let header = req.headers().get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = header.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    (!token.is_empty()).then_some(token)
}

fn extract_idempotency(
    req: &HttpRequest,
    method: &str,
    action: &str,
    event_id: Option<i32>,
    external_id: Option<&str>,
    body: &Value,
) -> Result<Option<IdempotencyMeta>, ApiError> {
    let Some(header) = req.headers().get("Idempotency-Key") else {
        return Ok(None);
    };
    let raw = header
        .to_str()
        .map_err(|_| ApiError::BadRequest("Idempotency-Key must be ASCII".into()))?;
    let key_hash = api_tokens::validate_and_hash_idempotency_key(raw)?;
    let fingerprint = serde_json::to_vec(&json!({
        "method": method,
        "action": action,
        "event_id": event_id,
        "external_id": external_id,
        "body": body,
    }))
    .map_err(anyhow::Error::from)?;
    Ok(Some(IdempotencyMeta {
        key_hash,
        request_hash: api_tokens::hash_hex(&fingerprint),
    }))
}

#[allow(clippy::too_many_arguments)]
async fn authenticate(
    req: &HttpRequest,
    state: &AppState,
    guild_id: &str,
    requires_write: bool,
    method: &str,
    action: &str,
    event_id: Option<i32>,
    external_id: Option<&str>,
) -> Result<AuthenticatedToken, ApiError> {
    if !is_snowflake(guild_id) {
        return Err(ApiError::BadRequest("guild_id must be a snowflake".into()));
    }
    let token = match extract_bearer_token(req) {
        Some(raw) => api_tokens::authenticate(&state.pool, guild_id, raw).await?,
        None => None,
    };
    let Some(token) = token else {
        let failure_key = format!("fail:{guild_id}:{}", client_ip(req));
        if !state
            .reserve_api_token_rate_limit(&failure_key, AUTH_FAILURE_LIMIT_PER_MINUTE)
            .await
        {
            return Err(ApiError::TooManyRequests(
                "認証の失敗が多すぎます。1分ほど待ってから再試行してください".into(),
            ));
        }
        return Err(ApiError::Unauthorized);
    };

    let rate_key = format!("token:{}", token.id);
    if !state
        .reserve_api_token_rate_limit(&rate_key, RATE_LIMIT_PER_MINUTE)
        .await
    {
        let err = ApiError::TooManyRequests(
            "API トークンの呼び出し上限に達しました。1分ほど待ってから再試行してください".into(),
        );
        log_token_call(
            state,
            &token,
            method,
            action,
            event_id,
            external_id,
            StatusCode::TOO_MANY_REQUESTS.as_u16(),
            Some(err.kind()),
        )
        .await;
        return Err(err);
    }

    if requires_write && !token.scope.can_write() {
        let err = ApiError::Forbidden("このトークンには予定の更新・削除権限がありません".into());
        log_token_call(
            state,
            &token,
            method,
            action,
            event_id,
            external_id,
            StatusCode::FORBIDDEN.as_u16(),
            Some(err.kind()),
        )
        .await;
        return Err(err);
    }

    Ok(token)
}

#[allow(clippy::too_many_arguments)]
async fn log_token_call(
    state: &AppState,
    token: &AuthenticatedToken,
    method: &str,
    action: &str,
    event_id: Option<i32>,
    external_id: Option<&str>,
    status: u16,
    error_kind: Option<&str>,
) {
    if let Err(err) = api_tokens::record_call(
        &state.pool,
        token.id,
        &token.guild_id,
        method,
        action,
        event_id,
        external_id,
        status,
        error_kind,
    )
    .await
    {
        tracing::warn!(
            token_id = token.id,
            guild_id = %token.guild_id,
            error = %err,
            "failed to record api token call log"
        );
    }
}

fn no_store_response(status: StatusCode, body: Option<Value>, replayed: bool) -> HttpResponse {
    let mut builder = HttpResponse::build(status);
    builder.insert_header(("Cache-Control", "no-store"));
    if replayed {
        builder.insert_header(("Idempotency-Replayed", "true"));
    }
    match body {
        Some(value) => builder.json(value),
        None => builder.finish(),
    }
}

async fn check_idempotency(
    state: &AppState,
    token: &AuthenticatedToken,
    meta: Option<&IdempotencyMeta>,
) -> Result<Option<HttpResponse>, ApiError> {
    let Some(meta) = meta else {
        return Ok(None);
    };
    let Some(saved) =
        api_tokens::lookup_operation(&state.pool, &token.guild_id, token.id, &meta.key_hash)
            .await?
    else {
        return Ok(None);
    };
    if saved.request_hash != meta.request_hash {
        return Err(ApiError::Conflict(
            "Idempotency-Key was used with different content".into(),
        ));
    }
    if saved.expired {
        return Err(ApiError::Conflict(
            "Idempotency-Key expired; do not reuse this key".into(),
        ));
    }
    let status = StatusCode::from_u16(saved.status as u16)
        .map_err(|e| anyhow::anyhow!("invalid saved status code: {e}"))?;
    if status != StatusCode::NO_CONTENT && saved.response_body.is_none() {
        return Err(ApiError::Conflict(
            "operation result is unavailable; do not reuse this key".into(),
        ));
    }
    Ok(Some(no_store_response(status, saved.response_body, true)))
}

async fn validate_mentions_for_token(
    state: &AppState,
    guild_id: &str,
    token_id: i64,
    mentions: Option<&[notification_mentions::NotificationMention]>,
) -> Result<(), ApiError> {
    let Some(mentions) = mentions.filter(|m| !m.is_empty()) else {
        return Ok(());
    };
    let user_count = mentions
        .iter()
        .filter(|m| matches!(m, notification_mentions::NotificationMention::User { .. }))
        .count() as u32;
    if user_count > 0
        && !state
            .discord
            .reserve_mention_lookups(&format!("token:{token_id}"), user_count)
            .await
    {
        return Err(ApiError::TooManyRequests(
            "メンバーの確認が多すぎます。1分ほど待ってから再試行してください".into(),
        ));
    }
    notification_mentions::validate_targets(&state.discord, guild_id, None, Some(mentions)).await
}

fn webhook_actor_id(token: &AuthenticatedToken) -> String {
    format!("api_token:{}", token.id)
}

async fn execute_create(
    state: &AppState,
    token: &AuthenticatedToken,
    body: &EventInput,
    external_id: Option<&str>,
    idempotency: Option<&IdempotencyMeta>,
) -> Result<HookEvent, ApiError> {
    body.validate()?;
    if let Some(ext) = external_id {
        api_tokens::validate_external_id(ext)?;
        let mut conn = state.pool.acquire().await?;
        if api_tokens::find_event_id_by_external_id(&mut conn, &token.guild_id, ext)
            .await?
            .is_some()
        {
            return Err(ApiError::Conflict(
                "external_id is already assigned to another event".into(),
            ));
        }
    }
    validate_mentions_for_token(
        state,
        &token.guild_id,
        token.id,
        body.notification_mentions.as_deref(),
    )
    .await?;

    let discord_scheduled_event = body.discord_scheduled_event.unwrap_or(false);
    events::validate_discord_flag(body, discord_scheduled_event, now_jst())?;

    let guild_id = token.guild_id.as_str();
    let actor_id = webhook_actor_id(token);

    if !discord_scheduled_event {
        let mut tx = state.pool.begin().await?;
        let row = events::create_with_actor(&mut *tx, guild_id, body, now_jst(), None).await?;
        recurring::attach_created_with_actor(&mut tx, guild_id, row.id, body, None).await?;
        if let Some(ext) = external_id {
            api_tokens::link_external_id(&mut tx, guild_id, ext, row.id).await?;
        }
        event_history::record_with_label(
            &mut tx,
            guild_id,
            row.id,
            None,
            Some(&token.name),
            Source::ApiToken,
            Action::Create,
            None,
        )
        .await?;
        let event = recurring::decorate(&mut tx, Event::from(row)).await?;
        crate::webhook_outbox::enqueue(&mut tx, guild_id, event.id, "event.created", &actor_id)
            .await?;
        let response = HookEvent {
            event,
            external_id: external_id.map(str::to_owned),
        };
        if let Some(meta) = idempotency {
            let value = serde_json::to_value(&response).map_err(anyhow::Error::from)?;
            api_tokens::save_operation(
                &mut *tx,
                guild_id,
                token.id,
                &meta.key_hash,
                &meta.request_hash,
                StatusCode::CREATED.as_u16(),
                Some(response.event.id),
                Some(&value),
            )
            .await?;
        }
        tx.commit().await?;
        return Ok(response);
    }

    let scheduled_event_id = state
        .discord
        .create_scheduled_event(guild_id, &payload_for(&state.site_base_url, guild_id, body))
        .await
        .map_err(|err| describe_create_error(guild_id, &body.name, err))?;

    let result: Result<HookEvent, ApiError> = async {
        let mut tx = state.pool.begin().await?;
        event_links::lock_guild(&mut *tx, guild_id).await?;
        let mut row = events::create_with_actor(&mut *tx, guild_id, body, now_jst(), None).await?;
        event_links::insert(&mut *tx, guild_id, row.id, &scheduled_event_id, now_jst()).await?;
        if let Some(ext) = external_id {
            api_tokens::link_external_id(&mut tx, guild_id, ext, row.id).await?;
        }
        event_history::record_with_label(
            &mut tx,
            guild_id,
            row.id,
            None,
            Some(&token.name),
            Source::ApiToken,
            Action::Create,
            None,
        )
        .await?;
        crate::webhook_outbox::enqueue(&mut tx, guild_id, row.id, "event.created", &actor_id)
            .await?;
        row.discord_scheduled_event_id = Some(scheduled_event_id.clone());
        let response = HookEvent {
            event: Event::from(row),
            external_id: external_id.map(str::to_owned),
        };
        if let Some(meta) = idempotency {
            let value = serde_json::to_value(&response).map_err(anyhow::Error::from)?;
            api_tokens::save_operation(
                &mut *tx,
                guild_id,
                token.id,
                &meta.key_hash,
                &meta.request_hash,
                StatusCode::CREATED.as_u16(),
                Some(response.event.id),
                Some(&value),
            )
            .await?;
        }
        tx.commit().await?;
        Ok(response)
    }
    .await;

    match result {
        Ok(response) => Ok(response),
        Err(err) => {
            cleanup_unsaved_scheduled_event(state, guild_id, &scheduled_event_id).await;
            Err(err)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_update(
    state: &AppState,
    storage: Option<&AttachmentStorage>,
    token: &AuthenticatedToken,
    event_id: i32,
    input: &EventInput,
    idempotency: Option<&IdempotencyMeta>,
) -> Result<HookEvent, ApiError> {
    let guild_id = token.guild_id.as_str();
    crate::mcp::ensure_resolved(&state.pool, guild_id, event_id).await?;
    let _event_lock = state.event_update_locks.lock(event_id).await;
    let actor_id = webhook_actor_id(token);

    for _ in 0..UPDATE_SYNC_ATTEMPTS {
        let old = events::find_by_id(&state.pool, guild_id, event_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
        input.validate_update(&old.notifications)?;
        let mut body = input.clone();
        body.notification_mentions.get_or_insert_with(|| {
            serde_json::from_value(old.notification_mentions.clone()).unwrap_or_default()
        });
        validate_mentions_for_token(
            state,
            guild_id,
            token.id,
            body.notification_mentions.as_deref(),
        )
        .await?;

        {
            let mut tx = state.pool.begin().await?;
            let previous_series = recurring_events::info(&mut tx, guild_id, event_id).await?;
            let affected = recurring::affected_ids(&mut tx, guild_id, event_id, body.scope).await?;
            let befores = event_history::snapshots(&mut tx, guild_id, &affected).await?;
            if let Some(row) =
                recurring::update_with_actor(&mut tx, guild_id, event_id, &body, None).await?
            {
                event_history::record_many_with_label(
                    &mut tx,
                    guild_id,
                    &with_befores(befores),
                    None,
                    Some(&token.name),
                    Source::ApiToken,
                    Action::Update,
                )
                .await?;
                crate::webhook_outbox::enqueue_with_scope(
                    &mut tx,
                    guild_id,
                    row.id,
                    "event.updated",
                    &actor_id,
                    Some(if body.scope == ChangeScope::Future {
                        "future"
                    } else {
                        "this"
                    }),
                    previous_series.as_ref(),
                )
                .await?;
                let external_id =
                    api_tokens::find_external_id_by_event_id(&mut tx, guild_id, row.id).await?;
                let event = recurring::decorate(&mut tx, Event::from(row)).await?;
                let response = HookEvent { event, external_id };
                if let Some(meta) = idempotency {
                    let value = serde_json::to_value(&response).map_err(anyhow::Error::from)?;
                    api_tokens::save_operation(
                        &mut *tx,
                        guild_id,
                        token.id,
                        &meta.key_hash,
                        &meta.request_hash,
                        StatusCode::OK.as_u16(),
                        Some(response.event.id),
                        Some(&value),
                    )
                    .await?;
                }
                tx.commit().await?;
                return Ok(response);
            }
        }

        let discord_scheduled_event = body
            .discord_scheduled_event
            .unwrap_or(old.discord_scheduled_event_id.is_some());
        events::validate_discord_flag(&body, discord_scheduled_event, now_jst())?;

        if old.discord_scheduled_event_id.is_none() && !discord_scheduled_event {
            let mut tx = state.pool.begin().await?;
            let before = event_history::snapshot(&mut tx, guild_id, event_id).await?;
            if let Some(row) = events::update_if_unlinked_with_actor(
                &mut *tx,
                guild_id,
                event_id,
                &body,
                None,
                now_jst(),
            )
            .await?
            {
                event_history::record_with_label(
                    &mut tx,
                    guild_id,
                    row.id,
                    None,
                    Some(&token.name),
                    Source::ApiToken,
                    Action::Update,
                    before.as_ref(),
                )
                .await?;
                crate::webhook_outbox::enqueue(
                    &mut tx,
                    guild_id,
                    row.id,
                    "event.updated",
                    &actor_id,
                )
                .await?;
                let external_id =
                    api_tokens::find_external_id_by_event_id(&mut tx, guild_id, row.id).await?;
                let response = HookEvent {
                    event: Event::from(row),
                    external_id,
                };
                if let Some(meta) = idempotency {
                    let value = serde_json::to_value(&response).map_err(anyhow::Error::from)?;
                    api_tokens::save_operation(
                        &mut *tx,
                        guild_id,
                        token.id,
                        &meta.key_hash,
                        &meta.request_hash,
                        StatusCode::OK.as_u16(),
                        Some(response.event.id),
                        Some(&value),
                    )
                    .await?;
                }
                tx.commit().await?;
                return Ok(response);
            }
            tx.rollback().await?;
            if events::find_by_id(&state.pool, guild_id, event_id)
                .await?
                .is_none()
            {
                return Err(ApiError::NotFound("event not found".into()));
            }
            continue;
        }

        let (desired, created_now): (Option<String>, Option<String>) =
            match (&old.discord_scheduled_event_id, discord_scheduled_event) {
                (None, true) => {
                    let mut payload = payload_for(&state.site_base_url, guild_id, &body);
                    if let Some(storage) = storage {
                        payload = payload
                            .with_image(cover_image(state, storage, guild_id, event_id).await);
                    }
                    let id = state
                        .discord
                        .create_scheduled_event(guild_id, &payload)
                        .await
                        .map_err(|err| describe_create_error(guild_id, &body.name, err))?;
                    (Some(id.clone()), Some(id))
                }
                (Some(current), true) => {
                    let payload = payload_for(&state.site_base_url, guild_id, &body);
                    let modified = match state
                        .discord
                        .modify_scheduled_event(guild_id, current, &payload)
                        .await
                    {
                        Ok(modified) => modified,
                        Err(err) => {
                            undo_scheduled_event_changes(
                                state,
                                guild_id,
                                event_id,
                                &body,
                                &None,
                                &Some(current.clone()),
                                &old,
                            )
                            .await;
                            return Err(describe_scheduled_event_error(err));
                        }
                    };
                    if modified {
                        (Some(current.clone()), None)
                    } else {
                        let mut payload = payload;
                        if let Some(storage) = storage {
                            payload = payload
                                .with_image(cover_image(state, storage, guild_id, event_id).await);
                        }
                        let id = state
                            .discord
                            .create_scheduled_event(guild_id, &payload)
                            .await
                            .map_err(|err| describe_create_error(guild_id, &body.name, err))?;
                        (Some(id.clone()), Some(id))
                    }
                }
                (Some(_), false) => (None, None),
                (None, false) => unreachable!("handled above"),
            };

        let tx_result: Result<Option<(HookEvent, Option<String>)>, ApiError> = async {
            let mut tx = state.pool.begin().await?;
            event_links::lock_guild(&mut *tx, guild_id).await?;
            let current = events::find_by_id_for_update(&mut tx, guild_id, event_id)
                .await?
                .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
            if current.discord_scheduled_event_id != old.discord_scheduled_event_id {
                return Ok(None);
            }
            let before = event_history::snapshot(&mut tx, guild_id, event_id).await?;
            let mut row =
                events::update_with_actor(&mut *tx, guild_id, event_id, &body, None, now_jst())
                    .await?
                    .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
            let mut replaced: Option<String> = None;
            match (&current.discord_scheduled_event_id, &desired) {
                (None, Some(sid)) => {
                    event_links::insert(&mut *tx, guild_id, row.id, sid, now_jst()).await?;
                }
                (Some(existing), Some(sid)) if existing != sid => {
                    event_links::set_scheduled_event_id(&mut *tx, guild_id, row.id, sid).await?;
                    replaced = Some(existing.clone());
                }
                (Some(existing), None) => {
                    event_links::delete(&mut *tx, guild_id, row.id).await?;
                    replaced = Some(existing.clone());
                }
                _ => {}
            }
            event_history::record_with_label(
                &mut tx,
                guild_id,
                row.id,
                None,
                Some(&token.name),
                Source::ApiToken,
                Action::Update,
                before.as_ref(),
            )
            .await?;
            crate::webhook_outbox::enqueue(&mut tx, guild_id, row.id, "event.updated", &actor_id)
                .await?;
            row.discord_scheduled_event_id = desired.clone();
            let external_id =
                api_tokens::find_external_id_by_event_id(&mut tx, guild_id, row.id).await?;
            let response = HookEvent {
                event: Event::from(row),
                external_id,
            };
            if let Some(meta) = idempotency {
                let value = serde_json::to_value(&response).map_err(anyhow::Error::from)?;
                api_tokens::save_operation(
                    &mut *tx,
                    guild_id,
                    token.id,
                    &meta.key_hash,
                    &meta.request_hash,
                    StatusCode::OK.as_u16(),
                    Some(response.event.id),
                    Some(&value),
                )
                .await?;
            }
            tx.commit().await?;
            Ok(Some((response, replaced)))
        }
        .await;

        match tx_result {
            Ok(Some((response, replaced))) => {
                if let Some(sid) = &replaced {
                    cleanup_scheduled_event(&state.discord, guild_id, sid).await;
                }
                return Ok(response);
            }
            Ok(None) => {
                undo_scheduled_event_changes(
                    state,
                    guild_id,
                    event_id,
                    &body,
                    &created_now,
                    &desired,
                    &old,
                )
                .await;
            }
            Err(err) => {
                undo_scheduled_event_changes(
                    state,
                    guild_id,
                    event_id,
                    &body,
                    &created_now,
                    &desired,
                    &old,
                )
                .await;
                return Err(err);
            }
        }
    }

    Err(ApiError::Conflict(
        "the event was updated concurrently, please retry".into(),
    ))
}

async fn execute_delete(
    state: &AppState,
    token: &AuthenticatedToken,
    event_id: i32,
    options: &DeleteOptions,
    idempotency: Option<&IdempotencyMeta>,
) -> Result<(), ApiError> {
    let guild_id = token.guild_id.as_str();
    crate::mcp::ensure_resolved(&state.pool, guild_id, event_id).await?;
    let actor_id = webhook_actor_id(token);

    let mut tx = state.pool.begin().await?;
    let row = events::find_by_id_for_update(&mut tx, guild_id, event_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    let before = event_history::snapshot(&mut tx, guild_id, event_id).await?;
    crate::webhook_outbox::enqueue_with_scope(
        &mut tx,
        guild_id,
        event_id,
        "event.deleted",
        &actor_id,
        Some(if options.scope == ChangeScope::Future {
            "future"
        } else {
            "this"
        }),
        None,
    )
    .await?;

    if recurring::before_delete(&mut tx, guild_id, event_id, options).await? {
        events::delete_permanently(&mut *tx, guild_id, event_id).await?;
    } else {
        events::soft_delete_with_actor(&mut tx, guild_id, event_id, None, now_jst()).await?;
        api_tokens::unlink_external_id_by_event_id(&mut tx, guild_id, event_id).await?;
        event_history::record_with_label(
            &mut tx,
            guild_id,
            event_id,
            None,
            Some(&token.name),
            Source::ApiToken,
            Action::Delete,
            before.as_ref(),
        )
        .await?;
    }
    if let Some(meta) = idempotency {
        api_tokens::save_operation(
            &mut *tx,
            guild_id,
            token.id,
            &meta.key_hash,
            &meta.request_hash,
            StatusCode::NO_CONTENT.as_u16(),
            Some(event_id),
            None,
        )
        .await?;
    }
    if let Err(err) = tx.commit().await {
        if let Some(scheduled_event_id) = &row.discord_scheduled_event_id
            && matches!(
                events::find_by_id(&state.pool, guild_id, event_id).await,
                Ok(None)
            )
        {
            cleanup_scheduled_event(&state.discord, guild_id, scheduled_event_id).await;
        }
        return Err(err.into());
    }
    if let Some(scheduled_event_id) = &row.discord_scheduled_event_id {
        cleanup_scheduled_event(&state.discord, guild_id, scheduled_event_id).await;
    }
    Ok(())
}

/// API トークンで予定を作成する
#[utoipa::path(
    tag = "hooks",
    security(("api_token" = [])),
    params(
        HookGuildPath,
        ("Idempotency-Key" = Option<String>, Header, description = "再送時の重複作成を防ぐ冪等キー (1〜128 文字)")
    ),
    request_body = HookEventInput,
    responses(
        (status = 201, body = HookEvent),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 409, body = ErrorBody),
        (status = 429, body = ErrorBody),
    )
)]
#[post("/guilds/{guild_id}/events")]
pub async fn create_event(
    req: HttpRequest,
    path: web::Path<HookGuildPath>,
    body: web::Json<HookEventInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let external_id = body.external_id.as_deref();
    let token = authenticate(
        &req,
        &state,
        &path.guild_id,
        false,
        "POST",
        "create",
        None,
        external_id,
    )
    .await?;

    let body_json = serde_json::to_value(&*body).map_err(anyhow::Error::from)?;
    let idempotency =
        match extract_idempotency(&req, "POST", "create", None, external_id, &body_json) {
            Ok(meta) => meta,
            Err(err) => {
                log_token_call(
                    &state,
                    &token,
                    "POST",
                    "create",
                    None,
                    external_id,
                    err.status_code().as_u16(),
                    Some(err.kind()),
                )
                .await;
                return Err(err);
            }
        };

    let _writer = event_links::lock_writer(&state.pool, &token.guild_id).await?;
    match check_idempotency(&state, &token, idempotency.as_ref()).await {
        Ok(Some(replayed)) => {
            log_token_call(
                &state,
                &token,
                "POST",
                "create",
                None,
                external_id,
                replayed.status().as_u16(),
                None,
            )
            .await;
            return Ok(replayed);
        }
        Ok(None) => {}
        Err(err) => {
            log_token_call(
                &state,
                &token,
                "POST",
                "create",
                None,
                external_id,
                err.status_code().as_u16(),
                Some(err.kind()),
            )
            .await;
            return Err(err);
        }
    }

    match execute_create(
        &state,
        &token,
        &body.event,
        external_id,
        idempotency.as_ref(),
    )
    .await
    {
        Ok(created) => {
            log_token_call(
                &state,
                &token,
                "POST",
                "create",
                Some(created.event.id),
                external_id,
                StatusCode::CREATED.as_u16(),
                None,
            )
            .await;
            let value = serde_json::to_value(&created).map_err(anyhow::Error::from)?;
            Ok(no_store_response(StatusCode::CREATED, Some(value), false))
        }
        Err(err) => {
            log_token_call(
                &state,
                &token,
                "POST",
                "create",
                None,
                external_id,
                err.status_code().as_u16(),
                Some(err.kind()),
            )
            .await;
            Err(err)
        }
    }
}

/// 外部 ID を指定して予定を作成または更新 (Upsert) する
#[utoipa::path(
    tag = "hooks",
    security(("api_token" = [])),
    params(
        HookExternalPath,
        ("Idempotency-Key" = Option<String>, Header, description = "再送時の重複を防ぐ冪等キー (1〜128 文字)")
    ),
    request_body = EventInput,
    responses(
        (status = 200, description = "既存の予定を更新", body = HookEvent),
        (status = 201, description = "新しい予定を作成", body = HookEvent),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 409, body = ErrorBody),
        (status = 429, body = ErrorBody),
    )
)]
#[put("/guilds/{guild_id}/events/external/{external_id}")]
pub async fn upsert_by_external_id(
    req: HttpRequest,
    path: web::Path<HookExternalPath>,
    body: web::Json<EventInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let external_id = path.external_id.as_str();
    let token = authenticate(
        &req,
        &state,
        &path.guild_id,
        true,
        "PUT",
        "upsert_external",
        None,
        Some(external_id),
    )
    .await?;

    if let Err(err) = api_tokens::validate_external_id(external_id) {
        log_token_call(
            &state,
            &token,
            "PUT",
            "upsert_external",
            None,
            Some(external_id),
            err.status_code().as_u16(),
            Some(err.kind()),
        )
        .await;
        return Err(err);
    }

    let body_json = serde_json::to_value(&*body).map_err(anyhow::Error::from)?;
    let idempotency = match extract_idempotency(
        &req,
        "PUT",
        "upsert_external",
        None,
        Some(external_id),
        &body_json,
    ) {
        Ok(meta) => meta,
        Err(err) => {
            log_token_call(
                &state,
                &token,
                "PUT",
                "upsert_external",
                None,
                Some(external_id),
                err.status_code().as_u16(),
                Some(err.kind()),
            )
            .await;
            return Err(err);
        }
    };

    let _writer = event_links::lock_writer(&state.pool, &token.guild_id).await?;
    match check_idempotency(&state, &token, idempotency.as_ref()).await {
        Ok(Some(replayed)) => {
            log_token_call(
                &state,
                &token,
                "PUT",
                "upsert_external",
                None,
                Some(external_id),
                replayed.status().as_u16(),
                None,
            )
            .await;
            return Ok(replayed);
        }
        Ok(None) => {}
        Err(err) => {
            log_token_call(
                &state,
                &token,
                "PUT",
                "upsert_external",
                None,
                Some(external_id),
                err.status_code().as_u16(),
                Some(err.kind()),
            )
            .await;
            return Err(err);
        }
    }

    let existing_id = {
        let mut conn = state.pool.acquire().await?;
        api_tokens::find_event_id_by_external_id(&mut conn, &token.guild_id, external_id).await?
    };

    let storage = req
        .app_data::<web::Data<AttachmentStorage>>()
        .map(|d| d.as_ref());
    let (status, result) = match existing_id {
        Some(event_id) => (
            StatusCode::OK,
            execute_update(
                &state,
                storage,
                &token,
                event_id,
                &body,
                idempotency.as_ref(),
            )
            .await,
        ),
        None => (
            StatusCode::CREATED,
            execute_create(
                &state,
                &token,
                &body,
                Some(external_id),
                idempotency.as_ref(),
            )
            .await,
        ),
    };

    match result {
        Ok(saved) => {
            log_token_call(
                &state,
                &token,
                "PUT",
                "upsert_external",
                Some(saved.event.id),
                Some(external_id),
                status.as_u16(),
                None,
            )
            .await;
            let value = serde_json::to_value(&saved).map_err(anyhow::Error::from)?;
            Ok(no_store_response(status, Some(value), false))
        }
        Err(err) => {
            log_token_call(
                &state,
                &token,
                "PUT",
                "upsert_external",
                existing_id,
                Some(external_id),
                err.status_code().as_u16(),
                Some(err.kind()),
            )
            .await;
            Err(err)
        }
    }
}

/// 外部 ID を指定して予定を削除する (ゴミ箱へ移動し、外部 ID の紐付けも解除する)
#[utoipa::path(
    tag = "hooks",
    security(("api_token" = [])),
    params(
        HookExternalPath,
        DeleteOptions,
        ("Idempotency-Key" = Option<String>, Header, description = "再送時の冪等キー (1〜128 文字)")
    ),
    responses(
        (status = 204),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
        (status = 429, body = ErrorBody),
    )
)]
#[delete("/guilds/{guild_id}/events/external/{external_id}")]
pub async fn delete_by_external_id(
    req: HttpRequest,
    path: web::Path<HookExternalPath>,
    options: web::Query<DeleteOptions>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let external_id = path.external_id.as_str();
    let token = authenticate(
        &req,
        &state,
        &path.guild_id,
        true,
        "DELETE",
        "delete_external",
        None,
        Some(external_id),
    )
    .await?;

    if let Err(err) = api_tokens::validate_external_id(external_id) {
        log_token_call(
            &state,
            &token,
            "DELETE",
            "delete_external",
            None,
            Some(external_id),
            err.status_code().as_u16(),
            Some(err.kind()),
        )
        .await;
        return Err(err);
    }

    let body_json = json!({
        "scope": options.scope,
        "expected_series_version": options.expected_series_version,
    });
    let idempotency = match extract_idempotency(
        &req,
        "DELETE",
        "delete_external",
        None,
        Some(external_id),
        &body_json,
    ) {
        Ok(meta) => meta,
        Err(err) => {
            log_token_call(
                &state,
                &token,
                "DELETE",
                "delete_external",
                None,
                Some(external_id),
                err.status_code().as_u16(),
                Some(err.kind()),
            )
            .await;
            return Err(err);
        }
    };

    let _writer = event_links::lock_writer(&state.pool, &token.guild_id).await?;
    match check_idempotency(&state, &token, idempotency.as_ref()).await {
        Ok(Some(replayed)) => {
            log_token_call(
                &state,
                &token,
                "DELETE",
                "delete_external",
                None,
                Some(external_id),
                replayed.status().as_u16(),
                None,
            )
            .await;
            return Ok(replayed);
        }
        Ok(None) => {}
        Err(err) => {
            log_token_call(
                &state,
                &token,
                "DELETE",
                "delete_external",
                None,
                Some(external_id),
                err.status_code().as_u16(),
                Some(err.kind()),
            )
            .await;
            return Err(err);
        }
    }

    let event_id = {
        let mut conn = state.pool.acquire().await?;
        api_tokens::find_event_id_by_external_id(&mut conn, &token.guild_id, external_id).await?
    };
    let Some(event_id) = event_id else {
        let err = ApiError::NotFound("event not found".into());
        log_token_call(
            &state,
            &token,
            "DELETE",
            "delete_external",
            None,
            Some(external_id),
            err.status_code().as_u16(),
            Some(err.kind()),
        )
        .await;
        return Err(err);
    };

    match execute_delete(&state, &token, event_id, &options, idempotency.as_ref()).await {
        Ok(()) => {
            log_token_call(
                &state,
                &token,
                "DELETE",
                "delete_external",
                Some(event_id),
                Some(external_id),
                StatusCode::NO_CONTENT.as_u16(),
                None,
            )
            .await;
            Ok(no_store_response(StatusCode::NO_CONTENT, None, false))
        }
        Err(err) => {
            log_token_call(
                &state,
                &token,
                "DELETE",
                "delete_external",
                Some(event_id),
                Some(external_id),
                err.status_code().as_u16(),
                Some(err.kind()),
            )
            .await;
            Err(err)
        }
    }
}

/// 予定 ID を指定して予定を更新する
#[utoipa::path(
    tag = "hooks",
    security(("api_token" = [])),
    params(
        HookEventPath,
        ("Idempotency-Key" = Option<String>, Header, description = "再送時の冪等キー (1〜128 文字)")
    ),
    request_body = EventInput,
    responses(
        (status = 200, body = HookEvent),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
        (status = 429, body = ErrorBody),
    )
)]
#[put("/guilds/{guild_id}/events/{event_id}")]
pub async fn update_event(
    req: HttpRequest,
    path: web::Path<HookEventPath>,
    body: web::Json<EventInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let event_id = path.event_id;
    let token = authenticate(
        &req,
        &state,
        &path.guild_id,
        true,
        "PUT",
        "update",
        Some(event_id),
        None,
    )
    .await?;

    let body_json = serde_json::to_value(&*body).map_err(anyhow::Error::from)?;
    let idempotency =
        match extract_idempotency(&req, "PUT", "update", Some(event_id), None, &body_json) {
            Ok(meta) => meta,
            Err(err) => {
                log_token_call(
                    &state,
                    &token,
                    "PUT",
                    "update",
                    Some(event_id),
                    None,
                    err.status_code().as_u16(),
                    Some(err.kind()),
                )
                .await;
                return Err(err);
            }
        };

    let _writer = event_links::lock_writer(&state.pool, &token.guild_id).await?;
    match check_idempotency(&state, &token, idempotency.as_ref()).await {
        Ok(Some(replayed)) => {
            log_token_call(
                &state,
                &token,
                "PUT",
                "update",
                Some(event_id),
                None,
                replayed.status().as_u16(),
                None,
            )
            .await;
            return Ok(replayed);
        }
        Ok(None) => {}
        Err(err) => {
            log_token_call(
                &state,
                &token,
                "PUT",
                "update",
                Some(event_id),
                None,
                err.status_code().as_u16(),
                Some(err.kind()),
            )
            .await;
            return Err(err);
        }
    }

    let storage = req
        .app_data::<web::Data<AttachmentStorage>>()
        .map(|d| d.as_ref());
    match execute_update(
        &state,
        storage,
        &token,
        event_id,
        &body,
        idempotency.as_ref(),
    )
    .await
    {
        Ok(updated) => {
            log_token_call(
                &state,
                &token,
                "PUT",
                "update",
                Some(event_id),
                updated.external_id.as_deref(),
                StatusCode::OK.as_u16(),
                None,
            )
            .await;
            let value = serde_json::to_value(&updated).map_err(anyhow::Error::from)?;
            Ok(no_store_response(StatusCode::OK, Some(value), false))
        }
        Err(err) => {
            log_token_call(
                &state,
                &token,
                "PUT",
                "update",
                Some(event_id),
                None,
                err.status_code().as_u16(),
                Some(err.kind()),
            )
            .await;
            Err(err)
        }
    }
}

/// 予定 ID を指定して予定を削除する (ゴミ箱へ移動)
#[utoipa::path(
    tag = "hooks",
    security(("api_token" = [])),
    params(
        HookEventPath,
        DeleteOptions,
        ("Idempotency-Key" = Option<String>, Header, description = "再送時の冪等キー (1〜128 文字)")
    ),
    responses(
        (status = 204),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
        (status = 429, body = ErrorBody),
    )
)]
#[delete("/guilds/{guild_id}/events/{event_id}")]
pub async fn delete_event(
    req: HttpRequest,
    path: web::Path<HookEventPath>,
    options: web::Query<DeleteOptions>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let event_id = path.event_id;
    let token = authenticate(
        &req,
        &state,
        &path.guild_id,
        true,
        "DELETE",
        "delete",
        Some(event_id),
        None,
    )
    .await?;

    let body_json = json!({
        "scope": options.scope,
        "expected_series_version": options.expected_series_version,
    });
    let idempotency =
        match extract_idempotency(&req, "DELETE", "delete", Some(event_id), None, &body_json) {
            Ok(meta) => meta,
            Err(err) => {
                log_token_call(
                    &state,
                    &token,
                    "DELETE",
                    "delete",
                    Some(event_id),
                    None,
                    err.status_code().as_u16(),
                    Some(err.kind()),
                )
                .await;
                return Err(err);
            }
        };

    let _writer = event_links::lock_writer(&state.pool, &token.guild_id).await?;
    match check_idempotency(&state, &token, idempotency.as_ref()).await {
        Ok(Some(replayed)) => {
            log_token_call(
                &state,
                &token,
                "DELETE",
                "delete",
                Some(event_id),
                None,
                replayed.status().as_u16(),
                None,
            )
            .await;
            return Ok(replayed);
        }
        Ok(None) => {}
        Err(err) => {
            log_token_call(
                &state,
                &token,
                "DELETE",
                "delete",
                Some(event_id),
                None,
                err.status_code().as_u16(),
                Some(err.kind()),
            )
            .await;
            return Err(err);
        }
    }

    match execute_delete(&state, &token, event_id, &options, idempotency.as_ref()).await {
        Ok(()) => {
            log_token_call(
                &state,
                &token,
                "DELETE",
                "delete",
                Some(event_id),
                None,
                StatusCode::NO_CONTENT.as_u16(),
                None,
            )
            .await;
            Ok(no_store_response(StatusCode::NO_CONTENT, None, false))
        }
        Err(err) => {
            log_token_call(
                &state,
                &token,
                "DELETE",
                "delete",
                Some(event_id),
                None,
                err.status_code().as_u16(),
                Some(err.kind()),
            )
            .await;
            Err(err)
        }
    }
}
