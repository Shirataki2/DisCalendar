use std::{collections::HashSet, sync::Arc, time::Instant};

use sqlx::PgPool;

use crate::{auth::AuthConfig, discord::DiscordClient};

/// 全ハンドラで共有する状態 (`web::Data<AppState>`)
pub struct AppState {
    pub pool: PgPool,
    /// SQL コンソール (#36) 専用のプール。権限を絞ったロール `discalendar_sql_console_<DB 名>` でログインする
    /// (`models::admin_sql`)。接続は遅延なので、ロールが無い環境でも起動はできる
    pub sql_console_pool: PgPool,
    /// SQL コンソールの監査用の伏せ字で残してよい既知の単語 (`admin_sql::load_known_words`) のキャッシュ
    pub sql_known_words: tokio::sync::Mutex<Option<KnownWords>>,
    pub discord: DiscordClient,
    /// web の公開 URL (`Config::site_base_url`)。Discord スケジュールイベントの「場所」リンクに使う (#94)
    pub site_base_url: String,
    /// Discord 連携が絡む予定の更新を予定単位で直列化するロック (#94)。
    /// Discord への反映はトランザクションの外で行うため、同じ予定への並行更新は
    /// 反映順と commit 順が食い違いうる。ここで外部反映から書き込みまでを直列化する
    /// (api は 1 プロセスで動かす前提。複数プロセスに分けるならこのロックでは足りない)
    pub event_update_locks: EventUpdateLocks,
    pub auth: AuthConfig,
    /// 日次アクティビティ (#81) を同じ日に何度も書きに行かないためのキャッシュ
    /// (Better Auth の user.id → 記録済みの JST の日付)
    pub activity_days: moka::future::Cache<String, chrono::NaiveDate>,
    pub admin: AdminConfig,
    /// プロセスの起動時刻 (`GET /admin/status` の稼働時間、#37)
    pub started_at: chrono::DateTime<chrono::Utc>,
}

/// [`AppState::event_update_locks`] の実体。予定 ID のハッシュで固定本数のロックを引く
/// (ストライプ方式)。別の予定が同じロックに当たることがあるが、待ちが増えるだけで
/// 整合性には影響しない。固定本数なので予定が増えてもメモリは増えない
pub struct EventUpdateLocks {
    stripes: [tokio::sync::Mutex<()>; Self::STRIPES],
}

impl Default for EventUpdateLocks {
    fn default() -> Self {
        Self {
            stripes: std::array::from_fn(|_| tokio::sync::Mutex::new(())),
        }
    }
}

impl EventUpdateLocks {
    const STRIPES: usize = 64;

    /// 予定に対応するロックを取る (解放は戻り値の drop)
    pub async fn lock(&self, event_id: i32) -> tokio::sync::MutexGuard<'_, ()> {
        self.stripes[event_id.unsigned_abs() as usize % Self::STRIPES]
            .lock()
            .await
    }
}

/// 管理コンソール (`/admin/*`) の設定
#[derive(Debug, Clone, Default)]
pub struct AdminConfig {
    /// 管理者として扱う Discord ユーザー ID (`ADMIN_DISCORD_USER_IDS`)。空なら誰も管理者ではない
    pub discord_user_ids: HashSet<String>,
}

impl AdminConfig {
    pub fn is_admin(&self, discord_user_id: &str) -> bool {
        self.discord_user_ids.contains(discord_user_id)
    }
}

/// [`AppState::sql_known_words`] のキャッシュ 1 世代
#[derive(Debug, Clone)]
pub struct KnownWords {
    pub words: Arc<HashSet<String>>,
    pub loaded_at: Instant,
}
