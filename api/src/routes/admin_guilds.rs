//! 管理コンソールのギルド・予定 (`/admin/guilds/*`、#35)。
//!
//! すべて [`AdminUser`] を要求し、[`super::GuildMember`] (ギルドのメンバーシップ) は見ない。
//! 管理者は所属していないギルドの予定も扱えるが、`guild_id` + `event_id` での絞り込みは通常 API と同じく必ず行う。
//! 書き込み (予定の作成・更新・削除、restricted の切替) は操作と同じトランザクションで `admin_audit_logs` に残す。

use actix_web::{HttpResponse, delete, get, post, put, web};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{events::ListQuery, guilds::GuildConfigInput, member::is_snowflake};
use crate::{
    admin::AdminUser,
    error::{ApiError, ErrorBody},
    models::{
        admin_audit::{self, AuditEntry},
        admin_guilds::{self, AdminGuild, MAX_PAGE, PAGE_SIZE},
        events::{self, Event, EventInput},
        guilds::{self, GuildConfig},
        now_jst,
    },
    state::AppState,
};

#[derive(Deserialize, IntoParams)]
pub struct GuildListQuery {
    /// guild_id の完全一致か名前の部分一致。空なら全件
    #[serde(default)]
    pub q: String,
    /// 1 始まりのページ番号 (上限 1,000,000)
    #[serde(default = "default_page")]
    #[param(example = 1, minimum = 1, maximum = 1_000_000)]
    pub page: i64,
}

fn default_page() -> i64 {
    1
}

/// ギルド一覧のレスポンス
#[derive(Serialize, ToSchema)]
pub struct AdminGuildPage {
    pub items: Vec<AdminGuild>,
    /// 検索条件に一致する総件数
    #[schema(example = 123)]
    pub total: i64,
    #[schema(example = 1)]
    pub page: i64,
    #[schema(example = 50)]
    pub page_size: i64,
}

/// 全ギルドの一覧・検索
#[utoipa::path(
    tag = "admin",
    params(GuildListQuery),
    responses(
        (status = 200, body = AdminGuildPage),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[get("/guilds")]
pub async fn list_guilds(
    _admin: AdminUser,
    query: web::Query<GuildListQuery>,
    state: web::Data<AppState>,
) -> Result<web::Json<AdminGuildPage>, ApiError> {
    if !(1..=MAX_PAGE).contains(&query.page) {
        return Err(ApiError::BadRequest(format!(
            "page must be between 1 and {MAX_PAGE}"
        )));
    }
    let q = query.q.trim();
    let (items, total) = tokio::try_join!(
        admin_guilds::list(&state.pool, q, query.page),
        admin_guilds::count(&state.pool, q),
    )?;
    Ok(web::Json(AdminGuildPage {
        items,
        total,
        page: query.page,
        page_size: PAGE_SIZE,
    }))
}

#[derive(Deserialize, IntoParams)]
pub struct GuildPath {
    /// ギルド ID
    pub guild_id: String,
}

#[derive(Deserialize, IntoParams)]
pub struct GuildEventPath {
    /// ギルド ID
    pub guild_id: String,
    /// 予定 ID
    pub event_id: i32,
}

/// パスの guild_id が Snowflake の形式であることを確認する (`GuildMember` extractor と同じ扱い)
pub(super) fn validated_guild_id(guild_id: &str) -> Result<&str, ApiError> {
    if !is_snowflake(guild_id) {
        return Err(ApiError::BadRequest("guild_id must be a snowflake".into()));
    }
    Ok(guild_id)
}

/// ギルド詳細 (一覧の項目 + Bot の参加状況)
#[derive(Serialize, ToSchema)]
pub struct AdminGuildDetail {
    #[serde(flatten)]
    pub guild: AdminGuild,
    /// Bot が現在このギルドに参加しているか (Discord API で確認)。
    /// Discord API に問い合わせられなかったときは null
    pub bot_joined: Option<bool>,
}

/// ギルドの詳細。`guilds` テーブルに無い (Bot が一度も参加していない) ギルドは 404
#[utoipa::path(
    tag = "admin",
    params(GuildPath),
    responses(
        (status = 200, body = AdminGuildDetail),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 404, body = ErrorBody),
    )
)]
#[get("/guilds/{guild_id}")]
pub async fn get_guild(
    _admin: AdminUser,
    path: web::Path<GuildPath>,
    state: web::Data<AppState>,
) -> Result<web::Json<AdminGuildDetail>, ApiError> {
    let guild_id = validated_guild_id(&path.guild_id)?;
    let guild = admin_guilds::find(&state.pool, guild_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("guild not found".into()))?;
    // Discord 障害時も DB の情報は見られるようにする (参加状況だけ不明にする)
    let bot_joined = match state.discord.guild(guild_id).await {
        Ok(snapshot) => Some(snapshot.is_some()),
        Err(error) => {
            tracing::warn!(guild_id, error = %error, "failed to check bot membership for the admin console");
            None
        }
    };
    Ok(web::Json(AdminGuildDetail { guild, bot_joined }))
}

