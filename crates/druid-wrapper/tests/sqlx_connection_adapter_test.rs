//! `SQLx` Adapter 真实 `SQLite` 驱动合同测试。

use druid::core::{
    BatchExecContext, BeforeFilter, DruidError, ExecContext, FilterChain, PhysicalConnection,
    PreparedInputParameter, PreparedStatementKey, PreparedStatementMethodType, RdbcCharacterLength,
    RdbcInputStream, RdbcReader, RdbcRowId, RdbcStreamLength, RdbcUrl, Row, Value,
};
use druid::pool::DruidPool;
use druid_wrapper::sqlx::{SqlxConnectionAdapter, SqlxConnectionFactory};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct SqlxPreparedDescriptorRecorder {
    executions: Mutex<Vec<Vec<PreparedInputParameter>>>,
    batches: Mutex<Vec<Vec<Vec<PreparedInputParameter>>>>,
}

#[async_trait::async_trait]
impl BeforeFilter for SqlxPreparedDescriptorRecorder {
    fn name(&self) -> &'static str {
        "sqlx_prepared_descriptor_recorder"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        if let Some(parameters) = context.prepared_parameters {
            self.executions
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(parameters.to_vec());
        }
        Ok(())
    }

    async fn before_batch(&self, context: &mut BatchExecContext<'_>) -> Result<(), DruidError> {
        if let Some(parameter_sets) = context.prepared_parameter_sets {
            self.batches
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(parameter_sets.to_vec());
        }
        Ok(())
    }
}

async fn sqlite_pool() -> DruidPool {
    DruidPool::builder()
        .name("sqlite-contract")
        .driver_name("sqlx-sqlite")
        .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:")))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("SQLite pool must build")
}

#[tokio::test]
async fn sqlx_adapter_exec_fetch_and_type_mapping() {
    let pool = sqlite_pool().await;
    let mut connection = pool.get().await.expect("SQLite connection must open");
    assert_eq!(connection.driver_name(), "SQLite");

    connection
        .exec(
            "CREATE TABLE item (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                payload BLOB,
                score REAL,
                enabled BOOLEAN
            )",
            vec![],
        )
        .await
        .expect("table creation must succeed");
    let result = connection
        .exec(
            "INSERT INTO item(name, payload, score, enabled) VALUES (?, ?, ?, ?)",
            vec![
                Value::String("alpha".to_string()),
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
            vec![Value::String("alpha".to_string())],
        )
        .await
        .expect("query must succeed");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].values,
        vec![
            Value::Int(1),
            Value::String("alpha".to_string()),
            Value::Bytes(vec![1, 2, 3]),
            Value::Float(9.5),
            Value::Bool(true),
        ]
    );

    let aggregate = connection
        .fetch("SELECT COUNT(*) FROM item", vec![])
        .await
        .expect("SQLite expression columns must use their runtime value type");
    assert_eq!(aggregate[0].values, vec![Value::Int(1)]);

    let mut statement = connection
        .create_statement()
        .await
        .expect("池化 Statement 必须创建");
    let mut result_set = statement
        .execute_query_result_set(
            &mut connection,
            "SELECT id AS identifier, name AS display_name FROM item",
        )
        .await
        .expect("真实 SQLx ResultSet 必须创建");
    let current_result_set = statement
        .result_set(&mut connection)
        .unwrap()
        .expect("Statement#getResultSet 必须返回同一物理结果集");
    assert!(std::ptr::eq(
        result_set.raw_result_set(),
        current_result_set.raw_result_set()
    ));
    drop(current_result_set);
    let meta_data = result_set
        .meta_data(&mut connection)
        .expect("真实 SQLx 列标签必须保留");
    assert_eq!(meta_data.column_label(1).unwrap(), "identifier");
    assert_eq!(meta_data.column_label(2).unwrap(), "display_name");
    assert_eq!(
        result_set
            .find_column(&mut connection, "DISPLAY_NAME")
            .unwrap(),
        2
    );
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(
        result_set
            .string_by_label(&mut connection, "display_name")
            .unwrap()
            .as_deref(),
        Some("alpha")
    );
    result_set.close_with_connection(&mut connection).unwrap();

    let mut empty_result_set = statement
        .execute_query_result_set(
            &mut connection,
            "SELECT id AS empty_identifier, name AS empty_display_name FROM item WHERE 1 = 0",
        )
        .await
        .expect("零行 SQLx ResultSet 也必须保留 prepared descriptor");
    let empty_meta_data = empty_result_set.meta_data(&mut connection).unwrap();
    assert_eq!(empty_meta_data.column_label(1).unwrap(), "empty_identifier");
    assert_eq!(
        empty_meta_data.column_label(2).unwrap(),
        "empty_display_name"
    );
    assert!(!empty_result_set.next(&mut connection).unwrap());
    empty_result_set
        .close_with_connection(&mut connection)
        .unwrap();
}

