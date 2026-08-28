use actix_web::{HttpResponse, delete, get, post, put, web};
use chrono::{Duration, NaiveDateTime};
use serde::Deserialize;
use sqlx::PgPool;
use utoipa::IntoParams;

use super::GuildMember;
use crate::{
    discord::{DiscordClient, DiscordError, scheduled_events::ScheduledEventPayload},
    error::{ApiError, ErrorBody},
    models::{
        event_links,
        events::{self, Event, EventInput},
        guilds, now_jst,
    },
    state::AppState,
};

/// 一度に取得できる期間の上限 (FullCalendar の月表示は最大 6 週間)
const MAX_RANGE_DAYS: i64 = 400;

#[derive(Deserialize, IntoParams)]
pub struct ListQuery {
    /// 取得範囲の開始 (JST、この時刻を含む)
    #[param(example = "2026-08-01T00:00:00")]
    pub start: NaiveDateTime,
    /// 取得範囲の終了 (JST、この時刻を含まない)
    #[param(example = "2026-09-01T00:00:00")]
    pub end: NaiveDateTime,
}

impl ListQuery {
    /// 範囲の向きと長さを確認する (管理コンソールの一覧でも同じ条件を使う)
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.end <= self.start {
            return Err(ApiError::BadRequest("end must be after start".into()));
        }
        if self.end - self.start > Duration::days(MAX_RANGE_DAYS) {
            return Err(ApiError::BadRequest(format!(
                "range must be at most {MAX_RANGE_DAYS} days"
            )));
        }
        Ok(())
    }
}

#[derive(Deserialize, IntoParams)]
pub struct EventPath {
    /// ギルド ID (認可は `GuildMember` が行うのでここでは読まない。OpenAPI 用)
    #[allow(dead_code)]
    pub guild_id: String,
    /// 予定 ID
    pub event_id: i32,
}

/// 期間に重なる予定の一覧
#[utoipa::path(
    tag = "events",
    params(("guild_id" = String, Path, description = "ギルド ID"), ListQuery),
    responses(
        (status = 200, body = Vec<Event>),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[get("/{guild_id}")]
pub async fn list(
    member: GuildMember,
    query: web::Query<ListQuery>,
    state: web::Data<AppState>,
) -> Result<web::Json<Vec<Event>>, ApiError> {
    query.validate()?;
    let rows = events::list_between(&state.pool, member.guild_id(), query.start, query.end).await?;
    Ok(web::Json(rows.into_iter().map(Event::from).collect()))
}

/// 予定の作成
#[utoipa::path(
    tag = "events",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    request_body = EventInput,
    responses(
        (status = 201, body = Event),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, description = "非メンバー / restricted モードで権限なし", body = ErrorBody),
    )
)]
#[post("/{guild_id}")]
pub async fn create(
    member: GuildMember,
    body: web::Json<EventInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    ensure_can_edit(&state.pool, &member).await?;
    body.validate()?;
    events::validate_discord_flag(&body, now_jst())?;
    let guild_id = member.guild_id();

    if !body.discord_scheduled_event {
        let row = events::create(&state.pool, guild_id, &body, now_jst()).await?;
        tracing::info!(guild_id, event_id = row.id, user_id = %member.user.discord_user_id, "event created");
        return Ok(HttpResponse::Created().json(Event::from(row)));
    }

    // Discord 連携あり (#94): 予定と対応付けを同じトランザクションで作り、間で Discord に
    // イベントを作る。Discord 側が失敗したら全体を失敗にする (オプトインしたのに連携されて
    // いない予定を作らない)。DB 側が失敗したら作ってしまったイベントを後始末する
    let mut tx = state.pool.begin().await?;
    let mut row = events::create(&mut *tx, guild_id, &body, now_jst()).await?;
    let scheduled_event_id = state
        .discord
        .create_scheduled_event(guild_id, &payload_for(guild_id, &body))
        .await
        .map_err(describe_scheduled_event_error)?;
    if let Err(err) =
        event_links::insert(&mut *tx, guild_id, row.id, &scheduled_event_id, now_jst()).await
    {
        cleanup_scheduled_event(&state.discord, guild_id, &scheduled_event_id).await;
        return Err(err.into());
    }
    if let Err(err) = tx.commit().await {
        cleanup_scheduled_event(&state.discord, guild_id, &scheduled_event_id).await;
        return Err(err.into());
    }
    row.discord_scheduled_event_id = Some(scheduled_event_id);
    tracing::info!(guild_id, event_id = row.id, user_id = %member.user.discord_user_id, "event created with a discord scheduled event");
    Ok(HttpResponse::Created().json(Event::from(row)))
}

