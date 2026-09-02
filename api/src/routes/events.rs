use actix_web::{HttpResponse, delete, get, post, put, web};
use chrono::{Duration, NaiveDateTime};
use futures_util::{StreamExt as _, TryStreamExt as _, stream};
use serde::Deserialize;
use sqlx::PgPool;
use utoipa::IntoParams;

use super::GuildMember;
use crate::{
    auth::AuthUser,
    discord::{DiscordClient, DiscordError, is_snowflake, scheduled_events::ScheduledEventPayload},
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

/// 横断カレンダー (#98) で一度に問い合わせられるギルド数の上限。
/// `/guilds/joined` と同じ根拠 (Discord のユーザーあたり参加上限は 200)
pub(crate) const JOINED_MAX_IDS: usize = 200;

/// 横断カレンダー (#98) でメンバー確認のために同時に Discord へ問い合わせるギルド数。
/// キャッシュが冷えていると 1 ギルドにつき最大 2 回 (ギルド情報 + メンバー情報) 呼ぶので、
/// 参加ギルドが多い利用者でもレート制限に当たりにくいよう絞る (2 回目以降はキャッシュで済む)
const MEMBER_ACCESS_CONCURRENCY: usize = 4;

/// 予定の更新で、状態を読み直してやり直す回数の上限 (#94)。
/// 同じ予定への更新は予定単位のロックで直列化しているので、やり直しが要るのは
/// ロックの外 (別プロセスの api、管理コンソール、Discord 側の手動操作) から
/// 対応付けが変わったときだけ。何度も割り込まれるのは考えにくいので少なめでよい
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
        validate_range(self.start, self.end)
    }
}

/// 取得範囲の向きと長さの確認。横断カレンダー (#98) のクエリも同じ条件にする
fn validate_range(start: NaiveDateTime, end: NaiveDateTime) -> Result<(), ApiError> {
    if end <= start {
        return Err(ApiError::BadRequest("end must be after start".into()));
    }
    if end - start > Duration::days(MAX_RANGE_DAYS) {
        return Err(ApiError::BadRequest(format!(
            "range must be at most {MAX_RANGE_DAYS} days"
        )));
    }
    Ok(())
}

#[derive(Deserialize, IntoParams)]
pub struct JoinedListQuery {
    /// カンマ区切りのギルド ID (Bot 参加済みのもの。web が `/guilds/joined` で絞った結果を渡す)
    #[param(example = "782502586817314816,123456789012345678")]
    pub guild_ids: String,
    /// 取得範囲の開始 (JST、この時刻を含む)
    #[param(example = "2026-08-01T00:00:00")]
    pub start: NaiveDateTime,
    /// 取得範囲の終了 (JST、この時刻を含まない)
    #[param(example = "2026-09-01T00:00:00")]
    pub end: NaiveDateTime,
}

/// カンマ区切りのギルド ID を解析する (`/guilds/joined` と横断カレンダー #98 で共通)。
/// 空要素は捨て、重複は最初の 1 つだけ残す (並びは保つ)。
/// Snowflake でない値と [`JOINED_MAX_IDS`] を超える個数は 400
pub(crate) fn parse_guild_ids(raw: &str) -> Result<Vec<String>, ApiError> {
    let mut ids: Vec<String> = Vec::new();
    for id in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if !is_snowflake(id) {
            return Err(ApiError::BadRequest(
                "guild_ids must be comma-separated snowflakes".into(),
            ));
        }
        if !ids.iter().any(|known| known == id) {
            ids.push(id.to_owned());
        }
    }
    if ids.len() > JOINED_MAX_IDS {
        return Err(ApiError::BadRequest(format!(
            "at most {JOINED_MAX_IDS} guild_ids are allowed"
        )));
    }
    Ok(ids)
}

#[derive(Deserialize, IntoParams)]
pub struct EventPath {
    /// ギルド ID (認可は `GuildMember` が行うのでここでは読まない。OpenAPI 用)
    #[allow(dead_code)]
    pub guild_id: String,
    /// 予定 ID
    pub event_id: i32,
}

