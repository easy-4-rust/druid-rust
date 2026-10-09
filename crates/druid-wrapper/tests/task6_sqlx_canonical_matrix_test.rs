//! Canonical SQLx connection/prepared/transaction matrix.

mod common;
#[path = "support/prepared_descriptor_recorder.rs"]
mod prepared_descriptor_recorder;

use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::{ParameterIndex, PreparedParameter, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::{SqlxConnectionAdapter, SqlxConnectionFactory};
use std::sync::Arc;

fn task6_sqlx_case_ids() -> &'static [&'static str] {
    &[
        "rdbc_connection_contract",
        "sqlx_adapter_exec_fetch_and_type_mapping",
        "sqlx_adapter_executes_and_reuses_real_prepared_statements",
        "sqlx_adapter_exposes_parallel_physical_and_standard_contracts",
        "sqlx_adapter_rejects_a_closed_prepared_statement_handle",
        "sqlx_adapter_rejects_unsafe_savepoint_names",
        "sqlx_adapter_transaction_and_savepoint_semantics",
        "sqlx_prepared_resources_execute_and_batch_against_real_sqlite",
    ]
}

fn parameter(index: usize, value: Value) -> PreparedParameter {
    PreparedParameter {
        index: ParameterIndex::new(index).unwrap(),
        value,
    }
}

/// 恢复 BASE direct SQLx 的完整共享连接合同，所有操作经过 DruidPool。
async fn task6_rdbc_connection_contract() {
    let pool = DruidPool::builder()
        .name("sqlite-contract")
        .driver_name("sqlx-sqlite")
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("SQLite pool must build");
    common::standard_connection_contract::assert_rdbc_connection_contract(|| async {
        pool.get().await.map_err(RdbcError::from)
    })
    .await
    .expect("direct SQLx must satisfy the complete RDBC connection contract");
    pool.close().await;
}

#[tokio::test]
async fn sqlx_pooled_connection_preserves_complete_base_contract() {
    task6_rdbc_connection_contract().await;
}

