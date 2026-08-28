//! DisCalendar REST API。
//!
//! - 認証: web (Next.js + Better Auth) が発行するセッション cookie を検証し、
//!   共有 Postgres の `session` / `account` テーブルから Discord ユーザー ID を引く
//! - 認可: Bot トークンで Discord API からメンバーシップとロールを取得して権限を計算する
//! - データ: 旧実装と同じ `guilds` / `events` / `guild_config` テーブル (Bot と共有)
//! - 管理コンソール (`/admin/*`): `ADMIN_DISCORD_USER_IDS` に含まれるユーザーだけが使える (`admin` モジュール)

pub mod admin;
pub mod auth;
pub mod build_info;
pub mod config;
pub mod discord;
pub mod error;
pub mod models;
pub mod openapi;
pub mod routes;
pub mod state;

use actix_web::{App, HttpServer, middleware, web};
use anyhow::Context as _;
use sqlx::postgres::PgPoolOptions;
use tracing_actix_web::TracingLogger;
use utoipa::OpenApi as _;
use utoipa_actix_web::AppExt as _;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    auth::AuthConfig,
    config::Config,
    discord::DiscordClient,
    error::ApiError,
    openapi::ApiDoc,
    state::{AdminConfig, AppState},
};

/// DB 接続・マイグレーション・HTTP サーバー起動までを行う
pub async fn run(config: Config) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_db_connections)
        .connect(&config.database_url)
        .await
        .context("failed to connect to database")?;

    run_startup_migrations(&pool).await?;
    let sql_console_pool = setup_sql_console(&pool, &config).await?;

    let state = web::Data::new(AppState {
        pool,
        sql_console_pool,
        sql_known_words: tokio::sync::Mutex::new(None),
        discord: DiscordClient::new(&config.discord_bot_token, &config.discord_api_base_url)?,
        auth: AuthConfig {
            secret: config.better_auth_secret.clone(),
            cookie_names: config.session_cookie_names().to_vec(),
        },
        admin: AdminConfig {
            discord_user_ids: config.admin_discord_user_ids.iter().cloned().collect(),
        },
        // 日付が変わったかは値 (記録済みの日付) の比較で見るので、TTL は使わなくなった
        // エントリを捨ててメモリを抑えるためだけ
        activity_days: moka::future::Cache::builder()
            .max_capacity(100_000)
            .time_to_live(std::time::Duration::from_secs(48 * 3600))
            .build(),
        started_at: chrono::Utc::now(),
    });
    if config.admin_discord_user_ids.is_empty() {
        tracing::info!("ADMIN_DISCORD_USER_IDS is empty: the admin console (/admin) is disabled");
    } else {
        tracing::info!(
            count = config.admin_discord_user_ids.len(),
            "admin console is enabled for the configured Discord users"
        );
    }

    let addr = (config.host.as_str(), config.port);
    tracing::info!(host = %config.host, port = config.port, "starting DisCalendar API");

    HttpServer::new(move || {
        App::new()
            .into_utoipa_app()
            .openapi(ApiDoc::openapi())
            .map(|app| {
                app.wrap(TracingLogger::default())
                    .wrap(middleware::Compress::default())
                    // リクエストごとの Sentry Hub を張り、イベントに URL・メソッドなどの情報を付ける (#17)。
                    // 一番外側 (最後に wrap) に置き、内側の TracingLogger のスパンやエラーが紐づくようにする。
                    // エラーの送信自体は tracing 側の layer に任せる (ApiError::error_response が 5xx を
                    // tracing::error! で記録する)。capture_server_errors を既定の true のままにすると
                    // 同じエラーが二重にイベント化され、無料枠を余分に使う
                    .wrap(
                        sentry_actix::Sentry::builder()
                            .capture_server_errors(false)
                            .finish(),
                    )
            })
            .app_data(state.clone())
            .app_data(json_config())
            .app_data(path_config())
            .app_data(query_config())
            .configure(routes::configure)
            .openapi_service(|api| SwaggerUi::new("/docs/{_:.*}").url("/openapi.json", api))
            .into_app()
    })
    .bind(addr)
    .with_context(|| format!("failed to bind {}:{}", config.host, config.port))?
    .run()
    .await
    .context("server error")
}

/// SQL コンソール (#36) 用の権限を絞ったロールとプールを用意する。
/// `SQL_CONSOLE_DATABASE_URL` があればそれで接続し (ロールは手で用意した前提)、無ければ `DATABASE_URL` と同じ DB に
/// `discalendar_sql_console_<DB 名>` でログインする (パスワードは `BETTER_AUTH_SECRET` から導出し、起動時にロールへ設定)。
/// ロールの作成・権限付与ができない環境 (CREATEROLE が無い等) では警告だけ出し、コンソールは実行時に接続と権限を
/// 検証して 503 を返す (README の手順で手動作成する)。プールの接続は遅延なので、ここでは DB に繋がない
async fn setup_sql_console(pool: &sqlx::PgPool, config: &Config) -> anyhow::Result<sqlx::PgPool> {
    use models::admin_sql;
    let (console_pool, password) = match &config.sql_console_database_url {
        Some(url) => (
            admin_sql::console_pool_from_url(url).context("SQL_CONSOLE_DATABASE_URL is invalid")?,
            None,
        ),
        None => {
            let password = admin_sql::derive_password(&config.better_auth_secret);
            (
                admin_sql::console_pool(&pool.connect_options(), &password),
                Some(password),
            )
        }
    };
    if let Err(error) = admin_sql::setup_role(pool, password.as_deref()).await {
        tracing::warn!(
            error = ?error,
            "failed to set up the SQL console role; POST /admin/sql will be unavailable until it is fixed (see README)"
        );
    }
    Ok(console_pool)
}