#[tokio::test]
async fn sqlx_adapter_transaction_and_savepoint_semantics() {
    let pool = sqlite_pool().await;
    let mut connection = pool.get().await.expect("SQLite connection must open");
    connection
        .exec(
            "CREATE TABLE account(id INTEGER PRIMARY KEY, balance INTEGER)",
            vec![],
        )
        .await
        .expect("table creation must succeed");

    connection.begin().await.expect("transaction must begin");
    connection
        .exec(
            "INSERT INTO account(id, balance) VALUES (?, ?)",
            vec![Value::Int(1), Value::Int(10)],
        )
        .await
        .expect("first insert must succeed");
    let savepoint = connection
        .set_savepoint_named("after_first_insert")
        .await
        .expect("savepoint must be created");
    connection
        .exec(
            "INSERT INTO account(id, balance) VALUES (?, ?)",
            vec![Value::Int(2), Value::Int(20)],
        )
        .await
        .expect("second insert must succeed");
    connection
        .rollback_to(&savepoint)
        .await
        .expect("rollback to savepoint must succeed");
    connection
        .release_savepoint(&savepoint)
        .await
        .expect("savepoint release must succeed");
    connection.commit().await.expect("transaction must commit");

    let rows = connection
        .fetch("SELECT id FROM account ORDER BY id", vec![])
        .await
        .expect("verification query must succeed");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values, vec![Value::Int(1)]);

    connection
        .begin()
        .await
        .expect("second transaction must begin");
    connection
        .exec(
            "INSERT INTO account(id, balance) VALUES (?, ?)",
            vec![Value::Int(3), Value::Int(30)],
        )
        .await
        .expect("third insert must succeed");
    connection
        .rollback()
        .await
        .expect("transaction must rollback");
    let rows = connection
        .fetch("SELECT id FROM account ORDER BY id", vec![])
        .await
        .expect("rollback verification query must succeed");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values, vec![Value::Int(1)]);
}

#[tokio::test]
async fn sqlx_adapter_rejects_unsafe_savepoint_names() {
    let pool = sqlite_pool().await;
    let mut connection = pool.get().await.expect("SQLite connection must open");
    connection.begin().await.expect("transaction must begin");
    let result = connection.set_savepoint_named("bad;DROP_TABLE").await;
    assert!(result.is_err());
    connection
        .rollback()
        .await
        .expect("transaction must rollback");
}

#[tokio::test]
async fn sqlx_adapter_executes_and_reuses_real_prepared_statements() {
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
        .expect("SQLite prepared pool must build");
    let mut connection = pool.get().await.expect("SQLite connection must open");
    connection
        .exec(
            "CREATE TABLE prepared_item(id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            vec![],
        )
        .await
        .expect("table creation must succeed");

    let insert_sql = "INSERT INTO prepared_item(id, name) VALUES (?, ?)";
    let mut first_insert = connection
        .prepare_statement(insert_sql)
        .await
        .expect("first prepare must succeed");
    first_insert
        .exec(
            &mut connection,
            vec![Value::Int(1), Value::String("first".to_string())],
        )
        .await
        .expect("first prepared insert must succeed");
    first_insert.close().expect("first statement must close");

    let mut second_insert = connection
        .prepare_statement(insert_sql)
        .await
        .expect("cached prepare must succeed");
    second_insert
        .exec(
            &mut connection,
            vec![Value::Int(2), Value::String("second".to_string())],
        )
        .await
        .expect("cached prepared insert must succeed");
    second_insert.close().expect("second statement must close");

    let state = pool.state();
    assert_eq!(state.prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_hit_count, 1);
    assert_eq!(state.cached_prepared_statement_miss_count, 1);
    assert_eq!(state.cached_prepared_statement_count, 1);

    let mut select = connection
        .prepare_statement("SELECT id, name FROM prepared_item ORDER BY id")
        .await
        .expect("select prepare must succeed");
    let rows = select
        .fetch(&mut connection, vec![])
        .await
        .expect("prepared select must succeed");
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0].values,
        vec![Value::Int(1), Value::String("first".to_string())]
    );
    assert_eq!(
        rows[1].values,
        vec![Value::Int(2), Value::String("second".to_string())]
    );
    select.close().expect("select statement must close");

    connection
        .close()
        .await
        .expect("pooled connection must close");
    pool.close().await;
}

