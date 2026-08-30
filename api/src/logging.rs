//! リクエスト完了ログ (#110)。
//!
//! `tracing_actix_web::TracingLogger` はリクエストごとに span を張るだけで完了時のイベントを出さず、
//! `fmt` レイヤにも `FmtSpan` を設定していないため、そのままではリクエストのログが 1 行も出ない。
//! ここでリクエストが終わるたびに 1 行足し、リクエスト単位のエラー率とレイテンシを Loki 側で
//! 集計できるようにする。
//!
//! ラベルは増やさない (`infra/alloy/config.alloy` は `level` しか起こさない)。集計は本文の JSON を
//! `| json` で読む形にするので、フィールドはイベントに載せる
//! (`LOG_FORMAT=json` のとき `fields.status` → Loki の `fields_status`)。クエリ例は `infra/README.md`。
//!
//! 本文には実際の URL もエラーメッセージも載せない。ルートはパターン、エラーは
//! [`ApiError::kind`] の固定語彙だけにして、リクエストごとに出るこの行に予定 ID や
//! 貼り付けられた値が混ざらないようにする。

use std::time::Instant;

use actix_web::{
    Error, HttpMessage as _,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
};
use tracing_actix_web::RequestId;

use crate::error::ApiError;

/// 完了ログを DEBUG に落とすパス。compose の healthcheck が 10 秒おきに `/healthz` を叩くので、
/// これを INFO で出すと本番のログがヘルスチェックで埋まる (`RUST_LOG` の `discalendar_api=info` で消える)。
/// ローカルの既定は `discalendar_api=debug` なので、開発中は見えたままになる
const QUIET_PATHS: [&str; 1] = ["/healthz"];

/// `match_pattern()` がルートを決められなかったとき (未登録のパスへの 404) に入れる値。
/// 実際のパスを入れると予定 ID などが本文に載り、ルート別の集計もカーディナリティが爆発する
const UNMATCHED_ROUTE: &str = "unmatched";

/// リクエストが終わるたびに 1 行 (`request completed`) を出すミドルウェア。
///
/// **`TracingLogger` の外側**に置く (`lib.rs` の wrap 順を参照)。内側に置くと root span の中で
/// イベントが起き、JSON 出力の `span` / `spans` に実 URI (`http.target`) や User-Agent が
/// 毎リクエスト載ってしまう (`tracing::event!(parent: None, ..)` では消せない。fmt の JSON
/// フォーマッタは親を持たないイベントでも現在の span を引く)。エラーの詳細を出す ERROR ログとの
/// 突き合わせは `request_id` で足りる。
///
/// 圧縮 (`Compress`) より内側なので、`duration_ms` に gzip の時間は入らない
pub async fn log_requests<B: MessageBody>(
    request: ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<B>, Error> {
    let started = Instant::now();
    let method = request.method().clone();
    // ルートのパターン (`/guilds/{guild_id}/events`)。`match_pattern` は resource map を自分で辿るので
    // ルーティング前でも引ける (同じパスに複数のメソッドを登録していてもパターン文字列は同じ)。
    // 未登録のパスだけ `None` になる
    let route = request.match_pattern();
    // クエリ文字列を含まないパスで判定する
    let is_quiet = QUIET_PATHS.contains(&request.path());

    let outcome = next.call(request).await;

    let (status, request_id, error) = match &outcome {
        Ok(response) => (
            response.status().as_u16(),
            // 内側の TracingLogger が入れた ID。`ApiError` の ERROR ログは root span 経由で同じ値を持つ
            response
                .request()
                .extensions()
                .get::<RequestId>()
                .map(ToString::to_string),
            error_kind(response.response().error()),
        ),
        // 抽出エラーもハンドラのエラーも actix が `Ok(ServiceResponse)` に変換する
        // (`HttpResponse::from_error`) ため、通常ここには来ない
        Err(error) => (
            error.as_response_error().status_code().as_u16(),
            None,
            error_kind(Some(error)),
        ),
    };

    emit(
        Completed {
            status,
            method: method.as_str(),
            route: route.as_deref().unwrap_or(UNMATCHED_ROUTE),
            // 小数以下 3 桁 (マイクロ秒) までにして行を短く保つ
            duration_ms: started.elapsed().as_micros() as f64 / 1000.0,
            request_id: request_id.as_deref(),
            error,
        },
        is_quiet,
    );

    outcome
}

/// 完了ログ 1 行のフィールド
struct Completed<'a> {
    status: u16,
    method: &'a str,
    route: &'a str,
    duration_ms: f64,
    request_id: Option<&'a str>,
    error: Option<&'static str>,
}

