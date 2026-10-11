#![cfg(feature = "libsql-native")]
//! 远程 libSQL HTTPS 工厂必须启用 TLS，初始化不得 panic。

use druid_wrapper::libsql::LibSqlConnectionAdapter;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

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

#[tokio::test]
async fn patched_remote_client_sends_http_one_request() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 4096];
        let count = socket.read(&mut request).await.unwrap();
        let first_line = String::from_utf8_lossy(&request[..count])
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned();
        socket
            .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        first_line
    });

    let database = ::libsql::Builder::new_remote(format!("http://{address}"), String::new())
        .build()
        .await
        .unwrap();
    let connection = database.connect().unwrap();
    let request = tokio::time::timeout(Duration::from_secs(3), async {
        let _ = connection.execute("SELECT 1", ()).await;
        server.await.unwrap()
    })
    .await
    .expect("local HTTP probe must finish");
    assert!(
        request.ends_with("HTTP/1.1"),
        "unexpected request: {request}"
    );
}