#[tokio::test]
async fn sqlx_adapter_rejects_a_closed_prepared_statement_handle() {
    let mut adapter = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .expect("SQLite adapter must connect");
    let key = PreparedStatementKey::new(
        Some("SELECT 1".to_string()),
        None,
        PreparedStatementMethodType::M1,
    )
    .expect("prepared key must build");
    let statement = adapter
        .prepare_physical_statement(&key)
        .await
        .expect("physical prepare must succeed");
    adapter
        .close_prepared_statement(statement.clone())
        .await
        .expect("physical statement close must succeed");

    assert!(adapter
        .fetch_prepared(statement.as_ref(), vec![])
        .await
        .is_err());
}

#[tokio::test]
async fn sqlx_prepared_resources_execute_and_batch_against_real_sqlite() {
    let recorder = Arc::new(SqlxPreparedDescriptorRecorder::default());
    let mut filters = FilterChain::new();
    filters.add_before(Arc::clone(&recorder) as Arc<dyn BeforeFilter>);
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
        .expect("SQLite prepared resource pool must build");
    let mut connection = pool.get().await.expect("SQLite connection must open");
    connection
        .exec(
            "CREATE TABLE prepared_resource(
                binary_value BLOB,
                character_value TEXT,
                url_value TEXT,
                row_id_value BLOB
            )",
            Vec::new(),
        )
        .await
        .unwrap();

    let mut invalid = connection.prepare_statement("SELECT ?").await.unwrap();
    let short = RdbcInputStream::from_bytes(vec![1, 2]);
    assert!(matches!(
        invalid.set_binary_stream_with_int_length(&mut connection, 1, Some(short.clone()), 3,),
        Err(DruidError::DriverError(_))
    ));
    assert!(short.read_to_end().unwrap().is_empty());
    assert!(matches!(
        invalid.set_character_stream_with_int_length(
            &mut connection,
            1,
            Some(RdbcReader::from_string("x")),
            -1,
        ),
        Err(DruidError::InvalidArgument(_))
    ));
    invalid.close_with_connection(&mut connection).unwrap();

    let binary = RdbcInputStream::from_bytes(vec![1, 2, 3, 4]);
    let reader = RdbcReader::from_string("reader-tail");
    let mut insert = connection
        .prepare_statement(
            "INSERT INTO prepared_resource(
                binary_value, character_value, url_value, row_id_value
             ) VALUES (?, ?, ?, ?)",
        )
        .await
        .unwrap();
    insert
        .set_binary_stream_with_long_length(&mut connection, 1, Some(binary.clone()), 3)
        .unwrap();
    insert
        .set_character_stream_with_int_length(&mut connection, 2, Some(reader.clone()), 6)
        .unwrap();
    insert
        .set_url(
            &mut connection,
            3,
            Some(RdbcUrl::new("https://example.com/sqlx")),
        )
        .unwrap();
    insert
        .set_row_id(&mut connection, 4, Some(RdbcRowId::new(vec![7, 8])))
        .unwrap();
    assert_eq!(binary.read_to_end().unwrap(), vec![4]);
    assert_eq!(reader.read_to_string().unwrap(), "-tail");
    assert_eq!(
        insert
            .execute_update_bound(&mut connection)
            .await
            .unwrap()
            .rows_affected,
        1
    );

    let mut query = connection
        .prepare_statement("SELECT ? AS binary_value, ? AS character_value")
        .await
        .unwrap();
    query
        .set_binary_stream(
            &mut connection,
            1,
            Some(RdbcInputStream::from_bytes(vec![9, 10])),
        )
        .unwrap();
    query
        .set_n_character_stream(&mut connection, 2, Some(RdbcReader::from_string("查询")))
        .unwrap();
    let mut result_set = query.execute_query_bound(&mut connection).await.unwrap();
    let meta_data = result_set.meta_data(&mut connection).unwrap();
    assert_eq!(meta_data.column_label(1).unwrap(), "binary_value");
    assert_eq!(meta_data.column_label(2).unwrap(), "character_value");
    assert_eq!(
        result_set
            .find_column(&mut connection, "CHARACTER_VALUE")
            .unwrap(),
        2
    );
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(
        result_set.object(&mut connection, 1).unwrap(),
        Value::Bytes(vec![9, 10])
    );
    assert_eq!(
        result_set.object(&mut connection, 2).unwrap(),
        Value::String("查询".to_string())
    );
    assert_eq!(
        result_set
            .string_by_label(&mut connection, "character_value")
            .unwrap(),
        Some("查询".to_string())
    );
    result_set.close_with_connection(&mut connection).unwrap();
    query.close_with_connection(&mut connection).unwrap();

    let mut generic = connection.prepare_statement("SELECT ?").await.unwrap();
    generic
        .set_clob_reader(&mut connection, 1, Some(RdbcReader::from_string("generic")))
        .unwrap();
    assert!(generic.execute_bound(&mut connection).await.unwrap());
    let mut generic_rows = generic.result_set(&mut connection).unwrap().unwrap();
    assert!(generic_rows.next(&mut connection).unwrap());
    assert_eq!(
        generic_rows.object(&mut connection, 1).unwrap(),
        Value::String("generic".to_string())
    );
    generic_rows.close_with_connection(&mut connection).unwrap();
    generic.close_with_connection(&mut connection).unwrap();

    let first_batch_stream = RdbcInputStream::from_bytes(vec![20, 21, 22]);
    let mut batch = connection
        .prepare_statement(
            "INSERT INTO prepared_resource(binary_value, character_value) VALUES (?, ?)",
        )
        .await
        .unwrap();
    batch
        .set_blob_stream_with_long_length(&mut connection, 1, Some(first_batch_stream.clone()), 2)
        .unwrap();
    batch
        .set_clob_reader(&mut connection, 2, Some(RdbcReader::from_string("first")))
        .unwrap();
    batch.add_bound_batch(&mut connection).unwrap();
    batch
        .set_binary_stream(
            &mut connection,
            1,
            Some(RdbcInputStream::from_bytes(vec![30, 31])),
        )
        .unwrap();
    batch
        .set_n_clob_reader(&mut connection, 2, Some(RdbcReader::from_string("第二")))
        .unwrap();
    batch.add_bound_batch(&mut connection).unwrap();
    assert_eq!(
        batch.execute_batch(&mut connection).await.unwrap(),
        vec![1, 1]
    );
    assert_eq!(first_batch_stream.read_to_end().unwrap(), vec![22]);
    batch.close_with_connection(&mut connection).unwrap();

    let rows = connection
        .fetch(
            "SELECT binary_value, character_value, url_value, row_id_value
             FROM prepared_resource ORDER BY rowid",
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        rows,
        vec![
            Row::new(vec![
                Value::Bytes(vec![1, 2, 3]),
                Value::String("reader".to_string()),
                Value::String("https://example.com/sqlx".to_string()),
                Value::Bytes(vec![7, 8]),
            ]),
            Row::new(vec![
                Value::Bytes(vec![20, 21]),
                Value::String("first".to_string()),
                Value::Null,
                Value::Null,
            ]),
            Row::new(vec![
                Value::Bytes(vec![30, 31]),
                Value::String("第二".to_string()),
                Value::Null,
                Value::Null,
            ]),
        ]
    );

    {
        let executions = recorder
            .executions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let batches = recorder
            .batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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

    insert.close_with_connection(&mut connection).unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}
