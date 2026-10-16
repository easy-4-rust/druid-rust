#![cfg(feature = "jdbc-agent")]

use druid::core::ConnectionFactory;
use druid::pool::DruidPool;
use druid_rdbc::data_source::PooledConnection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::ResourceOperations;
use druid_rdbc::types::RdbcString;
use druid_wrapper::jdbc_agent::{JdbcAgentConnectionFactory, JdbcAgentOptions};
use std::path::PathBuf;
use std::sync::Arc;

fn configured(
    driver_env: &str,
    identity: &str,
    url: &str,
    validation: &str,
) -> Option<JdbcAgentConnectionFactory> {
    let (Some(agent), Some(driver)) = (
        std::env::var_os("DRUID_JDBC_AGENT_JAR"),
        std::env::var_os(driver_env),
    ) else {
        return None;
    };
    let options = JdbcAgentOptions::java("java", [PathBuf::from(agent), PathBuf::from(driver)])
        .expect("JDBC Agent classpath")
        .runtime_identity(identity, "local", "isolated");
    Some(JdbcAgentConnectionFactory::new(
        url,
        Some(validation.to_owned()),
        options,
    ))
}

#[tokio::test]
async fn h2_standard_nclob_uses_national_creation_and_preserves_utf16() {
    let Some(factory) = configured(
        "DRUID_JDBC_DRIVER_JAR",
        "nclob-h2",
        "jdbc:h2:mem:druid_rust_nclob_h2;DB_CLOSE_DELAY=-1",
        "SELECT 1",
    ) else {
        return;
    };
    let mut connection = factory.create().await.expect("H2 connection");
    let mut national = connection.create_n_clob().await.expect("standard NClob");
    assert_eq!(
        national.capabilities(),
        ResourceOperations::READ
            | ResourceOperations::WRITE
            | ResourceOperations::RANGE
            | ResourceOperations::STREAM
            | ResourceOperations::FREE
    );
    let source = RdbcString::from_utf16(vec![0x0041, 0xd83d, 0xde00, 0xd800, 0x0042]);
    assert_eq!(
        national.set_string(1, &source).await.expect("set UTF-16"),
        5
    );
    assert_eq!(national.length().await.expect("UTF-16 length"), 5);
    assert_eq!(
        national.get_sub_string(2, 3).await.expect("raw units"),
        RdbcString::from_utf16(vec![0xd83d, 0xde00, 0x003f])
    );
    let error = national
        .position_string(&RdbcString::from("A"), 1)
        .await
        .expect_err("H2 search unsupported");
    assert!(
        matches!(error, RdbcError::SqlException(ref sql) if sql.sql_state() == Some("HYC00") && sql.error_code() == 50100)
    );
    let error = national
        .truncate(3)
        .await
        .expect_err("H2 truncation unsupported");
    assert!(
        matches!(error, RdbcError::SqlException(ref sql) if sql.sql_state() == Some("HYC00") && sql.error_code() == 50100)
    );
    assert_eq!(
        national
            .length()
            .await
            .expect("nonfatal error retains NClob"),
        5
    );
    national.free().await.expect("free NClob");
    assert!(national.length().await.is_err());
    connection.close().await.expect("close connection");
}

#[tokio::test]
async fn hsqldb_standard_nclob_search_and_truncation_succeed() {
    let Some(factory) = configured(
        "DRUID_HSQLDB_DRIVER_JAR",
        "nclob-hsqldb",
        "jdbc:hsqldb:mem:druid_rust_nclob_hsqldb",
        "VALUES(1)",
    ) else {
        return;
    };
    let mut connection = factory.create().await.expect("HSQLDB connection");
    let mut national = connection.create_n_clob().await.expect("standard NClob");
    let source = RdbcString::from("alpha beta beta");
    assert_eq!(national.set_string(1, &source).await.expect("write"), 15);
    assert_eq!(
        national
            .position_string(&RdbcString::from("beta"), 1)
            .await
            .expect("search"),
        Some(7)
    );
    national.truncate(10).await.expect("truncate");
    assert_eq!(national.length().await.expect("length"), 10);
    assert_eq!(
        national.get_sub_string(1, 10).await.expect("contents"),
        RdbcString::from("alpha beta")
    );
    national.free().await.expect("free NClob");
    connection.close().await.expect("close connection");
}

