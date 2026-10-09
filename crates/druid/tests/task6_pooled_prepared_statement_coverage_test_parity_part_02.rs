//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
#![allow(clippy::approx_constant)]
//! Canonical pooled `PreparedStatement` coverage contract.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    Connection, ConnectionFactory, DruidConnectionHolder, DruidPooledConnection, FilterAdapter,
    FilterChainImpl, ProxyAttributeValue, Value,
};
use druid_rdbc::metadata::ParameterMode;
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::{
    CurrentResultDisposition, ExecutionResult, FetchDirection, GeneratedKeys, ParameterIndex,
    PreparedInputParameter, PreparedParameter, PreparedStatement, ResultSetConcurrency,
    ResultSetHoldability, ResultSetType, Statement, StatementOptions,
};
use druid_rdbc::types::{RdbcCalendarArgument, RdbcCharacterLength, RdbcStreamLength};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::any::TypeId;
use std::sync::Arc;
use std::time::Duration;

async fn make_connection() -> DruidPooledConnection {
    make_connection_with_filter(None, 1, "ps-coverage").await
}

async fn make_connection_with_chain() -> DruidPooledConnection {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    make_connection_with_filter(Some(Arc::new(chain)), 2, "ps-filter-coverage").await
}

async fn make_connection_with_filter(
    filter_chain: Option<Arc<druid::core::FilterChain>>,
    id: u64,
    data_source: &str,
) -> DruidPooledConnection {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory");
    let physical = factory.create().await.expect("connection");
    let holder = DruidConnectionHolder::with_standard_connection(physical, id, Duration::ZERO, 0)
        .await
        .expect("holder");
    DruidPooledConnection::with_standard_holder(
        holder,
        data_source.to_owned(),
        filter_chain,
        false,
        false,
        Box::new(|_, _| true),
    )
}

async fn create_statement(connection: &mut DruidPooledConnection) -> Box<dyn Statement> {
    Connection::create_statement(connection, StatementOptions::default())
        .await
        .expect("statement")
}

async fn prepare_statement(
    connection: &mut DruidPooledConnection,
    sql: &str,
) -> Box<dyn PreparedStatement> {
    Connection::prepare_statement(connection, sql, StatementOptions::default())
        .await
        .expect("prepared statement")
}

async fn setup_table(connection: &mut DruidPooledConnection) {
    let mut statement = create_statement(connection).await;
    statement
        .execute_update(
            "CREATE TABLE IF NOT EXISTS ps_cov(id INTEGER PRIMARY KEY, name TEXT, val REAL)",
            GeneratedKeys::None,
        )
        .await
        .expect("create table");
    statement.close().await.expect("statement close");
}

fn index(value: usize) -> ParameterIndex {
    ParameterIndex::new(value).expect("1-based parameter index")
}

