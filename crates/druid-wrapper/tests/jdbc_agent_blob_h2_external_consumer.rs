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
        ResourceOperations::READ
            | ResourceOperations::WRITE
            | ResourceOperations::STREAM
            | ResourceOperations::RANGE
            | ResourceOperations::FREE,
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

/// 输入流由同步 RDBC 消费者在线程池读取，远端每次 RPC 不得超过 64 KiB。
#[tokio::test]
async fn h2_blob_input_stream_pages_and_parent_invalidation() {
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
    .runtime_identity("blob-stream-contract", "local", "isolated");
    let factory = JdbcAgentConnectionFactory::new(
        "jdbc:h2:mem:druid_rust_blob_stream;DB_CLOSE_DELAY=-1",
        Some("SELECT 1".to_owned()),
        options,
    );
    let mut connection = factory.create().await.expect("H2 JDBC connection");
    let mut blob = connection.create_blob().await.expect("H2 Blob");
    let source = vec![0x5au8; 160 * 1024];
    blob.set_bytes(1, &source)
        .await
        .expect("write source bytes");
    let stream = blob.get_binary_stream().await.expect("open remote input");
    let read = tokio::task::spawn_blocking(move || stream.read_to_end())
        .await
        .expect("reader thread")
        .expect("read all pages");
    assert_eq!(read, source);

    let ranged = blob
        .get_binary_stream_range(1024, 70000)
        .await
        .expect("open ranged input");
    let read = tokio::task::spawn_blocking(move || ranged.read_to_end())
        .await
        .expect("ranged reader thread")
        .expect("read ranged pages");
    assert_eq!(read, source[1023..71023]);

    let child = blob.get_binary_stream().await.expect("second input");
    blob.free().await.expect("free parent with child open");
    assert!(tokio::task::spawn_blocking(move || child.read_to_end())
        .await
        .expect("invalidated reader thread")
        .is_err());
    let mut other = connection.create_blob().await.expect("another H2 Blob");
    other
        .set_bytes(1, &[1, 2, 3])
        .await
        .expect("write child source");
    let child = other
        .get_binary_stream()
        .await
        .expect("open child before close");
    connection.close().await.expect("close connection");
    assert!(tokio::task::spawn_blocking(move || child.read_to_end())
        .await
        .expect("closed connection reader thread")
        .is_err());
}

/// 标准同步写入器必须将大写入拆成有界 Agent RPC，并等待远端关闭。
#[tokio::test]
async fn h2_blob_output_stream_pages_flush_close_and_parent_invalidation() {
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
    .runtime_identity("blob-output-contract", "local", "isolated");
    let factory = JdbcAgentConnectionFactory::new(
        "jdbc:h2:mem:druid_rust_blob_output;DB_CLOSE_DELAY=-1",
        Some("SELECT 1".to_owned()),
        options,
    );
    let mut connection = factory.create().await.expect("H2 JDBC connection");
    let mut blob = connection.create_blob().await.expect("H2 Blob");
    let source = vec![0x3cu8; 160 * 1024];
    let output = blob.set_binary_stream(1).await.expect("open output");
    tokio::task::spawn_blocking({
        let source = source.clone();
        move || {
            let mut written = 0;
            while written < source.len() {
                written += output.write(&source[written..]).expect("bounded write");
            }
            output.flush().expect("flush remote stream");
            output.close().expect("close remote stream");
        }
    })
    .await
    .expect("writer thread");
    assert_eq!(blob.length().await.unwrap(), source.len() as i64);
    assert_eq!(
        blob.get_bytes(1, source.len() as i32).await.unwrap(),
        source
    );

    let mut other = connection.create_blob().await.expect("second Blob");
    let output = other.set_binary_stream(1).await.expect("second output");
    other.free().await.expect("free parent with writer open");
    assert!(tokio::task::spawn_blocking(move || output.write(&[1]))
        .await
        .expect("invalidated writer thread")
        .is_err());
    blob.free().await.expect("free first Blob");
    connection.close().await.expect("close connection");
}