async fn assert_paged_nclob(factory: JdbcAgentConnectionFactory, ascii_supported: bool) {
    let mut connection = factory.create().await.expect("connection");
    let mut national = connection.create_n_clob().await.expect("NClob");
    let source = vec![0x0041_u16; 100_000];
    let writer = national
        .set_character_stream(1)
        .await
        .expect("character writer");
    tokio::task::spawn_blocking({
        let source = source.clone();
        move || {
            let mut offset = 0;
            while offset < source.len() {
                offset += writer.write_utf16(&source[offset..])?;
            }
            writer.close()?;
            Ok::<_, RdbcError>(())
        }
    })
    .await
    .expect("writer task")
    .expect("bounded write");
    assert_eq!(national.length().await.expect("length"), 100_000);
    let reader = national.get_character_stream().await.expect("reader");
    let returned = tokio::task::spawn_blocking(move || reader.read_to_end_utf16())
        .await
        .expect("reader task")
        .expect("bounded read");
    assert_eq!(returned, source);

    let ascii = vec![b'Z'; 160 * 1024];
    let mut second = connection.create_n_clob().await.expect("ASCII NClob");
    let output = second.set_ascii_stream(1).await;
    if !ascii_supported {
        assert!(
            matches!(output, Err(RdbcError::SqlException(ref sql)) if sql.sql_state() == Some("HYC00") && sql.error_code() == 50100)
        );
        second.free().await.expect("free rejected ASCII NClob");
        national.free().await.expect("free NClob");
        connection.close().await.expect("close connection");
        return;
    }
    let output = output.expect("ASCII writer");
    tokio::task::spawn_blocking({
        let ascii = ascii.clone();
        move || {
            let mut offset = 0;
            while offset < ascii.len() {
                offset += output.write(&ascii[offset..])?;
            }
            output.close()?;
            Ok::<_, RdbcError>(())
        }
    })
    .await
    .expect("ASCII writer task")
    .expect("bounded ASCII write");
    let input = second.get_ascii_stream().await.expect("ASCII reader");
    let returned = tokio::task::spawn_blocking(move || input.read_to_end())
        .await
        .expect("ASCII reader task")
        .expect("bounded ASCII read");
    assert_eq!(returned, ascii);
    let child = second.get_ascii_stream().await.expect("child input");
    second.free().await.expect("free with child");
    assert!(tokio::task::spawn_blocking(move || child.read_to_end())
        .await
        .expect("child task")
        .is_err());
    national.free().await.expect("free NClob");
    connection.close().await.expect("close connection");
}

async fn assert_owner_invalidates_nclob(factory: JdbcAgentConnectionFactory, name: &str) {
    let factory: Arc<dyn ConnectionFactory> = Arc::new(factory);
    let pool = DruidPool::builder()
        .name(name)
        .driver_name(name)
        .factory(factory)
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("pool");
    let mut owner = pool.get_pooled_connection_owner().await.expect("owner");
    let mut logical = owner.connection().await.expect("logical connection");
    let mut national = logical.create_n_clob().await.expect("NClob");
    national
        .set_string(1, &RdbcString::from("owner test"))
        .await
        .expect("seed");
    let reader = national.get_character_stream().await.expect("child reader");
    owner.close().await.expect("owner close");
    assert!(
        tokio::task::spawn_blocking(move || reader.read_to_end_utf16())
            .await
            .expect("reader task")
            .is_err()
    );
    assert!(national.length().await.is_err());
    assert!(logical.is_closed().expect("logical closed"));
    pool.close().await;
}

#[tokio::test]
async fn h2_nclob_paged_streams_and_owner_close() {
    let Some(factory) = configured(
        "DRUID_JDBC_DRIVER_JAR",
        "nclob-h2-pages",
        "jdbc:h2:mem:druid_rust_nclob_h2_pages;DB_CLOSE_DELAY=-1",
        "SELECT 1",
    ) else {
        return;
    };
    assert_paged_nclob(factory.clone(), false).await;
    assert_owner_invalidates_nclob(factory, "nclob-h2-owner").await;
}

#[tokio::test]
async fn hsqldb_nclob_paged_streams_and_owner_close() {
    let Some(factory) = configured(
        "DRUID_HSQLDB_DRIVER_JAR",
        "nclob-hsqldb-pages",
        "jdbc:hsqldb:mem:druid_rust_nclob_hsqldb_pages",
        "VALUES (1)",
    ) else {
        return;
    };
    let factory = factory.user_name("SA").password("");
    assert_paged_nclob(factory.clone(), true).await;
    assert_owner_invalidates_nclob(factory, "nclob-hsqldb-owner").await;
}
