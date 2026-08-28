//! 予定と Discord スケジュールイベントの対応付け (`event_discord_links`、#94)。
//!
//! 「Discord のイベントとしても作成する」を有効にした予定 1 件につき 1 行。
//! 行は `events` の削除に CASCADE で追随するが、Discord 側のイベント削除は
//! ルート層が削除前にこの表を読んで明示的に行う。

use chrono::NaiveDateTime;
use sqlx::PgExecutor;

/// 予定に紐付く scheduled_event_id。未連携なら `None`。他ギルドの予定 ID を指定しても返さない
pub async fn get<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    event_id: i32,
) -> sqlx::Result<Option<String>> {
    let row = sqlx::query!(
        "SELECT scheduled_event_id FROM event_discord_links WHERE event_id = $1 AND guild_id = $2",
        event_id,
        guild_id
    )
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|r| r.scheduled_event_id))
}

pub async fn insert<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    event_id: i32,
    scheduled_event_id: &str,
    created_at: NaiveDateTime,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO event_discord_links (event_id, guild_id, scheduled_event_id, created_at)
        VALUES ($1, $2, $3, $4)
        "#,
        event_id,
        guild_id,
        scheduled_event_id,
        created_at
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// 対応先を差し替える (Discord 側で手動削除されたイベントを作り直したとき)
pub async fn set_scheduled_event_id<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    event_id: i32,
    scheduled_event_id: &str,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        UPDATE event_discord_links SET scheduled_event_id = $3
        WHERE event_id = $1 AND guild_id = $2
        "#,
        event_id,
        guild_id,
        scheduled_event_id
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// 対応付けを外す (予定は残す)。消せたら `true`
pub async fn delete<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    event_id: i32,
) -> sqlx::Result<bool> {
    let result = sqlx::query!(
        "DELETE FROM event_discord_links WHERE event_id = $1 AND guild_id = $2",
        event_id,
        guild_id
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// ギルドの対応付けのうち、予定の開始が `starts_after` より後のものの scheduled_event_id。
/// 管理コンソールの一括削除で「開始前の Discord イベントだけ消す」のに使う
pub async fn list_scheduled_event_ids<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    starts_after: NaiveDateTime,
) -> sqlx::Result<Vec<String>> {
    let rows = sqlx::query!(
        r#"
        SELECT l.scheduled_event_id
        FROM event_discord_links l
        JOIN events e ON e.id = l.event_id
        WHERE l.guild_id = $1 AND e.start_at > $2
        "#,
        guild_id,
        starts_after
    )
    .fetch_all(executor)
    .await?;
    Ok(rows.into_iter().map(|r| r.scheduled_event_id).collect())
}
