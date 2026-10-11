#![cfg(feature = "libsql-native")]
//! 远程 libSQL HTTPS 工厂必须启用 TLS，初始化不得 panic。

use druid_wrapper::libsql::LibSqlConnectionAdapter;
use std::time::Duration;

#[tokio::test]
async fn https_endpoint_initializes_without_disabled_tls_panic() {
    let attempt = tokio::time::timeout(
        Duration::from_secs(3),
        LibSqlConnectionAdapter::connect("https://127.0.0.1:1", String::new()),
    )
    .await;
    assert!(
        attempt.is_ok(),
        "HTTPS initialization must not panic or hang"
    );
}