/// 参加している複数ギルドの予定をまとめて返す (横断カレンダー #98)。
///
/// 認可は単独カレンダー ([`GuildMember`]) と同じ「Bot が参加していて、かつ呼び出したユーザーがメンバー」を
/// ギルドごとに確かめる。api は利用者の参加ギルド一覧を自力で取れない (利用者の OAuth トークンは web にしか無い)
/// ので、候補の ID は web から受け取り、ここで 1 つずつ絞る。非メンバー / Bot 未参加のギルドは黙って除外する
/// (単独カレンダーの 403 に相当。web が持つ参加状況が古いときに起きる)。
/// restricted モードは編集だけを制限し閲覧には関わらないので、ここでは見ない。
/// Discord への問い合わせが 1 つでも失敗したら全体を失敗にする (そのギルドだけ黙って抜くと
/// 「予定が無い」ように見えてしまう。web は再試行を案内する)
#[utoipa::path(
    tag = "events",
    params(JoinedListQuery),
    responses(
        (status = 200, body = Vec<Event>),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 502, description = "Discord に問い合わせられなかった", body = ErrorBody),
        (status = 503, description = "Discord のレート制限", body = ErrorBody),
    )
)]
#[get("/@me")]
pub async fn list_joined(
    user: AuthUser,
    query: web::Query<JoinedListQuery>,
    state: web::Data<AppState>,
) -> Result<web::Json<Vec<Event>>, ApiError> {
    validate_range(query.start, query.end)?;
    let candidates = parse_guild_ids(&query.guild_ids)?;
    if candidates.is_empty() {
        return Ok(web::Json(Vec::new()));
    }

    // メンバー確認を並列に行う (結果の順序は問わない。予定の並びは SQL 側で決まる)
    let discord = &state.discord;
    let user_id = &user.discord_user_id;
    let checked: Vec<Option<String>> = stream::iter(candidates)
        .map(|guild_id| async move {
            discord
                .member_access(&guild_id, user_id)
                .await
                .map(|access| access.map(|_| guild_id))
        })
        .buffer_unordered(MEMBER_ACCESS_CONCURRENCY)
        .try_collect()
        .await?;
    let skipped = checked.iter().filter(|id| id.is_none()).count();
    let allowed: Vec<String> = checked.into_iter().flatten().collect();
    if skipped > 0 {
        tracing::debug!(
            user_id = %user.discord_user_id,
            skipped,
            "skipped guilds the user cannot view in the joined events list"
        );
    }
    if allowed.is_empty() {
        return Ok(web::Json(Vec::new()));
    }

    let rows = events::list_between_guilds(&state.pool, &allowed, query.start, query.end).await?;
    Ok(web::Json(rows.into_iter().map(Event::from).collect()))
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
        .map_err(|err| describe_create_error(guild_id, &body.name, err))?;
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
            // COMMIT の応答だけ失われて実は保存されていることがあるので、確かめてから消す
            cleanup_unsaved_scheduled_event(&state, guild_id, &scheduled_event_id).await;
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

    // 同じ予定への更新は、連携の有無に関わらず最初から直列化する (#94)。
    // 連携が絡む更新では Discord への反映と commit の順序を揃えるため、連携なしの更新でも
    // 「未連携と読んだ後に別リクエストが連携を足す」競合を防ぐために要る
    // (UPDATE 文の副問い合わせは行ロックを待つ間も文開始時のスナップショットを見るので、
    // SQL の条件だけでは新しい対応付けに気付けない)。
    // ロックは api のプロセス内の軽いもの (`AppState::event_update_locks`) で、DB 接続は握らない
    let _event_lock = state.event_update_locks.lock(path.event_id).await;

    // 変更前の状態をロックせずに読み、Discord への反映を分岐する。
    // DB 接続を握ったまま最大 10 秒の外部呼び出しを待たないため、Discord の操作は
    // トランザクションの外で先に済ませ、書き込み時にロックを取り直して実際の対応付けと
    // 突き合わせる。突き合わせがずれていたら (プロセス外からの変更)、割り込んだ側の
    // commit 後の後始末がこの更新の対応先を消しうるので、書き込まずに最初からやり直す
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

        // 連携なしの通常更新 (最頻パス) は 1 文で済ませる。読んでから書くまでに別リクエストが
        // 連携を足していたら更新は起きない (`update_if_unlinked` が `None`) ので、
        // 連携ありの経路でやり直す (そのまま書くと、連携先に反映されない値が入ってしまう)
        if old.discord_scheduled_event_id.is_none() && !discord_scheduled_event {
            if let Some(row) =
                events::update_if_unlinked(&state.pool, guild_id, path.event_id, &body).await?
            {
                tracing::info!(guild_id, event_id = row.id, user_id = %member.user.discord_user_id, "event updated");
                return Ok(web::Json(Event::from(row)));
            }
            // 予定自体が消えていたなら 404、連携が増えていたならやり直し
            if events::find_by_id(&state.pool, guild_id, path.event_id)
                .await?
                .is_none()
            {
                return Err(ApiError::NotFound("event not found".into()));
            }
            tracing::info!(
                guild_id,
                event_id = path.event_id,
                "retrying an event update: a discord link appeared"
            );
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
                            // 応答を受け取れなかっただけで Discord 側には適用済みかもしれないので、
                            // 変更前の値に戻しておく (適用されていなければ同じ値を書くだけで無害)
                            undo_scheduled_event_changes(
                                &state,
                                guild_id,
                                path.event_id,
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
                        // Discord 側で手動削除されていた: フラグが有効 = あるべき状態なので作り直す。
                        // 作り直しも Discord にイベントを作る操作なので、新規連携と同じ権限を要る
                        // (対応付けが残っているだけで権限チェックを免れると、Bot 経由で作れてしまう)
                        ensure_can_create_events(&member)?;
                        let id = state
                            .discord
                            .create_scheduled_event(guild_id, &payload)
                            .await
                            .map_err(|err| describe_create_error(guild_id, &body.name, err))?;
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
                    // Discord 側も消す。開始済みかどうかでは分けない: DB の start_at は
                    // Discord に同期していない変更 (管理コンソールの編集) でずれることがあり、
                    // 「開始前だけ消す」判定に使うと未来のイベントを取り残してしまう
                    replaced = Some(existing.clone());
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
            // 並行更新に割り込まれた: Discord への反映を取り消してやり直す
            Ok(None) => {
                undo_scheduled_event_changes(
                    &state,
                    guild_id,
                    path.event_id,
                    &body,
                    &created_now,
                    &desired,
                    &old,
                )
                .await;
                tracing::info!(
                    guild_id,
                    event_id = path.event_id,
                    "retrying an event update due to a concurrent link change"
                );
            }
            Err(err) => {
                undo_scheduled_event_changes(
                    &state,
                    guild_id,
                    path.event_id,
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
    if let Err(err) = tx.commit().await {
        // COMMIT の応答だけ失われて、実際には消えていることがある。その場合に何もしないと
        // 予定は消えたのに Discord のイベントだけ残り、再試行しても 404 で ID を回収できない。
        // 別の接続で消えたことを確かめられたときだけ Discord 側も片付ける
        if let Some(scheduled_event_id) = &row.discord_scheduled_event_id
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
            cleanup_scheduled_event(&state.discord, guild_id, scheduled_event_id).await;
        }
        return Err(err.into());
    }
    if let Some(scheduled_event_id) = &row.discord_scheduled_event_id {
        // 予定を消したら Discord のイベントも消す (開始済みかどうかでは分けない。
        // DB の start_at は Discord に同期していない変更でずれることがあり、判定に使えない)。
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
        // Bot が退出・追放されている (#122)。直し方は権限不足と同じ「招待し直す」なので
        // 同じ種別で返す (web はこの種別で権限を取り直し、チェックボックスを無効にする)
        DiscordError::GuildGone => {
            ApiError::BotPermission("the bot is no longer in this guild".into())
        }
        DiscordError::Status { status, .. } if *status == reqwest::StatusCode::FORBIDDEN => {
            // 利用者自身の権限不足 (Forbidden) と区別する: 直すには Bot の再招待が要る。
            // 権限のキャッシュが古いと、UI で有効なまま保存時にここへ来ることがある
            ApiError::BotPermission(
                "the bot lacks the Create Events permission in this guild".into(),
            )
        }
        DiscordError::Status { status, .. } if *status == reqwest::StatusCode::BAD_REQUEST => {
            ApiError::BadRequest("Discord did not accept the scheduled event".into())
        }
        _ => err.into(),
    }
}

/// スケジュールイベントの**作成**が失敗したときのエラー変換 (#94)。
///
/// Discord から応答を受け取れなかった場合 (接続の切断・タイムアウト)、要求自体は受理されていて
/// イベントだけが残っている可能性がある。採番された ID が分からないので api からは消せず、
/// 利用者には失敗として返すしかないため、後から手作業で片付けられるようログに残す
/// (HTTP のステータスが返ってきた失敗は結果が確定しているので、この扱いはしない)
fn describe_create_error(guild_id: &str, name: &str, err: DiscordError) -> ApiError {
    if matches!(err, DiscordError::Http(_)) {
        tracing::warn!(guild_id, name, error = %err, "could not confirm whether a scheduled event was created: it may be left in discord");
    }
    describe_scheduled_event_error(err)
}

/// 保存できなかったつもりの Discord イベントを消す (#94)。
///
/// `COMMIT` の応答を受け取れなかった場合、DB 側では確定していることがある。そのまま消すと
/// 保存された予定が「消えた Discord イベント」を指してしまうので、**別の接続で対応付けを
/// 確かめてから**消す。確かめられないときは消さない (孤児のイベントが残る方が、
/// DB が実在しない ID を指すより直しやすい)
async fn cleanup_unsaved_scheduled_event(
    state: &AppState,
    guild_id: &str,
    scheduled_event_id: &str,
) {
    match event_links::exists_by_scheduled_event_id(&state.pool, guild_id, scheduled_event_id).await
    {
        Ok(false) => cleanup_scheduled_event(&state.discord, guild_id, scheduled_event_id).await,
        Ok(true) => {
            tracing::warn!(
                guild_id,
                scheduled_event_id,
                "the event was saved after all (the commit result was lost): keeping the scheduled event"
            );
        }
        Err(err) => {
            tracing::warn!(guild_id, scheduled_event_id, error = %err, "could not check whether the link was saved: leaving the scheduled event in discord");
        }
    }
}

/// この更新の内容が DB に入っているか (`COMMIT` の応答が失われたときの照合、#94)。
/// 値と対応付けの両方がこの更新の結果と一致していれば「確定していた」とみなす
/// (一致しなければ、確定していないか、後から別の更新が入っている)
async fn update_was_applied(
    state: &AppState,
    guild_id: &str,
    event_id: i32,
    input: &EventInput,
    desired: &Option<String>,
) -> sqlx::Result<bool> {
    let Some(row) = events::find_by_id(&state.pool, guild_id, event_id).await? else {
        return Ok(false);
    };
    Ok(row.name == input.name
        && row.description == input.description
        && row.color == input.color
        && row.is_all_day == input.is_all_day
        && row.start_at == input.start_at
        && row.end_at == input.end_at
        && row.discord_scheduled_event_id == *desired)
}

/// 予定を保存できなかったときに、先に済ませた Discord への反映を取り消す (#94。すべてベストエフォート)。
///
/// - `created_now` (この試行で作ったイベント) は削除する
/// - 既存のイベントに変更 (PATCH) を送っていたなら、変更前の値で PATCH し直す
///   (これをしないと、予定は元の値のまま Discord だけ新しい内容になって食い違う)
///
/// どちらも、`COMMIT` の応答だけ失われて実際には保存されている場合があるので、
/// **別の接続で保存されていないことを確かめてから**行う (確かめられないときは触らない)。
/// 取り消し自体が失敗したときは warn ログだけ残す (次の編集で改めて反映される)
async fn undo_scheduled_event_changes(
    state: &AppState,
    guild_id: &str,
    event_id: i32,
    input: &EventInput,
    created_now: &Option<String>,
    desired: &Option<String>,
    before: &events::EventRow,
) {
    if let Some(sid) = created_now {
        cleanup_unsaved_scheduled_event(state, guild_id, sid).await;
        return;
    }
    // 作っていないのに desired があるのは、既存のイベントを変更 (PATCH) した場合だけ
    let Some(sid) = desired else { return };
    // 実は保存されていたなら、戻すと DB (新しい内容) と Discord (古い内容) が食い違う
    match update_was_applied(state, guild_id, event_id, input, desired).await {
        Ok(true) => {
            tracing::warn!(guild_id, event_id, scheduled_event_id = %sid, "the event was updated after all (the commit result was lost): keeping the scheduled event as is");
            return;
        }
        Err(err) => {
            tracing::warn!(guild_id, event_id, scheduled_event_id = %sid, error = %err, "could not check whether the update was applied: leaving the scheduled event as is");
            return;
        }
        Ok(false) => {}
    }
    // 終日予定は終了日の翌日を送るので、翌日が無い値は組み立てられない
    // (`validate_discord_flag` が弾いた値が管理コンソール経由などで残っていた場合の保険)
    if before.is_all_day && before.end_at.date().succ_opt().is_none() {
        tracing::warn!(guild_id, scheduled_event_id = %sid, "skipped restoring a scheduled event: the end date cannot be converted");
        return;
    }
    let payload = ScheduledEventPayload::new(
        &state.site_base_url,
        guild_id,
        &before.name,
        before.description.as_deref(),
        before.is_all_day,
        before.start_at,
        before.end_at,
    );
    if let Err(err) = state
        .discord
        .modify_scheduled_event(guild_id, sid, &payload)
        .await
    {
        tracing::warn!(guild_id, scheduled_event_id = %sid, error = %err, "failed to restore a scheduled event to its previous values");
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

#[cfg(test)]
mod tests {
    use super::{JOINED_MAX_IDS, parse_guild_ids};
    use crate::error::ApiError;

    #[test]
    fn parse_guild_ids_trims_and_dedupes_in_order() {
        let ids = parse_guild_ids(" 200000000000000002, 200000000000000001 ,,200000000000000002")
            .unwrap();
        assert_eq!(ids, ["200000000000000002", "200000000000000001"]);
    }

    #[test]
    fn parse_guild_ids_accepts_empty() {
        assert!(parse_guild_ids("").unwrap().is_empty());
        assert!(parse_guild_ids(" , ").unwrap().is_empty());
    }

    #[test]
    fn parse_guild_ids_rejects_non_snowflakes() {
        for raw in ["abc", "200000000000000001,@me", "1/2", "-1"] {
            assert!(
                matches!(parse_guild_ids(raw), Err(ApiError::BadRequest(_))),
                "{raw}"
            );
        }
    }

    #[test]
    fn parse_guild_ids_rejects_too_many() {
        let ok = (0..JOINED_MAX_IDS)
            .map(|i| format!("2{i:017}"))
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(parse_guild_ids(&ok).unwrap().len(), JOINED_MAX_IDS);
        // 重複は数に入れない
        let dup = format!("{ok},200000000000000000");
        assert_eq!(parse_guild_ids(&dup).unwrap().len(), JOINED_MAX_IDS);
        let too_many = format!("{ok},300000000000000000");
        assert!(matches!(
            parse_guild_ids(&too_many),
            Err(ApiError::BadRequest(_))
        ));
    }
}
