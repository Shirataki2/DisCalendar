use discalendar_bot::config::Config;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // .env は開発用。本番は環境変数を直接渡す想定なのでファイルがなくてもエラーにしない
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,discalendar_bot=debug,sqlx=warn")),
        )
        .init();

    let config = Config::from_env()?;
    discalendar_bot::run(config).await
}
