use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgExecutor, PgPool};
use utoipa::ToSchema;

/// Bot が参加しているギルド (`guilds` テーブル。Bot が参加/更新時に書き込む)
#[derive(Debug, Serialize, ToSchema)]
pub struct Guild {
    #[schema(example = "782502586817314816")]
    pub guild_id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    #[schema(example = "ja")]
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

/// 指定した ID のうち Bot が参加しているギルド
pub async fn find_joined(pool: &PgPool, guild_ids: &[String]) -> sqlx::Result<Vec<Guild>> {
    sqlx::query_as!(
        Guild,
        "SELECT guild_id, name, avatar_url, locale FROM guilds WHERE guild_id = ANY($1) ORDER BY name",
        guild_ids
    )
    .fetch_all(pool)
    .await
}

/// ギルドごとの設定 (`guild_config` テーブル)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GuildConfig {
    #[schema(example = "782502586817314816")]
    pub guild_id: String,
    /// true の場合、予定の追加・編集・削除を管理権限
    /// (管理者 / サーバー管理 / メッセージの管理 / ロールの管理) を持つユーザーに限定する
    pub restricted: bool,
}

/// 未設定なら既定値 (restricted = false) を返す。読み取りでは行を作らない
pub async fn get_config<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
) -> sqlx::Result<GuildConfig> {
    let config = sqlx::query_as!(
        GuildConfig,
        "SELECT guild_id, restricted FROM guild_config WHERE guild_id = $1",
        guild_id
    )
    .fetch_optional(executor)
    .await?;
    Ok(config.unwrap_or_else(|| GuildConfig {
        guild_id: guild_id.to_owned(),
        restricted: false,
    }))
}

/// 監査ログの「変更前」として設定を読み、同じトランザクションの終わりまで行をロックする。
/// 行がまだ無いギルドでは `FOR UPDATE` だけだと何もロックできず、読み取りから upsert までの間に
/// 通常 API や別の管理リクエストが最初の行を作ると before (既定値) と実際の直前値がずれるので、
/// 先に既定値の行を `INSERT ... ON CONFLICT DO NOTHING` で確保してから `FOR UPDATE` で読む
/// (並行する upsert はこのトランザクションの完了まで行ロックで待つ。トランザクションを
/// ロールバックすれば確保した行も消える)。トランザクション内で呼ぶこと
pub async fn lock_config_for_update(
    conn: &mut PgConnection,
    guild_id: &str,
) -> sqlx::Result<GuildConfig> {
    sqlx::query!(
        "INSERT INTO guild_config (guild_id, restricted) VALUES ($1, false) ON CONFLICT (guild_id) DO NOTHING",
        guild_id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query_as!(
        GuildConfig,
        "SELECT guild_id, restricted FROM guild_config WHERE guild_id = $1 FOR UPDATE",
        guild_id
    )
    .fetch_one(&mut *conn)
    .await
}

/// 管理コンソールからは監査ログと同じトランザクションで呼べるよう executor を受け取る
pub async fn upsert_config<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    restricted: bool,
) -> sqlx::Result<GuildConfig> {
    sqlx::query_as!(
        GuildConfig,
        r#"
        INSERT INTO guild_config (guild_id, restricted) VALUES ($1, $2)
        ON CONFLICT (guild_id) DO UPDATE SET restricted = EXCLUDED.restricted
        RETURNING guild_id, restricted
        "#,
        guild_id,
        restricted
    )
    .fetch_one(executor)
    .await
}
