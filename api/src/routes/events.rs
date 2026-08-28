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

/// 予定の更新で、状態を読み直してやり直す回数の上限 (#94)。
/// 連携が絡む更新は予定単位のロック取得後に 1 回読み直すのと、
/// 別プロセス経由などロックの外から対応付けが変わっていたときのやり直しに使う。
/// 何度も割り込まれるのは考えにくいので少なめでよい
const UPDATE_SYNC_ATTEMPTS: u32 = 4;

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
    // 作成では省略 (フラグを知らない古いクライアント) は「作らない」として扱う
    let discord_scheduled_event = body.discord_scheduled_event.unwrap_or(false);
    events::validate_discord_flag(&body, discord_scheduled_event, now_jst())?;
    if discord_scheduled_event {
        ensure_can_create_events(&member)?;
    }
    let guild_id = member.guild_id();

    if !discord_scheduled_event {
        let row = events::create(&state.pool, guild_id, &body, now_jst()).await?;
        tracing::info!(guild_id, event_id = row.id, user_id = %member.user.discord_user_id, "event created");
        return Ok(HttpResponse::Created().json(Event::from(row)));
    }

    // Discord 連携あり (#94): 先に Discord にイベントを作り、成功したら短いトランザクションで
    // 予定と対応付けを保存する (DB 接続を握ったまま最大 10 秒の外部呼び出しを待つと、Discord の
    // 遅延時に少ないプールが埋まってしまう)。Discord 側が失敗したら予定も作らず全体を失敗にし、
    // DB 側が失敗したら作ってしまったイベントを後始末する
    let scheduled_event_id = state
        .discord
        .create_scheduled_event(
            guild_id,
            &payload_for(&state.site_base_url, guild_id, &body),
        )
        .await
        .map_err(describe_scheduled_event_error)?;
    let result: Result<events::EventRow, ApiError> = async {
        let mut tx = state.pool.begin().await?;
        // 管理コンソールの全予定削除と排他する (新しい行は行ロックでは待たせられないため、
        // ギルド単位の勧告ロックで、削除側が控えた対応付けの一覧からこの連携が漏れないようにする)
        event_links::lock_guild(&mut *tx, guild_id).await?;
        let row = events::create(&mut *tx, guild_id, &body, now_jst()).await?;
        event_links::insert(&mut *tx, guild_id, row.id, &scheduled_event_id, now_jst()).await?;
        tx.commit().await?;
        Ok(row)
    }
    .await;
    let mut row = match result {
        Ok(row) => row,
        Err(err) => {
            cleanup_scheduled_event(&state.discord, guild_id, &scheduled_event_id).await;
            return Err(err);
        }
    };
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
        (status = 409, description = "同じ予定への並行更新とぶつかった (やり直せば通る)", body = ErrorBody),
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
    let guild_id = member.guild_id();

    // 変更前の状態をロックせずに読み、Discord への反映を分岐する (#94)。
    // DB 接続を握ったまま最大 10 秒の外部呼び出しを待たないため、Discord の操作は
    // トランザクションの外で先に済ませ、書き込み時にロックを取り直して実際の対応付けと
    // 突き合わせる。突き合わせがずれていたら (同じ予定への並行更新)、割り込んだ側の
    // commit 後の後始末がこの更新の対応先を消しうるので、書き込まずに最初からやり直す
    let mut event_lock = None;
    for _ in 0..UPDATE_SYNC_ATTEMPTS {
        let old = events::find_by_id(&state.pool, guild_id, path.event_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("event not found".into()))?;

        // 更新では省略 (フラグを知らない古いクライアント) は「現在の連携状態を保持」として扱う
        // (既定 false にすると、古いタブからの編集・ドラッグで既存の連携が意図せず外れてしまう)
        let discord_scheduled_event = body
            .discord_scheduled_event
            .unwrap_or(old.discord_scheduled_event_id.is_some());
        events::validate_discord_flag(&body, discord_scheduled_event, now_jst())?;
        // 連携を**増やす**操作だけ本人の権限を要求する (既存の連携予定の値の編集や
        // 連携の解除は、共有カレンダーとして誰でも編集できるという方針のまま制限しない)
        if discord_scheduled_event && old.discord_scheduled_event_id.is_none() {
            ensure_can_create_events(&member)?;
        }

        // 連携なしの通常更新 (最頻パス) は 1 文で済ませる。直前に別リクエストが連携を
        // 足していた場合は、その連携への値の反映が次の編集まで遅れるだけ
        if old.discord_scheduled_event_id.is_none() && !discord_scheduled_event {
            let row = events::update(&state.pool, guild_id, path.event_id, &body)
                .await?
                .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
            tracing::info!(guild_id, event_id = row.id, user_id = %member.user.discord_user_id, "event updated");
            return Ok(web::Json(Event::from(row)));
        }

        // 連携が絡む更新は予定単位で直列化する (同じ予定への並行更新では、突き合わせだけだと
        // 双方が同じ連携を維持したまま Discord への反映順と commit 順が食い違い、
        // どちらの内容とも言えない状態で成功してしまう)。ロックは api のプロセス内のもの
        // (`AppState::event_update_locks`)。待っている間に状態が変わりうるので、取れたら読み直す
        if event_lock.is_none() {
            event_lock = Some(state.event_update_locks.lock(path.event_id).await);
            continue;
        }

        // Discord 側を先に「あるべき状態」にする。失敗したら予定には一切触れず全体を失敗にする。
        // desired = この更新後に紐付いているべき scheduled_event_id (None = 連携なし)、
        // created_now = この更新で新しく作った Discord イベント (DB 失敗・やり直し時の後始末対象)
        let (desired, created_now): (Option<String>, Option<String>) =
            match (&old.discord_scheduled_event_id, discord_scheduled_event) {
                (None, true) => {
                    let id = state
                        .discord
                        .create_scheduled_event(
                            guild_id,
                            &payload_for(&state.site_base_url, guild_id, &body),
                        )
                        .await
                        .map_err(describe_scheduled_event_error)?;
                    (Some(id.clone()), Some(id))
                }
                (Some(current), true) => {
                    let payload = payload_for(&state.site_base_url, guild_id, &body);
                    let modified = state
                        .discord
                        .modify_scheduled_event(guild_id, current, &payload)
                        .await
                        .map_err(describe_scheduled_event_error)?;
                    if modified {
                        (Some(current.clone()), None)
                    } else {
                        // Discord 側で手動削除されていた: フラグが有効 = あるべき状態なので作り直す
                        let id = state
                            .discord
                            .create_scheduled_event(guild_id, &payload)
                            .await
                            .map_err(describe_scheduled_event_error)?;
                        (Some(id.clone()), Some(id))
                    }
                }
                // 連携を外す。Discord 側の削除は commit 後にベストエフォートで行う
                (Some(_), false) => (None, None),
                (None, false) => unreachable!("handled above"),
            };

        // 短いトランザクションで、対応付けが分岐の起点から変わっていないことを確かめてから
        // (変わっていたら `None` = やり直し)、値を更新して対応付けを desired に合わせる
        let tx_result: Result<Option<(events::EventRow, Option<String>)>, ApiError> = async {
            let mut tx = state.pool.begin().await?;
            // 管理コンソールの全予定削除と排他する (routes/admin_ops.rs のロック順と揃える)
            event_links::lock_guild(&mut *tx, guild_id).await?;
            let current = events::find_by_id_for_update(&mut tx, guild_id, path.event_id)
                .await?
                .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
            if current.discord_scheduled_event_id != old.discord_scheduled_event_id {
                return Ok(None);
            }
            let mut row = events::update(&mut *tx, guild_id, path.event_id, &body)
                .await?
                .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
            // 突き合わせ済みなので current の対応付けは old と同じ。desired との差分だけ書く
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
                    // 開始前のイベントだけ Discord 側も消す (開始済みはサーバーの履歴として残す)
                    if old.start_at > now_jst() {
                        replaced = Some(existing.clone());
                    }
                }
                _ => {}
            }
            tx.commit().await?;
            row.discord_scheduled_event_id = desired.clone();
            Ok(Some((row, replaced)))
        }
        .await;
        match tx_result {
            Ok(Some((row, replaced))) => {
                // 置き換え・解除で不要になったイベントの後始末 (ベストエフォート。
                // Bot が権限を失った後でも連携の解除まで塞がず、取り残しは Discord 側で手動削除できる)
                if let Some(sid) = &replaced {
                    cleanup_scheduled_event(&state.discord, guild_id, sid).await;
                }
                tracing::info!(guild_id, event_id = row.id, user_id = %member.user.discord_user_id, "event updated");
                return Ok(web::Json(Event::from(row)));
            }
            // 並行更新に割り込まれた: この試行で作った Discord イベントを片付けてやり直す
            Ok(None) => {
                if let Some(sid) = &created_now {
                    cleanup_scheduled_event(&state.discord, guild_id, sid).await;
                }
                tracing::info!(
                    guild_id,
                    event_id = path.event_id,
                    "retrying an event update due to a concurrent link change"
                );
            }
            Err(err) => {
                // この更新で新しく作った Discord イベントは後始末する
                // (変更 (PATCH) は戻せないので、値のずれは次の編集の反映に任せる)
                if let Some(sid) = &created_now {
                    cleanup_scheduled_event(&state.discord, guild_id, sid).await;
                }
                return Err(err);
            }
        }
    }
    Err(ApiError::Conflict(
        "the event was updated concurrently, please retry".into(),
    ))
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
    let row = events::find_by_id_for_update(&mut tx, guild_id, path.event_id)
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

