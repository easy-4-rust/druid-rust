#![cfg(feature = "jdbc-agent")]

use druid::core::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::ResourceOperations;
use druid_wrapper::jdbc_agent::{JdbcAgentConnectionFactory, JdbcAgentOptions};
use std::path::PathBuf;

/// 真实 H2 驱动验证 Blob 工厂、字节操作、驱动错误及释放。
#[tokio::test]
async fn h2_blob_byte_contract_through_standard_connection() {
    let (Some(agent_jar), Some(driver_jar)) = (
        std::env::var_os("DRUID_JDBC_AGENT_JAR"),
        std::env::var_os("DRUID_JDBC_DRIVER_JAR"),
    ) else {
        return;
    };
    let options = JdbcAgentOptions::java(
        "java",
        [PathBuf::from(agent_jar), PathBuf::from(driver_jar)],
    )
    .expect("JDBC Agent classpath")
    .runtime_identity("blob-byte-contract", "local", "isolated");
    let factory = JdbcAgentConnectionFactory::new(
        "jdbc:h2:mem:druid_rust_blob_bytes;DB_CLOSE_DELAY=-1",
        Some("SELECT 1".to_owned()),
        options,
    );
    let mut connection = factory.create().await.expect("H2 JDBC connection");
    let mut blob = connection.create_blob().await.expect("H2 Blob");
    assert_eq!(blob.length().await.expect("empty length"), 0);
    assert_eq!(
        blob.set_bytes(1, &[10, 20, 30, 40]).await.expect("write"),
        4
    );
    assert_eq!(blob.length().await.expect("length"), 4);
    assert_eq!(
        blob.get_bytes(2, 2).await.expect("one-based range"),
        vec![20, 30]
    );
    let error = blob
        .get_bytes(1, -1)
        .await
        .expect_err("driver rejects negative length");
    assert!(matches!(error, RdbcError::SqlException(ref sql)
        if sql.sql_state() == Some("90028") && sql.error_code() == 90028));
    let search = blob
        .position_bytes(&[20, 30], 1)
        .await
        .expect_err("H2 rejects LOB search");
    assert!(matches!(search, RdbcError::SqlException(ref sql)
        if sql.sql_state() == Some("HYC00") && sql.error_code() == 50100));
    let truncate = blob
        .truncate(2)
        .await
        .expect_err("H2 rejects LOB truncation");
    assert!(matches!(truncate, RdbcError::SqlException(ref sql)
        if sql.sql_state() == Some("HYC00") && sql.error_code() == 50100));
    assert_eq!(
        blob.capabilities(),
        ResourceOperations::READ | ResourceOperations::RANGE | ResourceOperations::FREE,
    );
    blob.free().await.expect("free");
    blob.free().await.expect("idempotent free");
    assert!(blob.length().await.is_err());
    let mut ranged = connection.create_blob().await.expect("second H2 Blob");
    assert_eq!(
        ranged
            .set_bytes_range(1, &[7, 8, 9, 10], 1, 2)
            .await
            .expect("zero-based source offset"),
        2,
    );
    assert_eq!(
        ranged.get_bytes(1, 2).await.expect("ranged write"),
        vec![8, 9]
    );
    ranged.free().await.expect("free second Blob");
    connection.close().await.expect("close connection");
}