/// 期間に重なる予定の一覧 (通常 API の `GET /events/{guild_id}` と同じ条件)
#[utoipa::path(
    tag = "admin",
    params(GuildPath, ListQuery),
    responses(
        (status = 200, body = Vec<Event>),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[get("/guilds/{guild_id}/events")]
pub async fn list_events(
    _admin: AdminUser,
    path: web::Path<GuildPath>,
    query: web::Query<ListQuery>,
    state: web::Data<AppState>,
) -> Result<web::Json<Vec<Event>>, ApiError> {
    let guild_id = validated_guild_id(&path.guild_id)?;
    query.validate()?;
    let rows = events::list_between(&state.pool, guild_id, query.start, query.end).await?;
    Ok(web::Json(rows.into_iter().map(Event::from).collect()))
}

/// 予定の作成 (管理者による代理登録)。`admin_audit_logs` に `event.create` を記録する。
/// どのテーブルにも無いギルド ID (打ち間違い) には 404 を返し、孤立した予定を作らない
#[utoipa::path(
    tag = "admin",
    params(GuildPath),
    request_body = EventInput,
    responses(
        (status = 201, body = Event),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 404, description = "存在しない (したことのない) ギルド", body = ErrorBody),
    )
)]
#[post("/guilds/{guild_id}/events")]
pub async fn create_event(
    admin: AdminUser,
    path: web::Path<GuildPath>,
    body: web::Json<EventInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let guild_id = validated_guild_id(&path.guild_id)?;
    body.validate()?;
    let mut tx = state.pool.begin().await?;
    ensure_guild_known(&mut *tx, guild_id).await?;
    let event = Event::from(events::create(&mut *tx, guild_id, &body, now_jst()).await?);
    admin_audit::record(
        &mut *tx,
        &admin,
        AuditEntry {
            action: "event.create",
            target_type: Some("event"),
            target_id: Some(&event.id.to_string()),
            after: Some(snapshot(&event)?),
            detail: Some(serde_json::json!({ "guild_id": guild_id })),
            ..Default::default()
        },
    )
    .await?;
    tx.commit().await?;
    tracing::info!(guild_id, event_id = event.id, admin = %admin.discord_user_id, "event created by admin");
    Ok(HttpResponse::Created().json(event))
}

/// 予定の更新 (全フィールド置き換え)。`admin_audit_logs` に変更前後を記録する
#[utoipa::path(
    tag = "admin",
    params(GuildEventPath),
    request_body = EventInput,
    responses(
        (status = 200, body = Event),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 404, body = ErrorBody),
    )
)]
#[put("/guilds/{guild_id}/events/{event_id}")]
pub async fn update_event(
    admin: AdminUser,
    path: web::Path<GuildEventPath>,
    body: web::Json<EventInput>,
    state: web::Data<AppState>,
) -> Result<web::Json<Event>, ApiError> {
    let guild_id = validated_guild_id(&path.guild_id)?;
    body.validate()?;
    let mut tx = state.pool.begin().await?;
    let before = events::find_by_id_for_update(&mut tx, guild_id, path.event_id)
        .await?
        .map(Event::from)
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    let mut after = events::update(&mut *tx, guild_id, path.event_id, &body)
        .await?
        .map(Event::from)
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    // 管理コンソールの更新は Discord 連携 (#94) に触れず対応付けも変えないので、
    // 変更前の値を引き継ぐ (`events::update` の戻り値は常に None のため、そのままだと
    // レスポンスと監査ログの after が「連携解除」に見えてしまう)
    after.discord_scheduled_event_id = before.discord_scheduled_event_id.clone();
    admin_audit::record(
        &mut *tx,
        &admin,
        AuditEntry {
            action: "event.update",
            target_type: Some("event"),
            target_id: Some(&after.id.to_string()),
            before: Some(snapshot(&before)?),
            after: Some(snapshot(&after)?),
            detail: Some(serde_json::json!({ "guild_id": guild_id })),
        },
    )
    .await?;
    tx.commit().await?;
    tracing::info!(guild_id, event_id = after.id, admin = %admin.discord_user_id, "event updated by admin");
    Ok(web::Json(after))
}

