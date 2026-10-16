#![cfg(feature = "jdbc-agent")]

use druid::core::ConnectionFactory;
use druid_rdbc::resource::ResourceOperations;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::StatementOptions;
use druid_rdbc::types::{RdbcObject, Value};
use druid_wrapper::jdbc_agent::{JdbcAgentConnectionFactory, JdbcAgentOptions};
use std::path::PathBuf;

/// 真实 H2 驱动验证 JDBC Agent Array 创建、值类型、范围和释放。
#[tokio::test]
async fn h2_array_factory_roundtrips_through_standard_connection() {
    let Some(agent_jar) = std::env::var_os("DRUID_JDBC_AGENT_JAR") else {
        return;
    };
    let Some(driver_jar) = std::env::var_os("DRUID_JDBC_DRIVER_JAR") else {
        return;
    };
    let options = JdbcAgentOptions::java(
        "java",
        [PathBuf::from(agent_jar), PathBuf::from(driver_jar)],
    )
    .expect("JDBC Agent classpath");
    let factory = JdbcAgentConnectionFactory::new(
        "jdbc:h2:mem:druid_rust_array;DB_CLOSE_DELAY=-1",
        Some("SELECT 1".to_owned()),
        options,
    );
    let mut connection = factory.create().await.expect("H2 JDBC connection");
    let _statement = connection
        .create_statement(StatementOptions::default())
        .await
        .expect("statement transfers transport to owned executor");
    let mut array = connection
        .create_array_of(
            Some("INTEGER"),
            Some(vec![
                RdbcObject::Integer(7),
                RdbcObject::Scalar(Value::Null),
                RdbcObject::Integer(9),
            ]),
        )
        .await
        .expect("H2 Array");

    assert_eq!(
        array.base_type_name().await.expect("base type name"),
        "INTEGER"
    );
    assert_eq!(array.base_type().await.expect("base type"), 4);
    assert_eq!(
        array.values().await.expect("values"),
        vec![
            RdbcObject::Integer(7),
            RdbcObject::Scalar(Value::Null),
            RdbcObject::Integer(9),
        ]
    );
    assert_eq!(
        array.values_range(2, 2).await.expect("range"),
        vec![RdbcObject::Scalar(Value::Null), RdbcObject::Integer(9),]
    );
    assert_eq!(
        array.capabilities(),
        ResourceOperations::READ
            | ResourceOperations::RANGE
            | ResourceOperations::RESULT_SET
            | ResourceOperations::FREE
    );
    let mut cursor = array.result_set().await.expect("Array ResultSet");
    assert_eq!(cursor.known_row_count(), None);
    assert_eq!(cursor.meta_data().expect("metadata").column_count(), 2);
    assert!(cursor.next().await.expect("first row"));
    assert_eq!(
        cursor
            .object(ColumnIndex::new(2).expect("column"))
            .expect("object"),
        RdbcObject::Integer(7)
    );
    assert!(cursor.next().await.expect("null row"));
    assert_eq!(
        cursor
            .value(ColumnIndex::new(2).expect("column"))
            .expect("null value"),
        Value::Null
    );
    array.free().await.expect("free");
    assert!(cursor.next().await.is_err());
    cursor
        .close()
        .await
        .expect("close cursor after parent free");
    assert!(array.values().await.is_err());
    // 在同一会话验证跨越 Agent 有界页；避免共享运行时的并行测试相互关闭进程。
    let mut array = connection
        .create_array_of(
            Some("INTEGER"),
            Some((1_i32..=9).map(RdbcObject::Integer).collect()),
        )
        .await
        .expect("H2 Array");
    let mut cursor = array.result_set().await.expect("Array ResultSet");
    let index = ColumnIndex::new(1).expect("index column");
    let value = ColumnIndex::new(2).expect("value column");
    for expected in 1_i32..=9 {
        assert!(cursor.next().await.expect("next array element"));
        assert_eq!(
            cursor.value(index).expect("index"),
            Value::Int(i64::from(expected))
        );
        assert_eq!(
            cursor.object(value).expect("value"),
            RdbcObject::Integer(expected)
        );
    }
    assert!(!cursor.next().await.expect("end of Array ResultSet"));
    cursor.close().await.expect("close cursor");
    array.free().await.expect("free array");
    connection.close().await.expect("close connection");
}