/// 予定の更新 (全フィールド置き換え)
#[utoipa::path(
    tag = "events",
    params(EventPath),
    request_body = EventInput,
    responses(
        (status = 200, body = Event),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, description = "非メンバー / restricted モードで権限なし", body = ErrorBody),
        (status = 404, body = ErrorBody),
    )
)]
#[put("/{guild_id}/{event_id}")]
pub async fn update(
    member: GuildMember,
    path: web::Path<EventPath>,
    body: web::Json<EventInput>,
    state: web::Data<AppState>,
) -> Result<web::Json<Event>, ApiError> {
    ensure_can_edit(&state.pool, &member).await?;
    body.validate()?;
    events::validate_discord_flag(&body, now_jst())?;
    let guild_id = member.guild_id();

    // 変更前の行をロックして読む (#94)。既存の対応付けと「Discord 側のイベントが開始前か」
    // (変更前の start_at で判断する) を見て Discord への反映を分岐する
    let mut tx = state.pool.begin().await?;
    let old = events::find_by_id_for_update(&mut *tx, guild_id, path.event_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    let mut row = events::update(&mut *tx, guild_id, path.event_id, &body)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;

    match (old.discord_scheduled_event_id, body.discord_scheduled_event) {
        // 連携なしのまま
        (None, false) => tx.commit().await?,
        // 連携を有効にした: 作成と同じ流れ
        (None, true) => {
            let scheduled_event_id = state
                .discord
                .create_scheduled_event(guild_id, &payload_for(guild_id, &body))
                .await
                .map_err(describe_scheduled_event_error)?;
            if let Err(err) =
                event_links::insert(&mut *tx, guild_id, row.id, &scheduled_event_id, now_jst())
                    .await
            {
                cleanup_scheduled_event(&state.discord, guild_id, &scheduled_event_id).await;
                return Err(err.into());
            }
            if let Err(err) = tx.commit().await {
                cleanup_scheduled_event(&state.discord, guild_id, &scheduled_event_id).await;
                return Err(err.into());
            }
            row.discord_scheduled_event_id = Some(scheduled_event_id);
        }
        // 連携を保ったまま変更: Discord 側にも反映する
        (Some(scheduled_event_id), true) => {
            let payload = payload_for(guild_id, &body);
            let modified = state
                .discord
                .modify_scheduled_event(guild_id, &scheduled_event_id, &payload)
                .await
                .map_err(describe_scheduled_event_error)?;
            if modified {
                // commit だけ失敗すると Discord 側が新しい値のまま残るが、次の編集の反映で追いつく
                tx.commit().await?;
                row.discord_scheduled_event_id = Some(scheduled_event_id);
            } else {
                // Discord 側で手動削除されていた: フラグが有効 = あるべき状態なので作り直す
                let new_id = state
                    .discord
                    .create_scheduled_event(guild_id, &payload)
                    .await
                    .map_err(describe_scheduled_event_error)?;
                if let Err(err) =
                    event_links::set_scheduled_event_id(&mut *tx, guild_id, row.id, &new_id).await
                {
                    cleanup_scheduled_event(&state.discord, guild_id, &new_id).await;
                    return Err(err.into());
                }
                if let Err(err) = tx.commit().await {
                    cleanup_scheduled_event(&state.discord, guild_id, &new_id).await;
                    return Err(err.into());
                }
                row.discord_scheduled_event_id = Some(new_id);
            }
        }
        // 連携を外した: 対応付けを消し、開始前のイベントだけ Discord 側も消す。
        // 開始済み (変更前の start_at が過去) のイベントはサーバーの履歴として残す。
        // Discord 側の削除はベストエフォート (Bot が権限を失った後でも連携の解除まで
        // 塞がないため。取り残したイベントは Discord 側で手動削除できる)
        (Some(scheduled_event_id), false) => {
            event_links::delete(&mut *tx, guild_id, row.id).await?;
            tx.commit().await?;
            if old.start_at > now_jst() {
                cleanup_scheduled_event(&state.discord, guild_id, &scheduled_event_id).await;
            }
        }
    }
    tracing::info!(guild_id, event_id = row.id, user_id = %member.user.discord_user_id, "event updated");
    Ok(web::Json(Event::from(row)))
}

/// 予定の削除
#[utoipa::path(
    tag = "events",
    params(EventPath),
    responses(
        (status = 204),
        (status = 401, body = ErrorBody),
        (status = 403, description = "非メンバー / restricted モードで権限なし", body = ErrorBody),
        (status = 404, body = ErrorBody),
    )
)]
#[delete("/{guild_id}/{event_id}")]
pub async fn delete(
    member: GuildMember,
    path: web::Path<EventPath>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    ensure_can_edit(&state.pool, &member).await?;
    let guild_id = member.guild_id();

    // 対応付け (#94) を読んでから消すため、行をロックして削除する。
    // 対応付けの行自体は events の削除に CASCADE で追随する
    let mut tx = state.pool.begin().await?;
    let row = events::find_by_id_for_update(&mut *tx, guild_id, path.event_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    events::delete(&mut *tx, guild_id, path.event_id).await?;
    tx.commit().await?;
    if let Some(scheduled_event_id) = &row.discord_scheduled_event_id
        && row.start_at > now_jst()
    {
        // 開始前のイベントだけ Discord 側も消す (開始済みはサーバーの履歴として残す)。
        // ベストエフォート: Discord 側の失敗で予定の削除を詰まらせない
        // (Bot が権限を失っていても削除はできる。取り残したイベントは Discord 側で手動削除できる)
        cleanup_scheduled_event(&state.discord, guild_id, scheduled_event_id).await;
    }
    tracing::info!(guild_id, event_id = path.event_id, user_id = %member.user.discord_user_id, "event deleted");
    Ok(HttpResponse::NoContent().finish())
}

/// restricted モードのギルドでは管理権限を持つユーザーだけが予定を編集できる。
/// 旧実装はこの判定をクライアント側だけで行っていたが、サーバー側で強制する
async fn ensure_can_edit(pool: &PgPool, member: &GuildMember) -> Result<(), ApiError> {
    let config = guilds::get_config(pool, member.guild_id()).await?;
    if config.restricted && !member.permissions().can_manage_server() {
        return Err(ApiError::Forbidden(
            "this guild restricts editing events to users with manage permissions".into(),
        ));
    }
    Ok(())
}

/// 予定の値から Discord スケジュールイベントのボディを組み立てる (#94)
fn payload_for(guild_id: &str, input: &EventInput) -> ScheduledEventPayload {
    ScheduledEventPayload::new(
        guild_id,
        &input.name,
        input.description.as_deref(),
        input.is_all_day,
        input.start_at,
        input.end_at,
    )
}

/// スケジュールイベント操作の Discord エラーを利用者に返せる形に変換する。
/// 403 は Bot の「イベントの管理」権限の不足 (再招待で直る)、
/// 400 は Discord 側が受け付けない内容 (開始済みイベントの日時変更など)。
/// それ以外は既存の変換のまま (429 → 503、他 → 502)
fn describe_scheduled_event_error(err: DiscordError) -> ApiError {
    match &err {
        DiscordError::Status { status, .. } if *status == reqwest::StatusCode::FORBIDDEN => {
            ApiError::Forbidden("the bot lacks the Manage Events permission in this guild".into())
        }
        DiscordError::Status { status, .. } if *status == reqwest::StatusCode::BAD_REQUEST => {
            ApiError::BadRequest("Discord did not accept the scheduled event".into())
        }
        _ => err.into(),
    }
}

/// Discord スケジュールイベントのベストエフォートの削除 (失敗はログに残すだけ)。
/// DB 側の失敗で対応付けを保存できなかったときの後始末と、連携の解除・予定の削除に伴う
/// Discord 側の削除 (どちらも Discord 側の失敗で操作全体を止めない) に使う。
/// 取り残したイベントは Discord 側で手動削除できる
async fn cleanup_scheduled_event(
    discord: &DiscordClient,
    guild_id: &str,
    scheduled_event_id: &str,
) {
    if let Err(err) = discord
        .delete_scheduled_event(guild_id, scheduled_event_id)
        .await
    {
        tracing::warn!(guild_id, scheduled_event_id, error = %err, "failed to clean up an orphan scheduled event");
    }
}
