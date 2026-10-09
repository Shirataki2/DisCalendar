//! 予定の変更履歴 (#165)。api / bot 共通。
//!
//! 予定の書き込みと同じトランザクションの中で [`record`] を呼び、1 操作につき 1 行を残す
//! (書き込みが失敗・ロールバックしたら履歴も残らない)。
//! スナップショットは SQL の中で `events` の行から組み立てるので、api と bot で中身の形が揃う。
//! 日時は `events` と同じ JST naive の文字列 (`YYYY-MM-DDTHH:MM:SS`)、`notifications` は
//! `events.notifications` の JSONB (`[{num, unit}]`) をそのまま入れる
use serde_json::Value;
use sqlx::{PgConnection, PgPool};

/// 予定ごとに残す件数。超えた分は古いものから [`record`] の中で消す
pub const MAX_PER_EVENT: i64 = 100;
/// 保持期間。過ぎた行は bot の定期タスクが [`prune_expired`] で消す
pub const RETENTION_DAYS: i32 = 180;

/// 予定 1 件 (`events e`) のスナップショットを作る式。`discord_linked` は Discord スケジュールイベントとの連携 (#94) の有無
macro_rules! snapshot_sql {
    () => {
        "jsonb_build_object(
            'name', e.name,
            'description', e.description,
            'location', e.location,
            'color', e.color,
            'is_all_day', e.is_all_day,
            'start_at', to_char(e.start_at, 'YYYY-MM-DD\"T\"HH24:MI:SS'),
            'end_at', to_char(e.end_at, 'YYYY-MM-DD\"T\"HH24:MI:SS'),
            'notifications', e.notifications,
            'notification_mentions', e.notification_mentions,
            'discord_linked', EXISTS (SELECT 1 FROM event_discord_links l WHERE l.event_id = e.id)
        )"
    };
}

/// どこからの操作か
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// 通常の画面・API (web)
    Web,
    /// Bot のスラッシュコマンド
    Bot,
    /// 管理コンソール
    Admin,
    /// AI アシスタント (MCP)
    Mcp,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Bot => "bot",
            Self::Admin => "admin",
            Self::Mcp => "mcp",
        }
    }
}

/// 何をしたか
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Create,
    Update,
    /// 予定の削除は今は物理削除で、履歴も `ON DELETE CASCADE` で一緒に消えるため記録しない。
    /// ゴミ箱 (#159) で論理削除になったら、削除・復元の各経路から記録する
    Delete,
    Restore,
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Restore => "restore",
        }
    }
}

/// 予定の今のスナップショット。更新・削除の前に読んで [`record`] の `before` に渡す。
/// ギルドに属さない (または存在しない) 予定なら `None`
pub async fn snapshot(
    conn: &mut PgConnection,
    guild_id: &str,
    event_id: i32,
) -> sqlx::Result<Option<Value>> {
    sqlx::query_scalar(concat!(
        "SELECT ",
        snapshot_sql!(),
        " FROM events e WHERE e.id = $1 AND e.guild_id = $2"
    ))
    .bind(event_id)
    .bind(guild_id)
    .fetch_optional(conn)
    .await
}

/// 履歴を 1 行残す。`after` は書き込み後の予定から作る (delete では NULL)。
/// 呼び出し元の書き込み (と Discord 連携の対応付けの変更) を済ませてから、同じトランザクションで呼ぶ。
/// 内容の変わらない更新 (`before` と同じ) は残さない。
/// 予定ごとの上限 ([`MAX_PER_EVENT`]) を超えた古い行もここで消す
pub async fn record(
    conn: &mut PgConnection,
    guild_id: &str,
    event_id: i32,
    actor_discord_user_id: Option<&str>,
    source: Source,
    action: Action,
    before: Option<&Value>,
) -> sqlx::Result<()> {
    let inserted = sqlx::query(concat!(
        "INSERT INTO event_history (event_id, guild_id, actor_discord_user_id, source, action, before, after)
         SELECT e.id, e.guild_id, $3, $4, $5, $6, CASE WHEN $5 = 'delete' THEN NULL ELSE ",
        snapshot_sql!(),
        " END
         FROM events e
         WHERE e.id = $1 AND e.guild_id = $2
           AND ($5 <> 'update' OR $6::jsonb IS DISTINCT FROM ",
        snapshot_sql!(),
        ")"
    ))
    .bind(event_id)
    .bind(guild_id)
    .bind(actor_discord_user_id)
    .bind(source.as_str())
    .bind(action.as_str())
    .bind(before)
    .execute(&mut *conn)
    .await?;
    if inserted.rows_affected() > 0 {
        sqlx::query(
            "DELETE FROM event_history
             WHERE event_id = $1
               AND id <= (SELECT id FROM event_history WHERE event_id = $1 ORDER BY id DESC OFFSET $2 LIMIT 1)",
        )
        .bind(event_id)
        .bind(MAX_PER_EVENT)
        .execute(conn)
        .await?;
    }
    Ok(())
}

/// 保持期間 ([`RETENTION_DAYS`]) を過ぎた履歴を消し、消した行数を返す
pub async fn prune_expired(pool: &PgPool) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "DELETE FROM event_history WHERE created_at < now() - make_interval(days => $1)",
    )
    .bind(RETENTION_DAYS)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
