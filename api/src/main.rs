use discalendar_api::config::Config;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer as _;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt as _;

fn main() -> anyhow::Result<()> {
    // .env は開発用。本番は環境変数を直接渡す想定なのでファイルがなくてもエラーにしない
    dotenvy::dotenv().ok();

    // エラー監視 (#17)。DSN と environment は sentry が環境変数 (SENTRY_DSN / SENTRY_ENVIRONMENT) から
    // 読み、SENTRY_DSN が未設定なら何も送らない。送信は専用スレッドで行われるため async ランタイムの
    // 開始前に初期化する必要があり、#[actix_web::main] をやめて System を手で作っている。
    // ガードは main を抜けるときに drop され、送信待ちのイベントを flush する
    let _sentry = sentry::init(
        sentry::ClientOptions::new()
            .maybe_release(sentry::release_name!())
            .sample_rate(sentry_sample_rate()),
    );

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,discalendar_api=debug,sqlx=warn")),
        )
        .with(fmt_layer())
        // ERROR をイベントとして Sentry へ送り、WARN 以下はパンくずとして直近のイベントに添える
        .with(sentry::integrations::tracing::layer())
        .init();

    // 起動に失敗してプロセスが終わるときも Sentry に残す。設定の読み込み・DB 接続・マイグレーション・bind の
    // 失敗は Err を返すだけで tracing にも panic にも乗らず、そのままでは監視から漏れる (プロセスが
    // 立ち上がらない障害ほど気づきたい)。ここで拾った ERROR は _sentry の drop 時に flush される
    let result = start();
    if let Err(error) = &result {
        tracing::error!(error = ?error, "fatal error; shutting down");
    }
    result
}

fn start() -> anyhow::Result<()> {
    let config = Config::from_env()?;
    actix_web::rt::System::new().block_on(discalendar_api::run(config))
}

/// ログの出力形式 (#104)。`LOG_FORMAT=json` のときだけ 1 行 1 JSON にする
/// (コンテナログを Grafana Alloy が Loki へ送るときに、レベルなどを取り出せるようにするため)。
/// 未設定なら人が読む形式のまま (cargo run やローカルの `docker compose logs`)
fn fmt_layer<S>() -> Box<dyn tracing_subscriber::Layer<S> + Send + Sync>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    if std::env::var("LOG_FORMAT").is_ok_and(|format| format.eq_ignore_ascii_case("json")) {
        tracing_subscriber::fmt::layer().json().boxed()
    } else {
        tracing_subscriber::fmt::layer().boxed()
    }
}

/// SENTRY_SAMPLE_RATE (0.0〜1.0、既定 1.0)。同種エラーの嵐が無料枠 (5,000 件/月) を
/// 使い切りそうなときに、再デプロイなしで送信率を絞るための逃し弁
fn sentry_sample_rate() -> f32 {
    std::env::var("SENTRY_SAMPLE_RATE")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        // ClientOptions::sample_rate は範囲外だと panic するので丸める
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(1.0)
}
