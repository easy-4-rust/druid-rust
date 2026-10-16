#![cfg(feature = "jdbc-agent")]

use druid::core::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::types::RdbcString;
use druid_wrapper::jdbc_agent::{JdbcAgentConnectionFactory, JdbcAgentOptions};
use std::path::PathBuf;

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
async fn h2_standard_clob_preserves_utf16_and_driver_errors() {
    let Some(factory) = configured(
        "DRUID_JDBC_DRIVER_JAR",
        "clob-h2",
        "jdbc:h2:mem:druid_rust_clob_h2;DB_CLOSE_DELAY=-1",
        "SELECT 1",
    ) else {
        return;
    };
    let mut connection = factory.create().await.expect("H2 connection");
    let mut clob = connection.create_clob().await.expect("standard Clob");
    let source = RdbcString::from_utf16(vec![0x0041, 0xd83d, 0xde00, 0xd800, 0x0042]);
    assert_eq!(clob.set_string(1, &source).await.expect("set UTF-16"), 5);
    assert_eq!(clob.length().await.expect("UTF-16 length"), 5);
    assert_eq!(
        clob.get_sub_string(2, 3).await.expect("raw units"),
        RdbcString::from_utf16(vec![0xd83d, 0xde00, 0x003f])
    );
    let error = clob
        .position_string(&RdbcString::from("A"), 1)
        .await
        .expect_err("H2 search unsupported");
    assert!(
        matches!(error, RdbcError::SqlException(ref sql) if sql.sql_state() == Some("HYC00") && sql.error_code() == 50100)
    );
    assert_eq!(clob.length().await.expect("nonfatal error retains Clob"), 5);
    let reader = clob
        .get_character_stream_range(2, 3)
        .await
        .expect("range reader");
    let units = tokio::task::spawn_blocking(move || reader.read_to_end_utf16())
        .await
        .unwrap()
        .expect("read UTF-16");
    assert_eq!(units, vec![0xd83d, 0xde00, 0x003f]);
    let mut paged = connection.create_clob().await.expect("paged Clob");
    let many = RdbcString::from_utf16(vec![0x0041; 100_000]);
    paged.set_string(1, &many).await.expect("large source");
    let page_reader = paged.get_character_stream().await.expect("full reader");
    let read = tokio::task::spawn_blocking(move || {
        let mut result = Vec::new();
        let mut page = vec![0_u16; 100_000];
        loop {
            let count = page_reader.read_utf16(&mut page)?;
            if count == 0 {
                break;
            }
            result.extend_from_slice(&page[..count]);
        }
        page_reader.close()?;
        Ok::<_, RdbcError>(result)
    })
    .await
    .unwrap()
    .expect("bounded UTF-16 pages");
    assert_eq!(read, many.as_utf16());
    let child = paged.get_character_stream().await.expect("child reader");
    paged.free().await.expect("free parent with reader");
    assert!(
        tokio::task::spawn_blocking(move || child.read_to_end_utf16())
            .await
            .unwrap()
            .is_err()
    );
    clob.free().await.expect("free");
    assert!(clob.length().await.is_err());
    connection.close().await.expect("close");
}

#[tokio::test]
async fn hsqldb_standard_clob_writer_ascii_and_search() {
    let Some(factory) = configured(
        "DRUID_HSQLDB_DRIVER_JAR",
        "clob-hsqldb",
        "jdbc:hsqldb:mem:druid_rust_clob_hsqldb",
        "VALUES (1)",
    ) else {
        return;
    };
    let mut connection = factory
        .user_name("SA")
        .password("")
        .create()
        .await
        .expect("HSQLDB connection");
    let mut clob = connection.create_clob().await.expect("standard Clob");
    let writer = clob
        .set_character_stream(1)
        .await
        .expect("character writer");
    let written = tokio::task::spawn_blocking(move || {
        let value = vec![0x0041; 100_000];
        let mut offset = 0;
        while offset < value.len() {
            offset += writer.write_utf16(&value[offset..])?;
        }
        writer.close()?;
        Ok::<_, RdbcError>(offset)
    })
    .await
    .unwrap()
    .expect("write character pages");
    assert_eq!(written, 100_000);
    assert_eq!(clob.length().await.expect("length"), 100_000);
    assert_eq!(
        clob.position_string(&RdbcString::from("AAA"), 1)
            .await
            .expect("search"),
        Some(1)
    );
    clob.truncate(4).await.expect("truncate");
    assert_eq!(
        clob.get_sub_string(1, 4).await.expect("truncated"),
        RdbcString::from("AAAA")
    );
    let input = clob.get_ascii_stream().await.expect("ASCII input");
    let bytes = tokio::task::spawn_blocking(move || input.read_to_end())
        .await
        .unwrap()
        .expect("ASCII bytes");
    assert_eq!(bytes, b"AAAA");
    let mut ascii_clob = connection.create_clob().await.expect("ASCII output Clob");
    let source = vec![b'Z'; 160 * 1024];
    let output = ascii_clob.set_ascii_stream(1).await.expect("ASCII output");
    assert!(output.write(b"x").is_err(), "runtime thread must not block");
    tokio::task::spawn_blocking({
        let source = source.clone();
        move || {
            let mut offset = 0;
            while offset < source.len() {
                offset += output
                    .write(&source[offset..])
                    .expect("bounded ASCII write");
            }
            output.flush().expect("flush ASCII");
            output.close().expect("close ASCII");
        }
    })
    .await
    .expect("ASCII writer thread");
    assert_eq!(
        ascii_clob.length().await.expect("ASCII length"),
        source.len() as i64
    );
    let full_input = ascii_clob
        .get_ascii_stream()
        .await
        .expect("paged ASCII input");
    let read_back = tokio::task::spawn_blocking(move || full_input.read_to_end())
        .await
        .unwrap()
        .expect("read ASCII pages");
    assert_eq!(read_back, source);
    let child = ascii_clob
        .get_ascii_stream()
        .await
        .expect("ASCII child input");
    ascii_clob.free().await.expect("free parent with child");
    assert!(tokio::task::spawn_blocking(move || child.read_to_end())
        .await
        .unwrap()
        .is_err());
    clob.free().await.expect("free");
    connection.close().await.expect("close");
}