/// 掃除からマイグレーション完了までを跨いで直列化するアドバイザリロックの ID。
/// sqlx が `Migrator::run` の内部で使うロック (データベース名の CRC32 から算出、
/// `sqlx-postgres` の非公開関数なのでここでは再現できない) とは別物で、値そのものに
/// 意味はなく他で使われていなければよい。
///
/// 複数の API インスタンスが同時にこのバージョンへ更新される場合、掃除とマイグレーションが
/// このロックなしでは直列化されない。一方が `CREATE INDEX CONCURRENTLY` を実行中の
/// 正常な中間状態 (`indisvalid = false`) を、もう一方の掃除が「失敗残骸」と誤認して
/// 削除してしまうと、削除完了を待ってから有効になったインデックスが消え、その後
/// 前者はマイグレーションを成功として記録してしまうため、二度と再作成されなくなる
const STARTUP_LOCK_ID: i64 = 8_612_004;
/// アドバイザリロックのポーリング間隔 (下記コメント参照)
const STARTUP_LOCK_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);
/// この間隔ごとに「まだ待っている」ことをログに残す (運用者が異常な遅延に気付けるように)
const STARTUP_LOCK_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

/// 無効なインデックスの掃除とマイグレーションの適用を、複数の API インスタンス間で
/// 直列化しながら行う。
///
/// アドバイザリロックの取得から掃除・マイグレーション適用・ロック解放までを
/// 同じ1本の接続で行う。ロック用とは別にプールから接続を確保すると、
/// `DATABASE_MAX_CONNECTIONS=1` のような設定でプールの接続が尽きて後続の
/// 操作がタイムアウトしてしまう (この設定値自体は現状拒否していない)
async fn run_startup_migrations(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    let mut conn = pool
        .acquire()
        .await
        .context("failed to acquire a connection for startup migrations")?;
    acquire_startup_lock(&mut conn).await?;

    let result = async {
        cleanup_invalid_concurrent_indexes(&mut conn).await?;
        // 旧実装の entrypoint が `sqlx migrate run` してから起動していたのと同じ振る舞い。
        // sqlx の内部ロックもブロッキングな pg_advisory_lock を使うため、
        // acquire_startup_lock と同じ理由でデッドロックの原因になり得る。
        // 直列化の責任は acquire_startup_lock (ポーリング方式) に一本化し、
        // sqlx 自身の内部ロックは無効化する
        sqlx::migrate!("./migrations")
            .set_locking(false)
            .run(&mut conn)
            .await
            .context("failed to run migrations")
    }
    .await;

    // pg_advisory_lock はセッションスコープなので、ロック取得と同じ接続で解放する
    if let Err(e) = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(STARTUP_LOCK_ID)
        .execute(&mut *conn)
        .await
    {
        tracing::warn!(error = %e, "failed to release the startup advisory lock");
    }

    result
}

/// ブロッキングな `pg_advisory_lock` ではなく、`pg_try_advisory_lock` を短い間隔でポーリングする。
///
/// `CREATE INDEX CONCURRENTLY` は構築の安全性を保証するため、自分より古いスナップショットを
/// 持つ同一データベース上の全バックエンドの完了を待つフェーズ (WaitForOlderSnapshots) を持つ。
/// ブロッキングな `pg_advisory_lock` で待機しているバックエンドも「まだ完了していない文」として
/// 扱われ、その `xmin` を保持し続けるため、レプリカ B がこのロック待ちでブロックしている間に
/// レプリカ A が `CREATE INDEX CONCURRENTLY` の当該フェーズに達すると、A は B の完了を待ち、
/// B は A が握っているこのロックの解放を待つ、という循環待機 (デッドロック) が成立し得る。
/// ポーリング方式なら各試行がすぐ完了してバックエンドがアイドルに戻るため、この待機要因にならない。
///
/// 待機に上限は設けない: ローリング更新で先行インスタンスが大きな `events` テーブルに対して
/// `CREATE INDEX CONCURRENTLY` を実行していると、構築に数分かかることもあり、固定の
/// タイムアウトを設けると後続インスタンスが正常な処理の完了を待たずに起動失敗してしまう。
/// `pg_advisory_lock` はセッションスコープなので、保持しているプロセスが (正常終了でも
/// クラッシュでも) 終了すれば PostgreSQL が自動的に解放するため、無期限に待っても安全である
async fn acquire_startup_lock(conn: &mut sqlx::PgConnection) -> anyhow::Result<()> {
    let start = tokio::time::Instant::now();
    let mut last_logged = start;
    loop {
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(STARTUP_LOCK_ID)
            .fetch_one(&mut *conn)
            .await
            .context("failed to attempt the startup advisory lock")?;
        if acquired {
            return Ok(());
        }
        let now = tokio::time::Instant::now();
        if now.duration_since(last_logged) >= STARTUP_LOCK_LOG_INTERVAL {
            tracing::warn!(
                waited = ?now.duration_since(start),
                "still waiting for the startup advisory lock (another instance may be running a long migration)"
            );
            last_logged = now;
        }
        tokio::time::sleep(STARTUP_LOCK_POLL_INTERVAL).await;
    }
}

