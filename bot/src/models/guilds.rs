use sqlx::PgPool;

use crate::models::now_jst;

/// Bot が参加しているギルド (`guilds` テーブル)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guild {
    pub guild_id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub locale: String,
}

pub async fn find_by_guild_id(pool: &PgPool, guild_id: &str) -> sqlx::Result<Option<Guild>> {
    sqlx::query_as!(
        Guild,
        "SELECT guild_id, name, avatar_url, locale FROM guilds WHERE guild_id = $1",
        guild_id
    )
    .fetch_optional(pool)
    .await
}

/// 参加時・更新時に名前とアイコンを反映する。`locale` は新規行だけ既定値 (`ja`) を入れ、既存行の値は上書きしない。
///
/// `joined_at` には現在時刻を入れる。既存行は `refresh_joined_at` のとき (= 新規参加のイベントで、
/// 退出直後の再参加により行が残っていた場合) だけ上書きし、名前・アイコンの更新では触らない。
/// 起動時の登録 (停止中に参加していたギルドの取りこぼし回復) で挿入される行は、
/// 実際の参加日時が分からないため `joined_at` は登録時刻の近似になる
pub async fn upsert(
    pool: &PgPool,
    guild_id: &str,
    name: &str,
    avatar_url: Option<&str>,
    refresh_joined_at: bool,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO guilds (guild_id, name, avatar_url, joined_at) VALUES ($1, $2, $3, $4)
        ON CONFLICT (guild_id) DO UPDATE SET
            name = EXCLUDED.name,
            avatar_url = EXCLUDED.avatar_url,
            joined_at = CASE WHEN $5 THEN EXCLUDED.joined_at ELSE guilds.joined_at END
        "#,
        guild_id,
        name,
        avatar_url,
        now_jst(),
        refresh_joined_at
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// 登録済みのギルド ID 一覧
pub async fn list_ids(pool: &PgPool) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar!("SELECT guild_id FROM guilds")
        .fetch_all(pool)
        .await
}

/// 退出時に行を消す。消した行があれば true
pub async fn delete(pool: &PgPool, guild_id: &str) -> sqlx::Result<bool> {
    let result = sqlx::query!("DELETE FROM guilds WHERE guild_id = $1", guild_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// 指定した ID の行をまとめて消す (停止中に退出したギルドの掃除用)。消した行数を返す
pub async fn delete_many(pool: &PgPool, guild_ids: &[String]) -> sqlx::Result<u64> {
    let result = sqlx::query!("DELETE FROM guilds WHERE guild_id = ANY($1)", guild_ids)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
