//! 在独立测试进程中验证生产请求 tracing，避免并行测试争用全局 callsite 缓存。

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt as _;

#[derive(Clone, Default)]
struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

struct CapturedLogWriter(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for CapturedLogWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        std::io::Write::write(&mut *self.0.lock().expect("capture lock"), buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for CapturedLogs {
    type Writer = CapturedLogWriter;

    fn make_writer(&'writer self) -> Self::Writer {
        CapturedLogWriter(Arc::clone(&self.0))
    }
}

#[tokio::test(flavor = "current_thread")]
async fn production_trace_logs_path_without_query_credentials() {
    let logs = CapturedLogs::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NEW)
        .without_time()
        .with_writer(logs.clone())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("set tracing subscriber");

    let app = druid_admin::standalone_router(druid_admin::MetricsRepository::new());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/druid/datasource.json?loginPassword=trace-password-secret&token=trace-token-secret")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let output = String::from_utf8(logs.0.lock().expect("capture lock").clone())
        .expect("tracing output is UTF-8");
    assert!(output.contains("/druid/datasource.json"), "{output}");
    for secret in [
        "trace-password-secret",
        "trace-token-secret",
        "loginPassword",
        "token=",
    ] {
        assert!(
            !output.contains(secret),
            "tracing leaked {secret}: {output}"
        );
    }
}
