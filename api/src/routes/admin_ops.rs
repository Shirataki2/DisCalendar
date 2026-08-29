//! 管理コンソールの定型操作 (`/admin/ops/*`、#36)。
//!
//! 書き込みを伴う操作は自由 SQL ではなくここに定型として用意する (#33 の検討)。
//! どれも [`AdminUser`] を要求し、操作と同じトランザクションで `admin_audit_logs` に記録する。
//! 予定 1 件の編集・削除と restricted の切替は `/admin/guilds/*` (#35) にある。

use actix_web::{post, web};
use futures_util::{StreamExt as _, stream};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::admin_guilds::{ensure_guild_known, snapshot, validated_guild_id};
use crate::{
    admin::AdminUser,
    error::{ApiError, ErrorBody},
    models::{
        admin_audit::{self, AuditEntry},
        admin_ops, event_links,
        events::Event,
    },
    state::AppState,
};

#[derive(Deserialize, ToSchema)]
pub struct GuildTarget {
    /// 対象のギルド ID
    #[schema(example = "782502586817314816")]
    pub guild_id: String,
}

/// 全予定削除の監査ログに残す 1 件分 (API 形式の予定 + `events.notifications` の生データ)
#[derive(Serialize)]
struct EventSnapshot {
    #[serde(flatten)]
    event: Event,
    /// DB に入っていた `notifications` そのまま (`Event` への変換で捨てられる要素も含む)
    raw_notifications: serde_json::Value,
}

/// 定型操作の結果
#[derive(Serialize, ToSchema)]
pub struct OpsResult {
    /// 削除した行数
    #[schema(example = 12)]
    pub deleted: u64,
}

/// 指定ギルドの予定をすべて削除する。削除した予定は監査ログ (`ops.delete_guild_events`) の `before.events` に
/// 先頭 200 件まで残し (`before.omitted` に残りの件数)、`detail.deleted` に削除件数を記録する。
/// どのテーブルにも無いギルド ID (打ち間違い) は 404
#[utoipa::path(
    tag = "admin",
    request_body = GuildTarget,
    responses(
        (status = 200, body = OpsResult),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 404, description = "存在しない (したことのない) ギルド", body = ErrorBody),
    )
)]
#[post("/ops/delete-guild-events")]
pub async fn delete_guild_events(
    admin: AdminUser,
    body: web::Json<GuildTarget>,
    state: web::Data<AppState>,
) -> Result<web::Json<OpsResult>, ApiError> {
    let guild_id = validated_guild_id(&body.guild_id)?;
    let mut tx = state.pool.begin().await?;
    ensure_guild_known(&mut *tx, guild_id).await?;
    // 連携している Discord スケジュールイベントを控えておく (#94)。
    // 控えてから削除するまでに連携が増えて取り残さないよう、まずギルド単位の勧告ロックで
    // 連携付きの新規作成 (新しい行は行ロックでは待たせられない) と排他し、
    // 次に既存の予定行をロックして連携の追加・変更・解除を待たせる。
    // 対応付けの行自体は events の削除に CASCADE で追随する
    event_links::lock_guild(&mut *tx, guild_id).await?;
    admin_ops::lock_guild_events(&mut *tx, guild_id).await?;
    let scheduled_event_ids = event_links::list_scheduled_event_ids(&mut *tx, guild_id).await?;
    let (snapshot_rows, count) = admin_ops::delete_guild_events(&mut tx, guild_id).await?;
    // スナップショットは API 形式 (Event) に加えて notifications の生データも残す
    // (壊れた要素は Event への変換で捨てられるため、削除した実データを復元・調査できるように)
    let sampled: Vec<EventSnapshot> = snapshot_rows
        .into_iter()
        .map(|row| EventSnapshot {
            raw_notifications: row.notifications.clone(),
            event: Event::from(row),
        })
        .collect();
    let omitted = count.saturating_sub(sampled.len() as u64);
    admin_audit::record(
        &mut *tx,
        &admin,
        AuditEntry {
            action: "ops.delete_guild_events",
            target_type: Some("guild"),
            target_id: Some(guild_id),
            before: Some(serde_json::json!({
                "events": snapshot(&sampled)?,
                "omitted": omitted,
            })),
            detail: Some(serde_json::json!({
                "deleted": count,
                "snapshot_limit": admin_ops::DELETE_SNAPSHOT_LIMIT,
            })),
            ..Default::default()
        },
    )
    .await?;
    if let Err(err) = tx.commit().await {
        // COMMIT の応答だけ失われて実際には消えていることがある。何もしないと Discord の
        // イベントだけ残り、対応付けも消えているので後から ID を回収できない。
        // 別の接続で対応付けが空になったことを確かめられたときだけ Discord 側も片付ける
        if matches!(
            event_links::list_scheduled_event_ids(&state.pool, guild_id).await,
            Ok(ref remaining) if remaining.is_empty()
        ) {
            tracing::warn!(
                guild_id,
                "the events were deleted after all (the commit result was lost): cleaning up the scheduled events"
            );
            delete_scheduled_events(&state, guild_id, &scheduled_event_ids).await;
        }
        return Err(err.into());
    }
    delete_scheduled_events(&state, guild_id, &scheduled_event_ids).await;
    tracing::info!(guild_id, deleted = count, admin = %admin.discord_user_id, "all events of a guild deleted by admin");
    Ok(web::Json(OpsResult { deleted: count }))
}

/// 一括削除に伴う Discord 側の後始末 (#94)。管理コンソールの削除は Discord 側の失敗で
/// 止めない (ベストエフォート)。1 件ずつ直列に待つと件数に比例して応答が遅くなるので、
/// 数件ずつ並行で呼ぶ (レート制限に当たりにくい程度の並行数に抑える)
async fn delete_scheduled_events(state: &AppState, guild_id: &str, scheduled_event_ids: &[String]) {
    let discord = &state.discord;
    stream::iter(scheduled_event_ids)
        .for_each_concurrent(4, |scheduled_event_id| async move {
            if let Err(err) = discord
                .delete_scheduled_event(guild_id, scheduled_event_id)
                .await
            {
                tracing::warn!(guild_id, scheduled_event_id, error = %err, "failed to delete a linked scheduled event");
            }
        })
        .await;
}

/// Better Auth の期限切れセッション (`session."expiresAt" < now()`) を削除する。
/// 有効なセッションには触らないのでログアウトされる人はいない。監査ログは `ops.purge_expired_sessions`
#[utoipa::path(
    tag = "admin",
    responses(
        (status = 200, body = OpsResult),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[post("/ops/purge-expired-sessions")]
pub async fn purge_expired_sessions(
    admin: AdminUser,
    state: web::Data<AppState>,
) -> Result<web::Json<OpsResult>, ApiError> {
    let mut tx = state.pool.begin().await?;
    let deleted = admin_ops::purge_expired_sessions(&mut *tx).await?;
    admin_audit::record(
        &mut *tx,
        &admin,
        AuditEntry {
            action: "ops.purge_expired_sessions",
            target_type: Some("session"),
            detail: Some(serde_json::json!({ "deleted": deleted })),
            ..Default::default()
        },
    )
    .await?;
    tx.commit().await?;
    tracing::info!(deleted, admin = %admin.discord_user_id, "expired sessions purged by admin");
    Ok(web::Json(OpsResult { deleted }))
}
