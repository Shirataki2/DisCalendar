use sqlx::PgPool;

use super::notifications::Notification;

/// ギルドの設定 (`guild_config` テーブル、web のサーバー設定ダイアログが書く)。未設定なら既定値
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildConfig {
    /// true の場合、予定の作成・編集・削除を管理権限 (管理者 / サーバー管理 / メッセージの管理 / ロールの管理) を
    /// 持つユーザーに限定する
    pub restricted: bool,
    /// 予定の開始時刻に通知するか (#181)。false なら通知タスクは「0 分前」を自動で足さず、
    /// 予定に保存された事前通知だけを送る
    pub notify_at_start: bool,
    /// web で予定を新規作成するときの事前通知の初期値 (#181)。Bot の `/create` では使わない
    /// (引数で指定しなければ事前通知なし、のままにしておく)
    pub default_notifications: Vec<Notification>,
}

impl Default for GuildConfig {
    fn default() -> Self {
        Self {
            restricted: false,
            notify_at_start: true,
            // 行が無いギルドの既定値。api (`models::guilds::DEFAULT_NOTIFICATIONS`) と同じ
            default_notifications: vec![
                Notification::new(1, super::notifications::NotificationUnit::Days),
                Notification::new(1, super::notifications::NotificationUnit::Hours),
            ],
        }
    }
}

/// ギルドの設定。行が無ければ既定値 (restricted = false、開始時刻に通知する)
pub async fn get(pool: &PgPool, guild_id: &str) -> sqlx::Result<GuildConfig> {
    let row = sqlx::query!(
        "SELECT restricted, notify_at_start, default_notifications FROM guild_config WHERE guild_id = $1",
        guild_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map_or_else(GuildConfig::default, |row| GuildConfig {
        restricted: row.restricted,
        notify_at_start: row.notify_at_start,
        default_notifications: Notification::decode_all(&row.default_notifications),
    }))
}

/// ギルドの restricted モード。未設定なら false
pub async fn is_restricted(pool: &PgPool, guild_id: &str) -> sqlx::Result<bool> {
    Ok(get(pool, guild_id).await?.restricted)
}
