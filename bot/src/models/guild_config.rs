use sqlx::PgPool;

use super::notifications::Notification;

/// ギルドの設定 (`guild_config` テーブル、web のサーバー設定ダイアログが書く)。未設定なら既定値
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildConfig {
    /// true の場合、予定の作成・編集・削除を管理権限 (管理者 / サーバー管理 / メッセージの管理 / ロールの管理) を
    /// 持つユーザー、または編集ロールを持つユーザーに限定する
    pub restricted: bool,
    pub editor_role_ids: Vec<String>,
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
            editor_role_ids: vec![],
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
        "SELECT restricted, editor_role_ids, notify_at_start, default_notifications FROM guild_config WHERE guild_id = $1",
        guild_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map_or_else(GuildConfig::default, |row| GuildConfig {
        restricted: row.restricted,
        editor_role_ids: row.editor_role_ids,
        notify_at_start: row.notify_at_start,
        default_notifications: Notification::decode_all(&row.default_notifications),
    }))
}

impl GuildConfig {
    /// API と同じ編集判定。ロール ID は DB と照合するときも文字列で扱う。
    pub fn can_edit_events(&self, can_manage_server: bool, roles: &[String]) -> bool {
        !self.restricted
            || can_manage_server
            || roles.iter().any(|id| self.editor_role_ids.contains(id))
    }
}