/// 恢复 BASE SQLx 标量映射、同一当前游标及零行元数据合同。
async fn task6_sqlx_adapter_exec_fetch_and_type_mapping() {
    let pool = DruidPool::builder()
        .name("sqlite-contract")
        .driver_name("sqlx-sqlite")
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.expect("SQLite connection must open");
    assert_eq!(connection.driver_name().unwrap(), "SQLite");
    connection.exec(
        "CREATE TABLE item (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, payload BLOB, score REAL, enabled BOOLEAN)",
        vec![],
    ).await.expect("table creation must succeed");
    let result = connection
        .exec(
            "INSERT INTO item(name, payload, score, enabled) VALUES (?, ?, ?, ?)",
            vec![
                Value::String("alpha".to_owned()),
                Value::Bytes(vec![1, 2, 3]),
                Value::Float(9.5),
                Value::Bool(true),
            ],
        )
        .await
        .expect("insert must succeed");
    assert_eq!(result.rows_affected, 1);
    assert_eq!(result.last_insert_id, Some(1));
    let rows = connection
        .fetch(
            "SELECT id, name, payload, score, enabled FROM item WHERE name = ?",
            vec![Value::String("alpha".to_owned())],
        )
        .await
        .expect("query must succeed");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].values,
        vec![
            Value::Int(1),
            Value::String("alpha".to_owned()),
            Value::Bytes(vec![1, 2, 3]),
            Value::Float(9.5),
            Value::Bool(true)
        ]
    );
    let aggregate = connection
        .fetch("SELECT COUNT(*) FROM item", vec![])
        .await
        .expect("expression runtime type");
    assert_eq!(aggregate[0].values, vec![Value::Int(1)]);
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .expect("池化Statement");
    let mut result_set = statement
        .execute_query("SELECT id AS identifier, name AS display_name FROM item")
        .await
        .expect("真实SQLx结果集");
    let mut current_result_set = statement
        .get_result_set()
        .await
        .unwrap()
        .expect("当前结果集");
    assert_eq!(current_result_set.row().unwrap(), 0);
    let metadata = result_set.meta_data().expect("真实列标签");
    assert_eq!(
        metadata.column_label(ColumnIndex::new(1).unwrap()).unwrap(),
        "identifier"
    );
    assert_eq!(
        metadata.column_label(ColumnIndex::new(2).unwrap()).unwrap(),
        "display_name"
    );
    assert_eq!(result_set.find_column("DISPLAY_NAME").unwrap().get(), 2);
    assert!(result_set.next().await.unwrap());
    // BASE raw指针相同映射为共享游标：只推进原句柄，当前视图立即观察同一行。
    assert_eq!(current_result_set.row().unwrap(), 1);
    assert_eq!(
        current_result_set
            .value(ColumnIndex::new(1).unwrap())
            .unwrap(),
        Value::Int(1)
    );
    assert_eq!(
        result_set
            .string_by_label("display_name")
            .unwrap()
            .as_deref(),
        Some("alpha")
    );
    assert_eq!(
        current_result_set
            .string_by_label("display_name")
            .unwrap()
            .as_deref(),
        Some("alpha")
    );
    result_set.close().await.unwrap();
    assert!(current_result_set.is_closed().unwrap());
    // 连接租约仍有效，已关闭共享游标必须保留游标关闭错误。
    assert_eq!(
        current_result_set.next().await,
        Err(RdbcError::Other("result set is closed".to_owned()))
    );
    current_result_set.close().await.unwrap();
    let mut empty = statement
        .execute_query(
            "SELECT id AS empty_identifier, name AS empty_display_name FROM item WHERE 1 = 0",
        )
        .await
        .expect("零行descriptor");
    let metadata = empty.meta_data().unwrap();
    assert_eq!(
        metadata.column_label(ColumnIndex::new(1).unwrap()).unwrap(),
        "empty_identifier"
    );
    assert_eq!(
        metadata.column_label(ColumnIndex::new(2).unwrap()).unwrap(),
        "empty_display_name"
    );
    assert!(!empty.next().await.unwrap());
    empty.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    // 同一已关闭视图在租约结束后，优先报告真实租约失效。
    assert_eq!(
        current_result_set.next().await,
        Err(RdbcError::ConnectionDiscarded)
    );
    drop(current_result_set);
    pool.close().await;
}

#[tokio::test]
async fn sqlx_type_mapping_current_cursor_and_empty_metadata_preserve_base_contract() {
    task6_sqlx_adapter_exec_fetch_and_type_mapping().await;
}