/// エラー応答の種別 ([`ApiError::kind`])。メッセージは載せない: Postgres は
/// `invalid input syntax for type uuid: "<値>"` のように入力の値を埋め込むため、
/// SQL コンソール (`POST /admin/sql`) で管理者が貼り付けた値がそのままログに残ってしまう
/// (監査ログ側は `models::admin_sql::sanitize_error_for_audit` で伏せている)
fn error_kind(error: Option<&Error>) -> Option<&'static str> {
    error?.as_error::<ApiError>().map(ApiError::kind)
}

/// 完了ログを 1 行出す。
///
/// レベルは INFO 固定にする。5xx を ERROR にすると `ApiError::error_response` のログと合わせて
/// 同じ障害が Sentry に二重にイベント化されるため (#17)
fn emit(log: Completed<'_>, is_quiet: bool) {
    let Completed {
        status,
        method,
        route,
        duration_ms,
        request_id,
        error,
    } = log;

    // `tracing::event!` はレベルを `static` の初期化式に埋めるため、実行時に決まる `Level` を渡せない。
    // フィールドの並びを 1 か所に保ったままレベルだけ差し替えるためにローカルマクロにする
    macro_rules! request_completed {
        ($level:expr) => {
            tracing::event!(
                $level,
                status,      // u16 → JSON では数値 (`| json | fields_status >= 500` が効く)
                method,
                route,
                duration_ms, // f64 → 同上
                request_id,  // Option<&str> → None ならキーごと出ない
                error,       // エラー応答のときだけ入る種別 (not_found / database_error など)
                "request completed"
            )
        };
    }

    if is_quiet {
        request_completed!(tracing::Level::DEBUG);
    } else {
        request_completed!(tracing::Level::INFO);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use actix_web::{App, HttpResponse, http::StatusCode, middleware::from_fn, test, web};
    use tracing_actix_web::TracingLogger;
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _};

    use super::log_requests;
    use crate::error::ApiError;

    const GUILD: &str = "111111111111111111";

    /// テスト中のログを溜める書き込み先。`LOG_FORMAT=json` と同じ形 (`fmt::layer().json()`) で
    /// 出させ、Loki が読むフィールドをそのまま検証する
    #[derive(Clone, Default)]
    struct LogBuf(Arc<Mutex<Vec<u8>>>);

    impl LogBuf {
        /// 完了ログだけを取り出す (root span の生成など他のログが混じっても落ちないように)
        fn completed(&self) -> Vec<serde_json::Value> {
            let raw = String::from_utf8(self.0.lock().unwrap().clone()).unwrap();
            raw.lines()
                .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                .filter(|line| line["fields"]["message"] == "request completed")
                .collect()
        }
    }

    impl std::io::Write for LogBuf {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogBuf {
        type Writer = Self;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    /// `main.rs` の `LOG_FORMAT=json` と同じ構成 (registry + EnvFilter + JSON の fmt レイヤ)
    fn subscriber(buf: &LogBuf, filter: &str) -> impl tracing::Subscriber + Send + Sync + 'static {
        tracing_subscriber::registry()
            .with(EnvFilter::new(filter))
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(buf.clone()),
            )
    }

    macro_rules! call {
        ($app:expr, $uri:expr) => {
            test::call_service(&$app, test::TestRequest::get().uri($uri).to_request()).await
        };
    }

    #[actix_web::test]
    async fn logs_one_line_with_the_route_pattern() {
        let buf = LogBuf::default();
        let _guard = tracing::subscriber::set_default(subscriber(&buf, "info"));

        let app = test::init_service(
            App::new()
                .wrap(TracingLogger::default())
                .wrap(from_fn(log_requests))
                .service(
                    web::resource("/guilds/{guild_id}/events")
                        .route(web::get().to(|| async { HttpResponse::Ok().finish() })),
                ),
        )
        .await;
        let response = call!(app, &format!("/guilds/{GUILD}/events"));
        assert_eq!(response.status(), StatusCode::OK);

        let lines = buf.completed();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["level"], "INFO");
        let fields = &lines[0]["fields"];
        // 数値は JSON でも数値のまま (Loki の `| json | fields_status >= 500` が効く形)
        assert_eq!(fields["status"].as_u64(), Some(200));
        assert_eq!(fields["method"], "GET");
        assert!(fields["duration_ms"].as_f64().is_some());
        // エラーではないので error は出ない
        assert!(fields["error"].is_null());
        // ERROR ログと突き合わせるための request_id は載せる
        assert!(
            fields["request_id"]
                .as_str()
                .is_some_and(|id| !id.is_empty())
        );
        // ルートはパターンのまま
        assert_eq!(fields["route"], "/guilds/{guild_id}/events");
        // 行のどこにも実 URI が出ない (root span の http.target が載らないこと)
        assert!(!lines[0].to_string().contains(GUILD), "{}", lines[0]);
        assert!(lines[0]["span"].is_null());
        assert!(lines[0]["spans"].is_null());
    }

    #[actix_web::test]
    async fn logs_the_status_and_message_of_a_failed_request() {
        let buf = LogBuf::default();
        let _guard = tracing::subscriber::set_default(subscriber(&buf, "info"));

        let app = test::init_service(
            App::new()
                .wrap(TracingLogger::default())
                .wrap(from_fn(log_requests))
                .service(web::resource("/events").route(web::get().to(|| async {
                    Err::<HttpResponse, ApiError>(ApiError::NotFound("event not found".into()))
                }))),
        )
        .await;
        let response = call!(app, "/events");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let lines = buf.completed();
        assert_eq!(lines.len(), 1);
        // 5xx を ERROR にすると ApiError::error_response のログと合わせて Sentry に二重に届く (#17)
        assert_eq!(lines[0]["level"], "INFO");
        assert_eq!(lines[0]["fields"]["status"].as_u64(), Some(404));
        // メッセージ本文ではなく ApiError::kind() の固定語彙を出す
        // (Postgres のエラーには入力値が埋め込まれることがあるため)
        assert_eq!(lines[0]["fields"]["error"], "not_found");
        assert!(
            !lines[0].to_string().contains("event not found"),
            "{}",
            lines[0]
        );
    }

    #[actix_web::test]
    async fn logs_an_unknown_path_without_the_requested_path() {
        let buf = LogBuf::default();
        let _guard = tracing::subscriber::set_default(subscriber(&buf, "info"));

        let app = test::init_service(
            App::new()
                .wrap(TracingLogger::default())
                .wrap(from_fn(log_requests)),
        )
        .await;
        let response = call!(app, &format!("/guilds/{GUILD}/unknown"));
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let lines = buf.completed();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["fields"]["status"].as_u64(), Some(404));
        assert_eq!(lines[0]["fields"]["route"], "unmatched");
    }

    #[actix_web::test]
    async fn does_not_log_the_health_check_unless_debug_is_enabled() {
        for (filter, expected) in [("info", 0), ("debug", 1)] {
            let buf = LogBuf::default();
            let _guard = tracing::subscriber::set_default(subscriber(&buf, filter));

            let app = test::init_service(
                App::new()
                    .wrap(TracingLogger::default())
                    .wrap(from_fn(log_requests))
                    .service(
                        web::resource("/healthz")
                            .route(web::get().to(|| async { HttpResponse::Ok().finish() })),
                    ),
            )
            .await;
            let response = call!(app, "/healthz");
            assert_eq!(response.status(), StatusCode::OK);

            let lines = buf.completed();
            assert_eq!(lines.len(), expected, "RUST_LOG={filter}");
            if let Some(line) = lines.first() {
                assert_eq!(line["level"], "DEBUG");
                assert_eq!(line["fields"]["route"], "/healthz");
            }
        }
    }
}