/// `CREATE INDEX CONCURRENTLY` (`migrations/..._create_events_start_at_index_concurrently.sql`)
/// が接続断・キャンセルなどで失敗すると、同名の `INVALID` なインデックスが残ることがある。
/// マイグレーション本体は一度成功すると `_sqlx_migrations` に記録され二度と実行されないため、
/// 掃除をマイグレーションの中に書くと、無効なインデックスが残ったまま
/// `IF NOT EXISTS` がそれをスキップし続け、次に起動しても直せない。
/// マイグレーション実行の直前に毎回この掃除を試みることで、失敗がいつ起きても
/// 次の起動で確実に作り直せるようにする。
///
/// `DROP INDEX` (非 CONCURRENTLY) は `events` への排他ロックを取るため、稼働中の
/// 旧 Bot / API による予定の作成・更新を削除完了までブロックしてしまう。せっかく作成側を
/// CONCURRENTLY にしても復旧経路で同じ停止が起きては意味がないので、DROP も
/// CONCURRENTLY で行う (トランザクション内では実行できないため DO ブロックは使わず、
/// 先に存在確認してから条件付きで実行する)。呼び出し元 (`run_startup_migrations`) が
/// アドバイザリロックで直列化しているので、他インスタンスが構築中の有効なインデックスを
/// 誤って消すことはない。
///
/// 存在確認は `public.events` テーブルに紐づくインデックスだけに絞る。
/// スキーマ・対象テーブルを限定しないと、同じデータベースの無関係なスキーマにたまたま
/// 同名の無効インデックスがあるだけで真になり、その後の DROP が `search_path` の解決順で
/// 見つかった別の (対象テーブルの有効な) インデックスを消しかねない
async fn cleanup_invalid_concurrent_indexes(conn: &mut sqlx::PgConnection) -> anyhow::Result<()> {
    for (index, drop_statement) in CONCURRENT_EVENT_INDEXES {
        let has_invalid_index: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM pg_index i
                JOIN pg_class idx ON idx.oid = i.indexrelid
                JOIN pg_class tbl ON tbl.oid = i.indrelid
                JOIN pg_namespace ns ON ns.oid = tbl.relnamespace
                WHERE idx.relname = $1
                  AND tbl.relname = 'events'
                  AND ns.nspname = 'public'
                  AND NOT i.indisvalid
            )
            "#,
        )
        .bind(index)
        .fetch_one(&mut *conn)
        .await
        .with_context(|| format!("failed to check for an invalid {index} index"))?;

        if !has_invalid_index {
            continue;
        }
        tracing::warn!(index, "found an invalid index, dropping it concurrently");
        sqlx::query(drop_statement)
            .execute(&mut *conn)
            .await
            .with_context(|| format!("failed to drop an invalid {index} index"))?;
    }
    Ok(())
}

/// `events` に `CREATE INDEX CONCURRENTLY` で作るインデックスと、それを落とす文。
/// マイグレーションで CONCURRENTLY のインデックスを増やしたらここにも足す
/// (足し忘れると、その作成が中断したときに無効なインデックスが残り続ける)。
/// DROP 文を組み立てず定数で持つのは、sqlx が動的な SQL 文字列を型で弾くため
const CONCURRENT_EVENT_INDEXES: [(&str, &str); 2] = [
    (
        "idx_events_start_at",
        "DROP INDEX CONCURRENTLY IF EXISTS public.idx_events_start_at",
    ),
    (
        "idx_events_guild_id_start_at",
        "DROP INDEX CONCURRENTLY IF EXISTS public.idx_events_guild_id_start_at",
    ),
];

// 抽出エラー (不正な JSON / パス / クエリ) も JSON のエラーレスポンスに統一する

fn json_config() -> web::JsonConfig {
    web::JsonConfig::default()
        .limit(64 * 1024)
        .error_handler(|err, _| ApiError::BadRequest(err.to_string()).into())
}

fn path_config() -> web::PathConfig {
    web::PathConfig::default().error_handler(|err, _| ApiError::BadRequest(err.to_string()).into())
}

fn query_config() -> web::QueryConfig {
    web::QueryConfig::default().error_handler(|err, _| ApiError::BadRequest(err.to_string()).into())
}