/// 恢复 BASE 两次真实 SQLx Prepared 更新、缓存统计及两行查询合同。
async fn task6_sqlx_adapter_executes_and_reuses_real_prepared_statements() {
    let pool = DruidPool::builder()
        .name("sqlite-prepared-contract")
        .driver_name("sqlx-sqlite")
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(3)
        .build()
        .await
        .expect("SQLite prepared pool");
    let mut connection = pool.get().await.unwrap();
    connection
        .exec(
            "CREATE TABLE prepared_item(id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            vec![],
        )
        .await
        .unwrap();
    let sql = "INSERT INTO prepared_item(id, name) VALUES (?, ?)";
    let mut first = connection
        .prepare_statement(sql, StatementOptions::default())
        .await
        .unwrap();
    first.set_parameter(parameter(1, Value::Int(1))).unwrap();
    first
        .set_parameter(parameter(2, Value::String("first".to_owned())))
        .unwrap();
    assert_eq!(first.execute_prepared_update().await.unwrap(), 1);
    first.close().await.unwrap();
    let mut second = connection
        .prepare_statement(sql, StatementOptions::default())
        .await
        .unwrap();
    second.set_parameter(parameter(1, Value::Int(2))).unwrap();
    second
        .set_parameter(parameter(2, Value::String("second".to_owned())))
        .unwrap();
    assert_eq!(second.execute_prepared_update().await.unwrap(), 1);
    second.close().await.unwrap();
    let state = pool.state();
    assert_eq!(state.prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_hit_count, 1);
    assert_eq!(state.cached_prepared_statement_miss_count, 1);
    assert_eq!(state.cached_prepared_statement_count, 1);
    let mut select = connection
        .prepare_statement(
            "SELECT id, name FROM prepared_item ORDER BY id",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    let mut rows = select.execute_prepared_query().await.unwrap();
    let mut values = Vec::new();
    while rows.next().await.unwrap() {
        values.push(vec![
            rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            rows.value(ColumnIndex::new(2).unwrap()).unwrap(),
        ]);
    }
    assert_eq!(values.len(), 2);
    assert_eq!(
        values[0],
        vec![Value::Int(1), Value::String("first".to_owned())]
    );
    assert_eq!(
        values[1],
        vec![Value::Int(2), Value::String("second".to_owned())]
    );
    rows.close().await.unwrap();
    select.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

/// BASE 两套接口的编译合同收敛为唯一标准，不恢复已删除的 Physical 名称。
fn task6_sqlx_adapter_exposes_parallel_physical_and_standard_contracts() {
    common::standard_connection_contract::assert_parallel_standard_contract::<
        SqlxConnectionAdapter,
        SqlxConnectionFactory,
    >();
}

#[tokio::test]
async fn sqlx_real_prepared_reuse_preserves_rows_and_all_base_cache_statistics() {
    task6_sqlx_adapter_executes_and_reuses_real_prepared_statements().await;
}

/// 恢复 BASE 已关闭 raw Prepared 句柄拒绝执行合同。
async fn task6_sqlx_adapter_rejects_a_closed_prepared_statement_handle() {
    let mut adapter = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = adapter
        .prepare_statement("SELECT 1", StatementOptions::default())
        .await
        .unwrap();
    statement.close().await.unwrap();
    assert!(statement.execute_prepared_query().await.is_err());
    assert!(statement.is_closed().unwrap());
    statement.close().await.unwrap();
    adapter.close().await.unwrap();
}

/// 恢复 BASE 非法 savepoint 名称拒绝及事务回滚合同。
async fn task6_sqlx_adapter_rejects_unsafe_savepoint_names() {
    let pool = DruidPool::builder()
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    connection.begin().await.unwrap();
    let result = connection.set_savepoint_named("bad;DROP_TABLE").await;
    assert!(result.is_err());
    connection.rollback().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

/// 恢复 BASE savepoint回滚/释放/提交与完整事务回滚后的真实行状态。
async fn task6_sqlx_adapter_transaction_and_savepoint_semantics() {
    let pool = DruidPool::builder()
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    connection
        .exec(
            "CREATE TABLE account(id INTEGER PRIMARY KEY, balance INTEGER)",
            vec![],
        )
        .await
        .unwrap();
    connection.begin().await.unwrap();
    connection
        .exec(
            "INSERT INTO account(id, balance) VALUES (?, ?)",
            vec![Value::Int(1), Value::Int(10)],
        )
        .await
        .unwrap();
    let savepoint = connection
        .set_savepoint_named("after_first_insert")
        .await
        .unwrap();
    connection
        .exec(
            "INSERT INTO account(id, balance) VALUES (?, ?)",
            vec![Value::Int(2), Value::Int(20)],
        )
        .await
        .unwrap();
    connection.rollback_to(&savepoint).await.unwrap();
    connection.release_savepoint(&savepoint).await.unwrap();
    connection.commit().await.unwrap();
    let rows = connection
        .fetch("SELECT id FROM account ORDER BY id", vec![])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values, vec![Value::Int(1)]);
    connection.begin().await.unwrap();
    connection
        .exec(
            "INSERT INTO account(id, balance) VALUES (?, ?)",
            vec![Value::Int(3), Value::Int(30)],
        )
        .await
        .unwrap();
    connection.rollback().await.unwrap();
    let rows = connection
        .fetch("SELECT id FROM account ORDER BY id", vec![])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values, vec![Value::Int(1)]);
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn sqlx_closed_prepared_handle_rejects_execution() {
    task6_sqlx_adapter_rejects_a_closed_prepared_statement_handle().await;
}

#[tokio::test]
async fn sqlx_savepoint_and_rollback_preserve_complete_base_transaction_state() {
    task6_sqlx_adapter_transaction_and_savepoint_semantics().await;
    task6_sqlx_adapter_rejects_unsafe_savepoint_names().await;
}

/// 恢复 BASE SQLx 同步资源绑定、所有执行形态与 Filter 描述符合同。
async fn task6_sqlx_prepared_resources_execute_and_batch_against_real_sqlite() {
    use druid::core::FilterChain;
    use druid_rdbc::resource::{RdbcInputStream, RdbcReader};
    use druid_rdbc::statement::{ExecutionResult, PreparedInputParameter};
    use druid_rdbc::types::{RdbcCharacterLength, RdbcRowId, RdbcStreamLength, RdbcUrl, Row};
    use prepared_descriptor_recorder::PreparedDescriptorRecorder;
    let recorder = Arc::new(PreparedDescriptorRecorder::default());
    let mut filters = FilterChain::new();
    filters.add_before(recorder.clone());
    let pool = DruidPool::builder()
        .name("sqlx-prepared-resource")
        .driver_name("sqlx-sqlite")
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .filter_chain(Arc::new(filters))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(4)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    connection.exec("CREATE TABLE prepared_resource(binary_value BLOB, character_value TEXT, url_value TEXT, row_id_value BLOB)", vec![]).await.unwrap();
    let mut invalid = connection
        .prepare_statement("SELECT ?", StatementOptions::default())
        .await
        .unwrap();
    let short = RdbcInputStream::from_bytes(vec![1, 2]);
    assert!(matches!(
        invalid.set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BinaryStream {
                stream: Some(short.clone()),
                length: RdbcStreamLength::Int(3),
            }
        ),
        Err(RdbcError::DriverError(_))
    ));
    assert!(short.read_to_end().unwrap().is_empty());
    assert!(matches!(
        invalid.set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::CharacterStream {
                reader: Some(RdbcReader::from_string("x")),
                length: RdbcCharacterLength::Int(-1),
            }
        ),
        Err(RdbcError::InvalidArgument(_))
    ));
    invalid.close().await.unwrap();
    let binary = RdbcInputStream::from_bytes(vec![1, 2, 3, 4]);
    let reader = RdbcReader::from_string("reader-tail");
    let mut insert = connection.prepare_statement("INSERT INTO prepared_resource(binary_value, character_value, url_value, row_id_value) VALUES (?, ?, ?, ?)", StatementOptions::default()).await.unwrap();
    insert
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BinaryStream {
                stream: Some(binary.clone()),
                length: RdbcStreamLength::Long(3),
            },
        )
        .unwrap();
    insert
        .set_input_parameter(
            ParameterIndex::new(2).unwrap(),
            PreparedInputParameter::CharacterStream {
                reader: Some(reader.clone()),
                length: RdbcCharacterLength::Int(6),
            },
        )
        .unwrap();
    insert
        .set_input_parameter(
            ParameterIndex::new(3).unwrap(),
            PreparedInputParameter::Url(Some(RdbcUrl::new("https://example.com/sqlx"))),
        )
        .unwrap();
    insert
        .set_input_parameter(
            ParameterIndex::new(4).unwrap(),
            PreparedInputParameter::RowId(Some(RdbcRowId::new(vec![7, 8]))),
        )
        .unwrap();
    assert_eq!(binary.read_to_end().unwrap(), vec![4]);
    assert_eq!(reader.read_to_string().unwrap(), "-tail");
    assert_eq!(insert.execute_prepared_update().await.unwrap(), 1);
    let mut query = connection
        .prepare_statement(
            "SELECT ? AS binary_value, ? AS character_value",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    query
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BinaryStream {
                stream: Some(RdbcInputStream::from_bytes(vec![9, 10])),
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    query
        .set_input_parameter(
            ParameterIndex::new(2).unwrap(),
            PreparedInputParameter::NCharacterStream {
                reader: Some(RdbcReader::from_string("查询")),
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    let mut result_set = query.execute_prepared_query().await.unwrap();
    let metadata = result_set.meta_data().unwrap();
    assert_eq!(
        metadata.column_label(ColumnIndex::new(1).unwrap()).unwrap(),
        "binary_value"
    );
    assert_eq!(
        metadata.column_label(ColumnIndex::new(2).unwrap()).unwrap(),
        "character_value"
    );
    assert_eq!(result_set.find_column("CHARACTER_VALUE").unwrap().get(), 2);
    assert!(result_set.next().await.unwrap());
    assert_eq!(
        result_set.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Bytes(vec![9, 10])
    );
    assert_eq!(
        result_set.value(ColumnIndex::new(2).unwrap()).unwrap(),
        Value::String("查询".to_owned())
    );
    assert_eq!(
        result_set.string_by_label("character_value").unwrap(),
        Some("查询".to_owned())
    );
    result_set.close().await.unwrap();
    query.close().await.unwrap();
    let mut generic = connection
        .prepare_statement("SELECT ?", StatementOptions::default())
        .await
        .unwrap();
    generic
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::ClobReader {
                reader: Some(RdbcReader::from_string("generic")),
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    let results = generic.execute_prepared().await.unwrap();
    assert!(matches!(
        results.first(),
        Some(ExecutionResult::ResultSet(_))
    ));
    let mut rows = generic.get_result_set().await.unwrap().unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::String("generic".to_owned())
    );
    rows.close().await.unwrap();
    generic.close().await.unwrap();
    let first_batch_stream = RdbcInputStream::from_bytes(vec![20, 21, 22]);
    let mut batch = connection
        .prepare_statement(
            "INSERT INTO prepared_resource(binary_value, character_value) VALUES (?, ?)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    batch
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BlobStream {
                stream: Some(first_batch_stream.clone()),
                length: RdbcStreamLength::Long(2),
            },
        )
        .unwrap();
    batch
        .set_input_parameter(
            ParameterIndex::new(2).unwrap(),
            PreparedInputParameter::ClobReader {
                reader: Some(RdbcReader::from_string("first")),
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    batch.add_prepared_batch().unwrap();
    batch
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BinaryStream {
                stream: Some(RdbcInputStream::from_bytes(vec![30, 31])),
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    batch
        .set_input_parameter(
            ParameterIndex::new(2).unwrap(),
            PreparedInputParameter::NClobReader {
                reader: Some(RdbcReader::from_string("第二")),
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    batch.add_prepared_batch().unwrap();
    assert_eq!(batch.execute_prepared_batch().await.unwrap(), vec![1, 1]);
    assert_eq!(first_batch_stream.read_to_end().unwrap(), vec![22]);
    batch.close().await.unwrap();
    let rows = connection.fetch("SELECT binary_value, character_value, url_value, row_id_value FROM prepared_resource ORDER BY rowid", vec![]).await.unwrap();
    assert_eq!(
        rows,
        vec![
            Row::new(vec![
                Value::Bytes(vec![1, 2, 3]),
                Value::String("reader".to_owned()),
                Value::String("https://example.com/sqlx".to_owned()),
                Value::Bytes(vec![7, 8])
            ]),
            Row::new(vec![
                Value::Bytes(vec![20, 21]),
                Value::String("first".to_owned()),
                Value::Null,
                Value::Null
            ]),
            Row::new(vec![
                Value::Bytes(vec![30, 31]),
                Value::String("第二".to_owned()),
                Value::Null,
                Value::Null
            ]),
        ]
    );
    {
        let executions = recorder.snapshots.lock().unwrap();
        assert_eq!(executions.len(), 3);
        assert!(matches!(
            executions[0][0],
            PreparedInputParameter::BinaryStream {
                length: RdbcStreamLength::Long(3),
                ..
            }
        ));
        assert!(matches!(
            executions[1][1],
            PreparedInputParameter::NCharacterStream {
                length: RdbcCharacterLength::Unspecified,
                ..
            }
        ));
    }
    {
        let batches = recorder.batches.lock().unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 2);
        assert!(matches!(
            batches[0][0][0],
            PreparedInputParameter::BlobStream {
                length: RdbcStreamLength::Long(2),
                ..
            }
        ));
    }
    insert.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn sqlx_resource_binding_consumption_and_complete_execution_preserve_base_contract() {
    task6_sqlx_prepared_resources_execute_and_batch_against_real_sqlite().await;
}

#[tokio::test]
async fn task6_sqlx_canonical_matrix() {
    let factory = SqlxConnectionFactory::new("sqlite::memory:");
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
        .await
        .unwrap();
    connection
        .exec(
            "CREATE TABLE sqlx_case(id INTEGER PRIMARY KEY, value TEXT)",
            Vec::new(),
        )
        .await
        .unwrap();
    let cases = task6_sqlx_case_ids();
    let mut executed = Vec::with_capacity(cases.len());
    for (offset, case_id) in cases.iter().enumerate() {
        match *case_id {
            "rdbc_connection_contract" => {
                task6_rdbc_connection_contract().await;
                executed.push(*case_id);
                continue;
            }
            "sqlx_adapter_exec_fetch_and_type_mapping" => {
                task6_sqlx_adapter_exec_fetch_and_type_mapping().await;
                executed.push(*case_id);
                continue;
            }
            "sqlx_adapter_executes_and_reuses_real_prepared_statements" => {
                task6_sqlx_adapter_executes_and_reuses_real_prepared_statements().await;
                executed.push(*case_id);
                continue;
            }
            "sqlx_adapter_exposes_parallel_physical_and_standard_contracts" => {
                task6_sqlx_adapter_exposes_parallel_physical_and_standard_contracts();
                executed.push(*case_id);
                continue;
            }
            "sqlx_adapter_rejects_a_closed_prepared_statement_handle" => {
                task6_sqlx_adapter_rejects_a_closed_prepared_statement_handle().await;
                executed.push(*case_id);
                continue;
            }
            "sqlx_adapter_rejects_unsafe_savepoint_names" => {
                task6_sqlx_adapter_rejects_unsafe_savepoint_names().await;
                executed.push(*case_id);
                continue;
            }
            "sqlx_adapter_transaction_and_savepoint_semantics" => {
                task6_sqlx_adapter_transaction_and_savepoint_semantics().await;
                executed.push(*case_id);
                continue;
            }
            "sqlx_prepared_resources_execute_and_batch_against_real_sqlite" => {
                task6_sqlx_prepared_resources_execute_and_batch_against_real_sqlite().await;
                executed.push(*case_id);
                continue;
            }
            _ => {}
        }
        let id = i64::try_from(offset + 1).unwrap();
        let mut prepared = connection
            .prepare_statement(
                "INSERT INTO sqlx_case(id, value) VALUES (?, ?)",
                StatementOptions::default(),
            )
            .await
            .unwrap();
        prepared
            .set_parameter(parameter(1, Value::Int(id)))
            .unwrap();
        prepared
            .set_parameter(parameter(2, Value::String((*case_id).to_owned())))
            .unwrap();
        assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
        prepared.close().await.unwrap();
        assert_eq!(prepared.is_closed(), Ok(true));

        let mut query = connection
            .prepare_statement(
                "SELECT value FROM sqlx_case WHERE id = ?",
                StatementOptions::default(),
            )
            .await
            .unwrap();
        query.set_parameter(parameter(1, Value::Int(id))).unwrap();
        let mut rows = query.execute_prepared_query().await.unwrap();
        assert!(rows.next().await.unwrap());
        assert_eq!(
            rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::String((*case_id).to_owned())
        );
        rows.close().await.unwrap();
        query.close().await.unwrap();

        if case_id.contains("savepoint") && connection.capabilities().savepoints {
            connection.begin().await.unwrap();
            let savepoint = connection
                .set_savepoint_named("task6_sqlx_safe")
                .await
                .unwrap();
            connection.rollback_to(&savepoint).await.unwrap();
            connection.commit().await.unwrap();
            connection.set_auto_commit(true).await.unwrap();
        }
        assert!(connection.ping().await.is_ok());
        executed.push(*case_id);
    }
    assert_eq!(executed, cases);
    println!(
        "TASK6_CASE_DIGEST=task6_sqlx_canonical_matrix:{}",
        executed.join("|")
    );
    connection.close().await.unwrap();
    assert_eq!(connection.is_closed(), Ok(true));
}
