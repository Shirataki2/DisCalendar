//! リクエスト完了ログ (#110)。
//!
//! `tracing_actix_web::TracingLogger` はリクエストごとに span を張るだけで完了時のイベントを出さず、
//! `fmt` レイヤにも `FmtSpan` を設定していないため、そのままではリクエストのログが 1 行も出ない。
//! `RootSpanBuilder` を差し替えて「1 リクエスト 1 行」を足し、リクエスト単位のエラー率と
//! レイテンシを Loki 側で集計できるようにする。
//!
//! ラベルは増やさない (`infra/alloy/config.alloy` は `level` しか起こさない)。集計は本文の JSON を
//! `| json` で読む形にするので、フィールドは span ではなく**イベント**に載せる
//! (`LOG_FORMAT=json` のとき `fields.status` → Loki の `fields_status`)。クエリ例は `infra/README.md`。

use std::time::Instant;

use actix_web::{
    Error, HttpMessage as _,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
};
use tracing::Span;
use tracing_actix_web::{DefaultRootSpanBuilder, RootSpanBuilder};

/// 完了ログを DEBUG に落とすパス。compose の healthcheck が 10 秒おきに `/healthz` を叩くので、
/// これを INFO で出すと本番のログがヘルスチェックで埋まる (`RUST_LOG` の `discalendar_api=info` で消える)。
/// ローカルの既定は `discalendar_api=debug` なので、開発中は見えたままになる
const QUIET_PATHS: [&str; 1] = ["/healthz"];

/// `match_pattern()` がルートを決められなかったとき (未登録のパスへの 404) に入れる値。
/// 実際のパスを入れると予定 ID などが本文に載り、ルート別の集計もカーディナリティが爆発する
const UNMATCHED_ROUTE: &str = "unmatched";

/// 受信時刻。`ServiceRequest` と `ServiceResponse` は同じ `HttpRequest` (= 同じ extensions) を指すので、
/// 開始時に入れておけば完了時に取り出せる
#[derive(Clone, Copy)]
struct ReceivedAt(Instant);

/// リクエスト完了時に 1 行のログを出す [`RootSpanBuilder`]。
///
/// span 自体は [`DefaultRootSpanBuilder`] のままにして、`request_id` や `http.*` の記録は既定に任せる
pub struct RequestLogRootSpanBuilder;

impl RootSpanBuilder for RequestLogRootSpanBuilder {
    fn on_request_start(request: &ServiceRequest) -> Span {
        // `RefMut` は文の終わりで落とす。このあと `DefaultRootSpanBuilder` が `connection_info()` 経由で
        // `extensions_mut()` を取るので、借用を跨いで持つと `RefCell` がパニックする
        request.extensions_mut().insert(ReceivedAt(Instant::now()));
        DefaultRootSpanBuilder::on_request_start(request)
    }

    fn on_request_end<B: MessageBody>(span: Span, outcome: &Result<ServiceResponse<B>, Error>) {
        match outcome {
            Ok(response) => {
                let request = response.request();
                // `Ref` を握ったまま `request` の他のメソッドを呼ぶとパニックするので、すぐ値にする
                let received_at = request.extensions().get::<ReceivedAt>().map(|at| at.0);
                // ルーティング後なので、実際に選ばれたリソースのパターン (`/guilds/{guild_id}/events`) が返る。
                // 未登録のパスだけ `None` になる (メソッド違いの 404 はパターンが返る)
                let route = request.match_pattern();
                emit(
                    response.status().as_u16(),
                    Some(request.method().as_str()),
                    route.as_deref().unwrap_or(UNMATCHED_ROUTE),
                    received_at.map(|at| at.elapsed().as_micros() as f64 / 1000.0),
                    // クエリ文字列を含まないパスで判定する
                    QUIET_PATHS.contains(&request.path()),
                    response.response().error().map(|error| error.to_string()),
                );
            }
            // 抽出エラーもハンドラのエラーも actix が `Ok(ServiceResponse)` に変換する (`HttpResponse::from_error`)
            // ため、通常ここには来ない。`Err` では `ServiceResponse` がなく request にも触れられないので、
            // 分かるのはステータスだけ。それでも 1 行残して「ログに出ないリクエスト」を作らない
            Err(error) => {
                let response_error = error.as_response_error();
                emit(
                    response_error.status_code().as_u16(),
                    None,
                    UNMATCHED_ROUTE,
                    None,
                    false,
                    Some(response_error.to_string()),
                );
            }
        }
        // `http.status_code` などを root span に記録するのは既定の実装に任せる
        DefaultRootSpanBuilder::on_request_end(span, outcome);
    }
}

/// 完了ログを 1 行出す。
///
/// レベルは INFO 固定にする。5xx を ERROR にすると `ApiError::error_response` のログと合わせて
/// 同じ障害が Sentry に二重にイベント化されるため (#17)
fn emit(
    status: u16,
    method: Option<&str>,
    route: &str,
    duration_ms: Option<f64>,
    is_quiet: bool,
    error: Option<String>,
) {
    let error = error.as_deref();

    // `tracing::event!` はレベルを `static` の初期化式に埋めるため、実行時に決まる `Level` を渡せない。
    // フィールドの並びを 1 か所に保ったままレベルだけ差し替えるためにローカルマクロにする
    macro_rules! request_completed {
        ($level:expr) => {
            tracing::event!(
                $level,
                status, // u16 → JSON では数値 (`| json | fields_status >= 500` が効く)
                method, // Option<&str> → None ならキーごと出ない
                route,
                duration_ms, // Option<f64> → 同上。小数以下 3 桁 (マイクロ秒) まで
                error,       // エラー応答のときだけ入る
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

    use actix_web::{App, HttpResponse, http::StatusCode, test, web};
    use tracing_actix_web::TracingLogger;
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _};

    use super::RequestLogRootSpanBuilder;
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
                .wrap(TracingLogger::<RequestLogRootSpanBuilder>::new())
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
        // ルートはパターンのまま。実 ID を入れない
        let route = fields["route"].as_str().unwrap();
        assert_eq!(route, "/guilds/{guild_id}/events");
        assert!(!route.contains(GUILD));
    }

    #[actix_web::test]
    async fn logs_the_status_and_message_of_a_failed_request() {
        let buf = LogBuf::default();
        let _guard = tracing::subscriber::set_default(subscriber(&buf, "info"));

        let app = test::init_service(
            App::new()
                .wrap(TracingLogger::<RequestLogRootSpanBuilder>::new())
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
        assert_eq!(lines[0]["fields"]["error"], "event not found");
    }

    #[actix_web::test]
    async fn logs_an_unknown_path_without_the_requested_path() {
        let buf = LogBuf::default();
        let _guard = tracing::subscriber::set_default(subscriber(&buf, "info"));

        let app =
            test::init_service(App::new().wrap(TracingLogger::<RequestLogRootSpanBuilder>::new()))
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
                    .wrap(TracingLogger::<RequestLogRootSpanBuilder>::new())
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