fn parameter(
    value: usize,
    input: PreparedInputParameter,
) -> (ParameterIndex, PreparedInputParameter) {
    (index(value), input)
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_parameter_binding_all_types() {
    let mut connection = make_connection().await;
    setup_table(&mut connection).await;
    let mut prepared = prepare_statement(
        &mut connection,
        "INSERT INTO ps_cov(id, name, val) VALUES (?1, ?2, ?3)",
    )
    .await;
    prepared
        .set_input_parameter(index(1), PreparedInputParameter::Int(42))
        .unwrap();
    prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::NString(Some("hello".to_owned())),
        )
        .unwrap();
    prepared
        .set_input_parameter(index(3), PreparedInputParameter::Double(3.14))
        .unwrap();
    prepared
        .set_input_parameter(index(1), PreparedInputParameter::Boolean(true))
        .unwrap();
    prepared
        .set_input_parameter(index(1), PreparedInputParameter::Byte(1))
        .unwrap();
    prepared
        .set_input_parameter(index(1), PreparedInputParameter::Short(1))
        .unwrap();
    prepared
        .set_input_parameter(index(1), PreparedInputParameter::Long(1))
        .unwrap();
    prepared
        .set_input_parameter(index(3), PreparedInputParameter::Float(1.5))
        .unwrap();
    prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::String(Some("world".to_owned())),
        )
        .unwrap();
    prepared
        .set_input_parameter(index(2), PreparedInputParameter::Bytes(Some(vec![1, 2, 3])))
        .unwrap();
    prepared
        .set_input_parameter(index(2), PreparedInputParameter::null(12))
        .unwrap();
    prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::null_with_type_name(12, None),
        )
        .unwrap();
    prepared
        .set_input_parameter(index(1), PreparedInputParameter::object(None))
        .unwrap();
    prepared
        .set_input_parameter(
            index(1),
            PreparedInputParameter::object_with_sql_type(None, 4),
        )
        .unwrap();
    prepared
        .set_input_parameter(
            index(1),
            PreparedInputParameter::object_with_sql_type_and_scale(None, 4, 0),
        )
        .unwrap();
    let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    let time = NaiveTime::from_hms_opt(10, 30, 0).unwrap();
    let timestamp = NaiveDateTime::new(date, time);
    prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::Date {
                value: Some(date),
                calendar: RdbcCalendarArgument::Unspecified,
            },
        )
        .unwrap();
    prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::Date {
                value: Some(date),
                calendar: RdbcCalendarArgument::Specified(None),
            },
        )
        .unwrap();
    prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::Time {
                value: Some(time),
                calendar: RdbcCalendarArgument::Unspecified,
            },
        )
        .unwrap();
    prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::Time {
                value: Some(time),
                calendar: RdbcCalendarArgument::Specified(None),
            },
        )
        .unwrap();
    prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::Timestamp {
                value: Some(timestamp),
                calendar: RdbcCalendarArgument::Unspecified,
            },
        )
        .unwrap();
    prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::Timestamp {
                value: Some(timestamp),
                calendar: RdbcCalendarArgument::Specified(None),
            },
        )
        .unwrap();
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Unspecified,
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Int(100),
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Long(100),
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Unspecified,
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Int(100),
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Long(100),
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::UnicodeStream {
                stream: None,
                length: 100,
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Unspecified,
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Int(100),
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(100),
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::NCharacterStream {
                reader: None,
                length: RdbcCharacterLength::Unspecified,
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::NCharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(100),
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::ClobReader {
                reader: None,
                length: RdbcCharacterLength::Unspecified,
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::ClobReader {
                reader: None,
                length: RdbcCharacterLength::Long(100),
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::NClobReader {
                reader: None,
                length: RdbcCharacterLength::Unspecified,
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::NClobReader {
                reader: None,
                length: RdbcCharacterLength::Long(100),
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::BlobStream {
                stream: None,
                length: RdbcStreamLength::Unspecified,
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert!(prepared
        .set_input_parameter(
            index(2),
            PreparedInputParameter::BlobStream {
                stream: None,
                length: RdbcStreamLength::Long(100),
            }
        )
        .is_ok());
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    // 对应 BASE 的 nullable 流绑定：每种描述符均须成功执行，并真实存储 SQL NULL。
    prepared.clear_parameters().unwrap();
    prepared.close().await.unwrap();
    let mut query = prepare_statement(
        &mut connection,
        "SELECT id, name, val FROM ps_cov ORDER BY id",
    )
    .await;
    let mut rows = query.execute_prepared_query().await.unwrap();
    let mut row_count = 0;
    while rows.next().await.unwrap() {
        row_count += 1;
        assert_eq!(rows.int(ColumnIndex::new(1).unwrap()).unwrap(), row_count);
        assert_eq!(
            rows.value(ColumnIndex::new(2).unwrap()).unwrap(),
            Value::Null
        );
        assert_eq!(
            rows.value(ColumnIndex::new(3).unwrap()).unwrap(),
            Value::Float(1.5)
        );
    }
    assert_eq!(row_count, 18);
    rows.close().await.unwrap();
    query.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_execute_bound_paths() {
    let mut connection = make_connection().await;
    setup_table(&mut connection).await;
    let mut insert = prepare_statement(
        &mut connection,
        "INSERT INTO ps_cov(id, name) VALUES (?1, ?2)",
    )
    .await;
    insert
        .set_parameter(PreparedParameter {
            index: index(1),
            value: Value::Int(1),
        })
        .unwrap();
    insert
        .set_parameter(PreparedParameter {
            index: index(2),
            value: Value::String("test".to_owned()),
        })
        .unwrap();
    assert_eq!(insert.execute_prepared_update().await.unwrap(), 1);
    insert.close().await.unwrap();
    let mut query =
        prepare_statement(&mut connection, "SELECT id, name FROM ps_cov WHERE id = ?1").await;
    query
        .set_parameter(PreparedParameter {
            index: index(1),
            value: Value::Int(1),
        })
        .unwrap();
    let mut result_set = query.execute_prepared_query().await.unwrap();
    assert!(result_set.next().await.unwrap());
    assert_eq!(result_set.int(ColumnIndex::new(1).unwrap()).unwrap(), 1);
    result_set.close().await.unwrap();
    query.close().await.unwrap();
    let mut generic = prepare_statement(&mut connection, "SELECT 1").await;
    assert!(matches!(
        generic.execute_prepared().await.unwrap().first(),
        Some(ExecutionResult::ResultSet(_))
    ));
    generic.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_fetch_bound_eager_rows() {
    let mut connection = make_connection().await;
    setup_table(&mut connection).await;
    let mut statement = create_statement(&mut connection).await;
    statement
        .execute_update(
            "INSERT INTO ps_cov(id, name) VALUES (10, 'a')",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    statement.close().await.unwrap();
    let mut prepared =
        prepare_statement(&mut connection, "SELECT id, name FROM ps_cov WHERE id = ?1").await;
    prepared
        .set_parameter(PreparedParameter {
            index: index(1),
            value: Value::Int(10),
        })
        .unwrap();
    let mut result_set = prepared.execute_prepared_query().await.unwrap();
    let mut rows = 0;
    while result_set.next().await.unwrap() {
        rows += 1;
    }
    assert_eq!(rows, 1);
    prepared.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_batch_add_execute_clear() {
    let mut connection = make_connection().await;
    setup_table(&mut connection).await;
    let mut prepared = prepare_statement(
        &mut connection,
        "INSERT INTO ps_cov(id, name) VALUES (?1, ?2)",
    )
    .await;
    prepared
        .add_parameter_batch(vec![
            PreparedParameter {
                index: index(1),
                value: Value::Int(1),
            },
            PreparedParameter {
                index: index(2),
                value: Value::String("a".to_owned()),
            },
        ])
        .unwrap();
    prepared
        .add_parameter_batch(vec![
            PreparedParameter {
                index: index(1),
                value: Value::Int(2),
            },
            PreparedParameter {
                index: index(2),
                value: Value::String("b".to_owned()),
            },
        ])
        .unwrap();
    assert_eq!(prepared.execute_prepared_batch().await.unwrap(), vec![1, 1]);
    prepared
        .set_parameter(PreparedParameter {
            index: index(1),
            value: Value::Int(3),
        })
        .unwrap();
    prepared
        .set_parameter(PreparedParameter {
            index: index(2),
            value: Value::String("c".to_owned()),
        })
        .unwrap();
    prepared.add_prepared_batch().unwrap();
    assert_eq!(prepared.execute_prepared_batch().await.unwrap(), vec![1]);
    prepared.add_prepared_batch().unwrap();
    prepared.clear_prepared_batch().unwrap();
    assert_eq!(
        prepared.execute_prepared_batch().await.unwrap(),
        Vec::<i64>::new()
    );
    assert_eq!(
        prepared.sql().unwrap(),
        "INSERT INTO ps_cov(id, name) VALUES (?1, ?2)"
    );
    prepared.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_generated_keys_and_more_results() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    statement
        .execute_update(
            "CREATE TABLE ps_gk(id INTEGER PRIMARY KEY, name TEXT)",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    statement.close().await.unwrap();
    let mut prepared =
        prepare_statement(&mut connection, "INSERT INTO ps_gk(name) VALUES (?1)").await;
    prepared
        .set_parameter(PreparedParameter {
            index: index(1),
            value: Value::String("test".to_owned()),
        })
        .unwrap();
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    let mut keys = prepared.generated_keys().await.unwrap();
    assert!(!keys.is_closed().unwrap());
    assert!(keys.next().await.unwrap());
    assert!(keys.long(ColumnIndex::new(1).unwrap()).unwrap() > 0);
    assert_eq!(keys.meta_data().unwrap().column_count(), 1);
    keys.close().await.unwrap();
    assert!(keys.is_closed().unwrap());
    assert!(!prepared
        .get_more_results(CurrentResultDisposition::CloseCurrent)
        .await
        .unwrap());
    assert!(!prepared
        .get_more_results(CurrentResultDisposition::KeepCurrent)
        .await
        .unwrap());
    assert!(!prepared
        .get_more_results(CurrentResultDisposition::CloseAll)
        .await
        .unwrap());
    assert_eq!(prepared.get_update_count().unwrap(), None);
    prepared.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_execute_query_bound() {
    let mut connection = make_connection().await;
    setup_table(&mut connection).await;
    let mut statement = create_statement(&mut connection).await;
    statement
        .execute_update(
            "INSERT INTO ps_cov(id, name) VALUES (100, 'rs')",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    statement.close().await.unwrap();
    let mut prepared =
        prepare_statement(&mut connection, "SELECT id, name FROM ps_cov WHERE id = ?1").await;
    prepared
        .set_parameter(PreparedParameter {
            index: index(1),
            value: Value::Int(100),
        })
        .unwrap();
    let mut result_set = prepared.execute_prepared_query().await.unwrap();
    assert!(result_set.next().await.unwrap());
    assert_eq!(result_set.int(ColumnIndex::new(1).unwrap()).unwrap(), 100);
    assert_eq!(
        result_set
            .n_string(ColumnIndex::new(2).unwrap())
            .unwrap()
            .as_deref(),
        Some("rs")
    );
    result_set.close().await.unwrap();
    prepared.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_execute_generic() {
    let mut connection = make_connection().await;
    setup_table(&mut connection).await;
    let mut prepared = prepare_statement(&mut connection, "SELECT 1").await;
    let mut results = prepared.execute_prepared().await.unwrap();
    assert_eq!(results.len(), 1);
    let ExecutionResult::ResultSet(mut result_set) = results.remove(0) else {
        panic!("SELECT must yield a canonical ResultSet");
    };
    assert!(result_set.next().await.unwrap());
    result_set.close().await.unwrap();
    prepared.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_fetch_eager_rows() {
    let mut connection = make_connection().await;
    setup_table(&mut connection).await;
    let mut statement = create_statement(&mut connection).await;
    statement
        .execute_update(
            "INSERT INTO ps_cov(id, name) VALUES (200, 'fetch')",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    statement.close().await.unwrap();
    let mut prepared =
        prepare_statement(&mut connection, "SELECT id, name FROM ps_cov WHERE id = ?1").await;
    prepared
        .set_parameter(PreparedParameter {
            index: index(1),
            value: Value::Int(200),
        })
        .unwrap();
    let mut result_set = prepared.execute_prepared_query().await.unwrap();
    let mut rows = 0;
    while result_set.next().await.unwrap() {
        rows += 1;
    }
    assert_eq!(rows, 1);
    prepared.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_fetch_result_set() {
    let mut connection = make_connection().await;
    setup_table(&mut connection).await;
    let mut statement = create_statement(&mut connection).await;
    statement
        .execute_update(
            "INSERT INTO ps_cov(id, name) VALUES (300, 'frs')",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    statement.close().await.unwrap();
    let mut prepared =
        prepare_statement(&mut connection, "SELECT id, name FROM ps_cov WHERE id = ?1").await;
    prepared
        .set_parameter(PreparedParameter {
            index: index(1),
            value: Value::Int(300),
        })
        .unwrap();
    let mut result_set = prepared.execute_prepared_query().await.unwrap();
    assert!(result_set.next().await.unwrap());
    assert_eq!(
        result_set.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(300)
    );
    assert_eq!(result_set.meta_data().unwrap().column_count(), 2);
    result_set.close().await.unwrap();
    assert!(result_set.is_closed().unwrap());
    prepared.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_exec_update_with_values() {
    let mut connection = make_connection().await;
    setup_table(&mut connection).await;
    let mut prepared = prepare_statement(
        &mut connection,
        "INSERT INTO ps_cov(id, name) VALUES (?1, ?2)",
    )
    .await;
    prepared
        .set_parameter(PreparedParameter {
            index: index(1),
            value: Value::Int(500),
        })
        .unwrap();
    prepared
        .set_parameter(PreparedParameter {
            index: index(2),
            value: Value::String("exec".to_owned()),
        })
        .unwrap();
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    prepared.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_batch_through_filter_chain() {
    let mut connection = make_connection_with_chain().await;
    setup_table(&mut connection).await;
    let mut prepared = prepare_statement(
        &mut connection,
        "INSERT INTO ps_cov(id, name) VALUES (?1, ?2)",
    )
    .await;
    prepared
        .add_parameter_batch(vec![
            PreparedParameter {
                index: index(1),
                value: Value::Int(1001),
            },
            PreparedParameter {
                index: index(2),
                value: Value::String("fc1".to_owned()),
            },
        ])
        .unwrap();
    prepared
        .add_parameter_batch(vec![
            PreparedParameter {
                index: index(1),
                value: Value::Int(1002),
            },
            PreparedParameter {
                index: index(2),
                value: Value::String("fc2".to_owned()),
            },
        ])
        .unwrap();
    assert_eq!(prepared.execute_prepared_batch().await.unwrap().len(), 2);
    prepared.close().await.unwrap();
}