/// 予定の削除。`admin_audit_logs` に削除前の内容を記録する
#[utoipa::path(
    tag = "admin",
    params(GuildEventPath),
    responses(
        (status = 204),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 404, body = ErrorBody),
    )
)]
#[delete("/guilds/{guild_id}/events/{event_id}")]
pub async fn delete_event(
    admin: AdminUser,
    path: web::Path<GuildEventPath>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let guild_id = validated_guild_id(&path.guild_id)?;
    let mut tx = state.pool.begin().await?;
    let before = events::find_by_id_for_update(&mut tx, guild_id, path.event_id)
        .await?
        .map(Event::from)
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    if !events::delete(&mut *tx, guild_id, path.event_id).await? {
        return Err(ApiError::NotFound("event not found".into()));
    }
    admin_audit::record(
        &mut *tx,
        &admin,
        AuditEntry {
            action: "event.delete",
            target_type: Some("event"),
            target_id: Some(&before.id.to_string()),
            before: Some(snapshot(&before)?),
            detail: Some(serde_json::json!({ "guild_id": guild_id })),
            ..Default::default()
        },
    )
    .await?;
    if let Err(err) = tx.commit().await {
        // COMMIT の応答だけ失われて実際には消えていることがある。何もしないと Discord の
        // イベントだけ残り、再試行しても 404 で ID を回収できない (通常の削除と同じ扱い)
        if let Some(scheduled_event_id) = &before.discord_scheduled_event_id
            && matches!(
                events::find_by_id(&state.pool, guild_id, path.event_id).await,
                Ok(None)
            )
        {
            tracing::warn!(
                guild_id,
                event_id = path.event_id,
                "the event was deleted after all (the commit result was lost): cleaning up the scheduled event"
            );
            delete_scheduled_event_best_effort(&state, guild_id, path.event_id, scheduled_event_id)
                .await;
        }
        return Err(err.into());
    }
    // 連携している Discord スケジュールイベントの後始末 (#94)。
    // 管理コンソールの削除は Discord 側の失敗で止めない (ベストエフォート)
    if let Some(scheduled_event_id) = &before.discord_scheduled_event_id {
        delete_scheduled_event_best_effort(&state, guild_id, path.event_id, scheduled_event_id)
            .await;
    }
    tracing::info!(guild_id, event_id = path.event_id, admin = %admin.discord_user_id, "event deleted by admin");
    Ok(HttpResponse::NoContent().finish())
}

/// ギルド設定 (restricted) の変更。通常 API の `PUT /guilds/{guild_id}/config` と同じ保存処理で、
/// 管理権限の判定の代わりに `AdminUser` を要求し、`admin_audit_logs` に変更前後を記録する。
/// どのテーブルにも無いギルド ID には 404 (孤立した設定行を作らない)
#[utoipa::path(
    tag = "admin",
    params(GuildPath),
    request_body = GuildConfigInput,
    responses(
        (status = 200, body = GuildConfig),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 404, description = "存在しない (したことのない) ギルド", body = ErrorBody),
    )
)]
#[put("/guilds/{guild_id}/config")]
pub async fn put_config(
    admin: AdminUser,
    path: web::Path<GuildPath>,
    body: web::Json<GuildConfigInput>,
    state: web::Data<AppState>,
) -> Result<web::Json<GuildConfig>, ApiError> {
    let guild_id = validated_guild_id(&path.guild_id)?;
    let mut tx = state.pool.begin().await?;
    ensure_guild_known(&mut *tx, guild_id).await?;
    let before = guilds::lock_config_for_update(&mut tx, guild_id).await?;
    let after = guilds::upsert_config(&mut *tx, guild_id, body.restricted).await?;
    admin_audit::record(
        &mut *tx,
        &admin,
        AuditEntry {
            action: "guild_config.update",
            target_type: Some("guild"),
            target_id: Some(guild_id),
            before: Some(snapshot(&before)?),
            after: Some(snapshot(&after)?),
            detail: None,
        },
    )
    .await?;
    tx.commit().await?;
    tracing::info!(guild_id, restricted = after.restricted, admin = %admin.discord_user_id, "guild config updated by admin");
    Ok(web::Json(after))
}

/// 連携している Discord スケジュールイベントを消す (#94)。管理コンソールの削除は
/// Discord 側の失敗で止めない (失敗は warn ログだけ残す)
async fn delete_scheduled_event_best_effort(
    state: &AppState,
    guild_id: &str,
    event_id: i32,
    scheduled_event_id: &str,
) {
    if let Err(err) = state
        .discord
        .delete_scheduled_event(guild_id, scheduled_event_id)
        .await
    {
        tracing::warn!(guild_id, event_id, scheduled_event_id, error = %err, "failed to delete the linked scheduled event");
    }
}

/// 書き込み前に、現在または過去に存在したギルドであることを確認する (無ければ 404)
pub(super) async fn ensure_guild_known<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    guild_id: &str,
) -> Result<(), ApiError> {
    if !admin_guilds::exists(executor, guild_id).await? {
        return Err(ApiError::NotFound("guild not found".into()));
    }
    Ok(())
}

/// 監査ログに入れるスナップショット (API レスポンスと同じ JSON)
pub(super) fn snapshot<T: Serialize>(value: &T) -> Result<serde_json::Value, ApiError> {
    serde_json::to_value(value)
        .map_err(|e| anyhow::anyhow!("failed to serialize audit snapshot: {e}").into())
}
