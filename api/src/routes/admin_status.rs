//! 管理コンソールの稼働状況・概要 (`/admin/stats`, `/admin/status`, `/admin/guilds/sync-check`、#37)。
//!
//! 運用で最初に見る画面 (`/admin`) のためのデータ。すべて読み取りだけなので監査ログには残さない。
//! `/admin/guilds/sync-check` は `/admin/guilds/{guild_id}` より先に登録すること
//! (actix は登録順に照合するので、後だと `sync-check` が guild_id として解釈される)。

use std::collections::{HashMap, HashSet};

use actix_web::{get, web};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    admin::AdminUser,
    build_info::{BUILD_INFO, BuildInfo},
    error::{ApiError, ErrorBody},
    models::{
        admin_guilds,
        admin_stats::{self, AdminCounts, LeftGuild, RecentGuild},
        admin_status::{self, MigrationStatus},
        now_jst,
    },
    state::AppState,
};

/// 概要 (件数と直近のギルドの出入り)
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminStats {
    pub counts: AdminCounts,
    /// 今日 (JST) 発火する通知の数。`day_start` から翌 0 時までが対象
    #[schema(example = 3)]
    pub notifications_today: i64,
    /// 集計の基準にした今日の 0 時 (JST)
    #[schema(example = "2026-08-23T00:00:00")]
    pub day_start: NaiveDateTime,
    /// 直近に `guilds` へ登録されたギルド (id の降順)。`guilds` に日時が無いので順序だけが手がかり
    pub recent_guilds: Vec<RecentGuild>,
    /// 退出済み (guilds に行が無い) でデータが残っているギルド。残っている予定の新しい順
    pub left_guilds: Vec<LeftGuild>,
}

