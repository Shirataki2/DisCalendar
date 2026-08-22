use sqlx::PgPool;

/// ギルドの restricted モード (`guild_config` テーブル、web のサーバー設定ダイアログが書く)。
/// true の場合、予定の作成・編集・削除を管理権限 (管理者 / サーバー管理 / メッセージの管理 / ロールの管理) を
/// 持つユーザーに限定する。未設定なら false
pub async fn is_restricted(pool: &PgPool, guild_id: &str) -> sqlx::Result<bool> {
    let restricted = sqlx::query_scalar!(
        "SELECT restricted FROM guild_config WHERE guild_id = $1",
        guild_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(restricted.unwrap_or(false))
}
