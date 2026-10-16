#![cfg(feature = "jdbc-agent")]

use druid::core::ConnectionFactory;
use druid_wrapper::jdbc_agent::{JdbcAgentConnectionFactory, JdbcAgentOptions};
use std::path::PathBuf;

/// 真实 HSQLDB 驱动验证 Blob 搜索、截断和区间写入的成功路径。
#[tokio::test]
async fn hsqldb_blob_mutation_through_standard_connection() {
    let (Some(agent_jar), Some(driver_jar)) = (
        std::env::var_os("DRUID_JDBC_AGENT_JAR"),
        std::env::var_os("DRUID_HSQLDB_DRIVER_JAR"),
    ) else {
        return;
    };
    let options = JdbcAgentOptions::java(
        "java",
        [PathBuf::from(agent_jar), PathBuf::from(driver_jar)],
    )
    .expect("JDBC Agent classpath")
    .runtime_identity("blob-hsqldb-contract", "local", "isolated");
    let factory = JdbcAgentConnectionFactory::new(
        "jdbc:hsqldb:mem:druid_rust_blob_hsqldb",
        Some("VALUES (1)".to_owned()),
        options,
    )
    .user_name("SA")
    .password("");
    let mut connection = factory.create().await.expect("HSQLDB JDBC connection");
    let mut blob = connection.create_blob().await.expect("HSQLDB Blob");
    assert_eq!(blob.set_bytes(1, &[10, 20, 30, 40]).await.unwrap(), 4);
    assert_eq!(blob.position_bytes(&[20, 30], 1).await.unwrap(), Some(2));
    assert_eq!(blob.position_bytes(&[99, 98], 1).await.unwrap(), None);
    blob.truncate(3).await.expect("HSQLDB supports truncate");
    assert_eq!(blob.length().await.unwrap(), 3);
    assert_eq!(blob.get_bytes(1, 3).await.unwrap(), vec![10, 20, 30]);
    assert_eq!(blob.set_bytes_range(2, &[5, 6, 7], 1, 2).await.unwrap(), 2);
    assert_eq!(blob.get_bytes(1, 3).await.unwrap(), vec![10, 6, 7]);
    blob.free().await.expect("free HSQLDB Blob");
    blob.free().await.expect("repeat free is inert");
    connection.close().await.expect("close connection");
}