/// 概要。運用で最初に見る件数をまとめて返す
#[utoipa::path(
    tag = "admin",
    responses(
        (status = 200, body = AdminStats),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[get("/stats")]
pub async fn stats(
    _admin: AdminUser,
    state: web::Data<AppState>,
) -> Result<web::Json<AdminStats>, ApiError> {
    let now = now_jst();
    let day_start = now.date().and_hms_opt(0, 0, 0).expect("valid midnight");
    let day_end = day_start + Duration::days(1);
    let (counts, notifications_today, recent_guilds, left_guilds) = tokio::try_join!(
        admin_stats::counts(&state.pool, now),
        admin_stats::notifications_between(&state.pool, day_start, day_end),
        admin_stats::recent_guilds(&state.pool),
        admin_stats::left_guilds(&state.pool),
    )?;
    Ok(web::Json(AdminStats {
        counts,
        notifications_today,
        day_start,
        recent_guilds,
        left_guilds,
    }))
}

/// DB の状態
#[derive(Debug, Serialize, ToSchema)]
pub struct DatabaseStatus {
    /// 接続して `SELECT 1` が通ったか
    pub reachable: bool,
    /// 疎通確認にかかった時間
    #[schema(example = 2)]
    pub latency_ms: Option<i64>,
    /// PostgreSQL のバージョン (`SHOW server_version`)
    #[schema(example = "18.0")]
    pub server_version: Option<String>,
    /// 繋がらなかったときの理由
    pub error: Option<String>,
    /// 接続プールの現在のコネクション数
    pub pool_connections: u32,
    /// そのうち空いているもの
    pub pool_idle: u32,
}

/// 稼働状況 (`GET /admin/status`)
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminStatusResponse {
    pub build: BuildInfo,
    /// api プロセスの起動時刻 (UTC)
    pub started_at: DateTime<Utc>,
    #[schema(example = 3600)]
    pub uptime_seconds: i64,
    pub database: DatabaseStatus,
    /// マイグレーションの適用状況。DB に繋がらなければ null
    pub migrations: Option<MigrationStatus>,
    /// マイグレーションの状態に問題があるか (未適用・失敗・チェックサム不一致など)
    pub migrations_ok: bool,
}

/// DB の疎通・マイグレーションの適用状況・ビルド情報。
///
/// 疎通に失敗しても 200 で返し、`database.reachable` を false にして理由を見せる。
/// ただし認証自体が `session` テーブルを引くので、DB が完全に落ちているとこのハンドラまで届かず
/// 500 になる。ここで拾えるのはプールの枯渇や一時的な失敗
#[utoipa::path(
    tag = "admin",
    responses(
        (status = 200, body = AdminStatusResponse),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[get("/status")]
pub async fn status(
    _admin: AdminUser,
    state: web::Data<AppState>,
) -> web::Json<AdminStatusResponse> {
    let started = std::time::Instant::now();
    let ping = sqlx::query("SELECT 1").execute(&state.pool).await;
    let latency_ms = started.elapsed().as_millis() as i64;

    let (database, migrations) = match ping {
        Ok(_) => {
            let server_version = admin_status::server_version(&state.pool).await.ok();
            let migrations = match admin_status::migration_status(&state.pool).await {
                Ok(status) => Some(status),
                Err(error) => {
                    tracing::warn!(error = %error, "failed to read the migration status");
                    None
                }
            };
            (
                DatabaseStatus {
                    reachable: true,
                    latency_ms: Some(latency_ms),
                    server_version,
                    error: None,
                    pool_connections: state.pool.size(),
                    pool_idle: state.pool.num_idle() as u32,
                },
                migrations,
            )
        }
        Err(error) => {
            tracing::warn!(error = %error, "database is not reachable (admin status)");
            (
                DatabaseStatus {
                    reachable: false,
                    latency_ms: None,
                    server_version: None,
                    error: Some(error.to_string()),
                    pool_connections: state.pool.size(),
                    pool_idle: state.pool.num_idle() as u32,
                },
                None,
            )
        }
    };

    let now = Utc::now();
    web::Json(AdminStatusResponse {
        build: BUILD_INFO,
        started_at: state.started_at,
        uptime_seconds: (now - state.started_at).num_seconds().max(0),
        migrations_ok: migrations.as_ref().is_some_and(|m| !m.has_problem()),
        database,
        migrations,
    })
}

/// 差分の 1 件
#[derive(Debug, Serialize, ToSchema)]
pub struct SyncGuild {
    #[schema(example = "782502586817314816")]
    pub guild_id: String,
    /// 分かる方の名前 (`guilds` テーブル または Discord)
    pub name: Option<String>,
}

/// 名前がずれているギルド
#[derive(Debug, Serialize, ToSchema)]
pub struct NameMismatch {
    #[schema(example = "782502586817314816")]
    pub guild_id: String,
    /// `guilds` テーブルの名前
    pub db_name: String,
    /// Discord 側の現在の名前
    pub discord_name: String,
}

/// Bot の参加ギルドと `guilds` テーブルの差分
#[derive(Debug, Serialize, ToSchema)]
pub struct GuildSyncCheck {
    /// Discord 上で Bot が参加しているギルド数
    pub discord_count: i64,
    /// `guilds` テーブルの行数
    pub db_count: i64,
    /// `guilds` にあるが Bot は参加していない (退出イベントを取りこぼした行)。[`SYNC_LIST_LIMIT`] 件まで
    pub only_in_db: Vec<SyncGuild>,
    /// `only_in_db` の総数 (一覧を切り詰めても件数は分かるように)
    pub only_in_db_count: i64,
    /// Bot は参加しているが `guilds` に無い (参加イベントを取りこぼした)。[`SYNC_LIST_LIMIT`] 件まで
    pub only_in_discord: Vec<SyncGuild>,
    pub only_in_discord_count: i64,
    /// 両方にあるが名前が違う (更新イベントを取りこぼした)。[`SYNC_LIST_LIMIT`] 件まで
    pub name_mismatch: Vec<NameMismatch>,
    pub name_mismatch_count: i64,
}

/// 差分の一覧で返す件数の上限 (種類ごと)。超えた分は件数だけ (`*_count` との差) 分かる
pub const SYNC_LIST_LIMIT: usize = 200;

/// Bot の参加ギルド (Discord API) と `guilds` テーブルの差分。
/// Discord API を全ギルド分辿るので、Discord 側が不調なら 502 / 503 になる
#[utoipa::path(
    tag = "admin",
    responses(
        (status = 200, body = GuildSyncCheck),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 502, description = "Discord API に問い合わせられなかった", body = ErrorBody),
        (status = 503, description = "Discord API のレート制限中", body = ErrorBody),
    )
)]
#[get("/guilds/sync-check")]
pub async fn sync_check(
    _admin: AdminUser,
    state: web::Data<AppState>,
) -> Result<web::Json<GuildSyncCheck>, ApiError> {
    let discord_guilds = state.discord.bot_guilds().await?;
    let db_guilds = admin_guilds::all_registered(&state.pool).await?;

    let discord_by_id: HashMap<&str, &str> = discord_guilds
        .iter()
        .map(|g| (g.id.as_str(), g.name.as_str()))
        .collect();
    let db_ids: HashSet<&str> = db_guilds.iter().map(|g| g.guild_id.as_str()).collect();

    let mut only_in_db = Vec::new();
    let mut name_mismatch = Vec::new();
    for guild in &db_guilds {
        match discord_by_id.get(guild.guild_id.as_str()) {
            None => only_in_db.push(SyncGuild {
                guild_id: guild.guild_id.clone(),
                name: Some(guild.name.clone()),
            }),
            Some(discord_name) if *discord_name != guild.name => name_mismatch.push(NameMismatch {
                guild_id: guild.guild_id.clone(),
                db_name: guild.name.clone(),
                discord_name: (*discord_name).to_owned(),
            }),
            Some(_) => {}
        }
    }
    let mut only_in_discord: Vec<SyncGuild> = discord_guilds
        .iter()
        .filter(|g| !db_ids.contains(g.id.as_str()))
        .map(|g| SyncGuild {
            guild_id: g.id.clone(),
            name: Some(g.name.clone()),
        })
        .collect();

    let only_in_db_count = only_in_db.len() as i64;
    let only_in_discord_count = only_in_discord.len() as i64;
    let name_mismatch_count = name_mismatch.len() as i64;
    only_in_db.truncate(SYNC_LIST_LIMIT);
    only_in_discord.truncate(SYNC_LIST_LIMIT);
    name_mismatch.truncate(SYNC_LIST_LIMIT);

    Ok(web::Json(GuildSyncCheck {
        discord_count: discord_guilds.len() as i64,
        db_count: db_guilds.len() as i64,
        only_in_db,
        only_in_db_count,
        only_in_discord,
        only_in_discord_count,
        name_mismatch,
        name_mismatch_count,
    }))
}