/// 予定を Discord のスケジュールイベントと連携させるには、**操作するユーザー自身**が
/// Discord の「イベントの作成」権限を持っていることを要求する (#94)。
/// 連携の実行は Bot が代行するため、これを見ないと本人の権限では作れないイベントを
/// web 経由で作れてしまう (権限昇格)。restricted モードとは独立した判定で、
/// restricted が off のギルドでも Discord 側の権限設定はそのまま効く。
/// 既存の連携予定の編集や連携の解除には要求しない (共有カレンダーとしての編集は誰でもできる方針)
fn ensure_can_create_events(member: &GuildMember) -> Result<(), ApiError> {
    if !member.permissions().create_events() {
        return Err(ApiError::Forbidden(
            "the Create Events permission is required to link an event to Discord".into(),
        ));
    }
    Ok(())
}

/// 予定の値から Discord スケジュールイベントのボディを組み立てる (#94)
fn payload_for(site_base_url: &str, guild_id: &str, input: &EventInput) -> ScheduledEventPayload {
    ScheduledEventPayload::new(
        site_base_url,
        guild_id,
        &input.name,
        input.description.as_deref(),
        input.is_all_day,
        input.start_at,
        input.end_at,
    )
}

/// スケジュールイベント操作の Discord エラーを利用者に返せる形に変換する。
/// 403 は Bot の「イベントの作成」権限の不足 (再招待で直る)、
/// 400 は Discord 側が受け付けない内容 (開始済みイベントの日時変更など)。
/// それ以外は既存の変換のまま (429 → 503、他 → 502)
fn describe_scheduled_event_error(err: DiscordError) -> ApiError {
    match &err {
        DiscordError::Status { status, .. } if *status == reqwest::StatusCode::FORBIDDEN => {
            ApiError::Forbidden("the bot lacks the Create Events permission in this guild".into())
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
