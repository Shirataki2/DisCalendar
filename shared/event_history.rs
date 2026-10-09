//! 予定の変更履歴 (#165)。api / bot 共通。
//!
//! 予定の書き込みと同じトランザクションの中で [`record`] を呼び、1 操作につき 1 行を残す
//! (書き込みが失敗・ロールバックしたら履歴も残らない)。
//! スナップショットは SQL の中で `events` の行から組み立てるので、api と bot で中身の形が揃う。
//! 日時は `events` と同じ JST naive の文字列 (`YYYY-MM-DDTHH:MM:SS`)、`notifications` は
//! `events.notifications` の JSONB (`[{num, unit}]`) をそのまま入れる。
//! 説明は本文もハッシュも残さず、更新の after に「説明が変わったか」(`description_changed`) だけを入れる。
//! 履歴はメンバー全員が読めるので、説明から消した内容を履歴から取り出せないようにするため
//! (短い合言葉などは、無ソルトのハッシュでも候補を総当たりすれば分かってしまう)。
//! 比べるためのハッシュ (`description_hash`) はスナップショットを読んでから書くまでの間だけ持ち、保存前に取り除く
use serde_json::Value;
use sqlx::{PgConnection, PgPool};

/// 予定ごとに残す件数。超えた分は古いものから [`record`] の中で消す
pub const MAX_PER_EVENT: i64 = 100;
/// 保持期間。過ぎた行は bot の定期タスクが [`prune_expired`] で消す
pub const RETENTION_DAYS: i32 = 180;

/// 予定 1 件 (`events e`) のスナップショットを作る式。`discord_linked` は Discord スケジュールイベントとの連携 (#94) の有無。
/// `recurrence` は所属するシリーズの繰り返し条件 (単発なら null)。回数で終わる条件は回数を除く:
/// 「この回以降」の分割では、回数を変えなくても消化済みの回を引いた残り回数がシリーズに入るため、
/// 回数まで比べると利用者が変えていない変更が履歴に出てしまう
macro_rules! snapshot_sql {
    () => {
        "jsonb_build_object(
            'name', e.name,
            'description_hash', md5(e.description),
            'location', e.location,
            'color', e.color,
            'is_all_day', e.is_all_day,
            'start_at', to_char(e.start_at, 'YYYY-MM-DD\"T\"HH24:MI:SS'),
            'end_at', to_char(e.end_at, 'YYYY-MM-DD\"T\"HH24:MI:SS'),
            'notifications', e.notifications,
            'notification_mentions', e.notification_mentions,
            'discord_linked', EXISTS (SELECT 1 FROM event_discord_links l WHERE l.event_id = e.id),
            'recurrence', (
                SELECT CASE WHEN s.recurrence->'end'->>'type' = 'count'
                            THEN jsonb_set(s.recurrence, '{end}', '{\"type\": \"count\"}')
                            ELSE s.recurrence END
                FROM event_series s WHERE s.id = e.series_id
            )
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
    Ok(snapshots(conn, guild_id, &[event_id])
        .await?
        .into_iter()
        .next()
        .map(|(_, value)| value))
}

/// 複数の予定の今のスナップショット (繰り返し予定の「この回以降」の変更など、1 操作で複数の回が変わるとき)。
/// ギルドに属さない予定は含まない
pub async fn snapshots(
    conn: &mut PgConnection,
    guild_id: &str,
    event_ids: &[i32],
) -> sqlx::Result<Vec<(i32, Value)>> {
    sqlx::query_as(concat!(
        "SELECT e.id, ",
        snapshot_sql!(),
        " FROM events e WHERE e.id = ANY($1) AND e.guild_id = $2"
    ))
    .bind(event_ids)
    .bind(guild_id)
    .fetch_all(conn)
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
    record_many(
        conn,
        guild_id,
        &[(event_id, before.cloned())],
        actor_discord_user_id,
        source,
        action,
    )
    .await
}

/// [`record`] の複数件版。`entries` は予定 ID と、その予定の変更前のスナップショット ([`snapshots`] で読んだもの)。
/// 消えた予定 (この操作で削除された回) と内容の変わらない予定は残さない
pub async fn record_many(
    conn: &mut PgConnection,
    guild_id: &str,
    entries: &[(i32, Option<Value>)],
    actor_discord_user_id: Option<&str>,
    source: Source,
    action: Action,
) -> sqlx::Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let (ids, befores): (Vec<i32>, Vec<Option<Value>>) = entries.iter().cloned().unzip();
    // 説明のハッシュは比べるためだけに使い、保存する before / after からは取り除く
    let recorded: Vec<i32> = sqlx::query_scalar(concat!(
        "WITH changed AS (
             SELECT e.id, e.guild_id, b.before, ",
        snapshot_sql!(),
        " AS snap
             FROM UNNEST($1::int[], $6::jsonb[]) AS b(event_id, before)
             JOIN events e ON e.id = b.event_id AND e.guild_id = $2
         )
         INSERT INTO event_history (event_id, guild_id, actor_discord_user_id, source, action, before, after)
         SELECT c.id, c.guild_id, $3, $4, $5, c.before - 'description_hash',
                CASE WHEN $5 = 'delete' THEN NULL
                     ELSE (c.snap - 'description_hash') || jsonb_build_object(
                         'description_changed',
                         c.before IS NOT NULL AND c.before->'description_hash' IS DISTINCT FROM c.snap->'description_hash')
                END
         FROM changed c
         WHERE $5 <> 'update' OR c.before IS DISTINCT FROM c.snap
         RETURNING event_id"
    ))
    .bind(&ids)
    .bind(guild_id)
    .bind(actor_discord_user_id)
    .bind(source.as_str())
    .bind(action.as_str())
    .bind(&befores)
    .fetch_all(&mut *conn)
    .await?;
    if !recorded.is_empty() {
        sqlx::query(
            "DELETE FROM event_history h
             USING (
                 SELECT id, row_number() OVER (PARTITION BY event_id ORDER BY id DESC) AS rank
                 FROM event_history WHERE event_id = ANY($1)
             ) r
             WHERE h.id = r.id AND r.rank > $2",
        )
        .bind(&recorded)
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
