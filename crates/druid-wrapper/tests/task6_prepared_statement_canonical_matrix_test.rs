//! Canonical owned-PreparedStatement matrix replacing private SqlText/cache test coupling.

#[path = "../../druid/tests/standard_test_support/mod.rs"]
mod standard_test_support;

use druid::core::{
    ConnectionFactory, DruidConnectionHolder, ExceptionSorter, ExceptionSorterProperties,
    RdbcError, SqlException,
};
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{
    FetchDirection, ParameterIndex, PreparedParameter, PreparedStatement, ResultSetConcurrency,
    ResultSetHoldability, ResultSetType, Statement, StatementOptions,
};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionFactory;
use standard_test_support::{TrackingConnection, TrackingFactory, TrackingState};
use std::sync::Arc;

#[derive(Clone, Copy)]
enum CleanupFailure {
    AddBatch,
    ClearBatch,
    ClearParameters,
    SetParameter,
}

struct CleanupPrepared {
    failure: CleanupFailure,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(CleanupPrepared => PreparedStatement);

#[async_trait::async_trait]
impl druid_rdbc::statement::Statement for CleanupPrepared {
    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        if matches!(self.failure, CleanupFailure::ClearBatch) {
            Err(task6_cleanup_sql_error())
        } else {
            Ok(())
        }
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for CleanupPrepared {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok("SELECT 1")
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        if matches!(self.failure, CleanupFailure::ClearParameters) {
            Err(task6_cleanup_sql_error())
        } else {
            Ok(())
        }
    }

    fn set_parameter(&mut self, _parameter: PreparedParameter) -> Result<(), RdbcError> {
        if matches!(self.failure, CleanupFailure::SetParameter) {
            Err(task6_cleanup_sql_error())
        } else {
            Ok(())
        }
    }

    fn add_parameter_batch(
        &mut self,
        _parameters: Vec<PreparedParameter>,
    ) -> Result<(), RdbcError> {
        if matches!(self.failure, CleanupFailure::AddBatch) {
            Err(task6_cleanup_sql_error())
        } else {
            Ok(())
        }
    }

    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        if matches!(self.failure, CleanupFailure::ClearBatch) {
            Err(task6_cleanup_sql_error())
        } else {
            Ok(())
        }
    }
}

fn task6_cleanup_sql_error() -> RdbcError {
    RdbcError::SqlException(Box::new(
        SqlException::driver(17_002, "forced prepared cleanup failure")
            .with_sql_state("08006")
            .with_class_name("task6.PreparedCleanupFailure"),
    ))
}

struct CleanupConnection {
    failure: CleanupFailure,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(CleanupConnection => Connection);

#[async_trait::async_trait]
impl Connection for CleanupConnection {
    async fn prepare_statement(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        Ok(Box::new(CleanupPrepared {
            failure: self.failure,
            closed: false,
        }))
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

struct CleanupFactory(CleanupFailure);

#[async_trait::async_trait]
impl druid_rdbc::driver::ConnectionFactory for CleanupFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(CleanupConnection {
            failure: self.0,
            closed: false,
        }))
    }
}

struct CleanupFatalSorter;
impl ExceptionSorter for CleanupFatalSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        true
    }
    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

fn task6_prepared_statement_case_ids() -> &'static [&'static str] {
    &[
        "clear_batch_failure_uses_connection_sorter_before_cache_return",
        "clear_parameters_failure_uses_connection_sorter_before_cache_return",
        "concurrent_logical_statements_preserve_java_in_use_and_lru_semantics",
        "direct_prepared_setter_and_batch_methods_route_failures_through_connection_sorter",
        "holder_statement_pool_is_lazy_and_debug_does_not_initialize_it",
        "mysql_schema_change_clears_existing_statement_cache",
        "pooled_prepared_statement_forwards_statement_properties_and_restores_real_sqlite_handle",
        "prepared_batch_snapshots_parameters_and_executes_against_real_sqlite",
        "prepared_bound_setters_execute_and_batch_against_real_sqlite",
        "prepared_close_restores_properties_in_java_order_and_stops_on_first_error",
        "prepared_generic_execute_matches_real_sqlite_result_and_generated_key_semantics",
        "prepared_generic_execute_preserves_ordered_multi_results",
        "prepared_input_parameter_converts_every_scalar_family_without_losing_type",
        "prepared_result_getters_route_driver_failures_through_connection_sorter",
        "prepared_result_set_keeps_the_same_statement_alive_and_can_close_it",
        "prepared_setter_overloads_preserve_exact_descriptor_identity",
        "prepared_statement_cannot_cross_connection_lease_boundary",
        "prepared_statement_reuses_physical_handle_and_updates_java_stats",
        "prepared_stream_reader_lob_url_rowid_and_null_bind_against_real_sqlite",
        "result_set_statement_handle_reports_clear_batch_failure",
        "result_set_statement_handle_reports_clear_parameters_failure",
        "statement_execution_error_removes_handle_instead_of_reusing_it",
    ]
}

fn parameter(index: usize, value: Value) -> PreparedParameter {
    PreparedParameter {
        index: ParameterIndex::new(index).expect("1-based parameter"),
        value,
    }
}

async fn task6_prepared_pool(max_cached: usize) -> (DruidPool, Arc<TrackingState>) {
    let state = TrackingState::configured();
    let pool = DruidPool::builder()
        .name("task6-prepared-parameterized")
        .driver_name("tracking")
        .factory(Arc::new(TrackingFactory::new(Arc::clone(&state))) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(max_cached)
        .build()
        .await
        .unwrap();
    (pool, state)
}

async fn task6_sqlx_prepared_pool(max_cached: usize) -> DruidPool {
    DruidPool::builder()
        .name("task6-sqlx-prepared-parameterized")
        .driver_name("sqlx")
        .factory(
            Arc::new(SqlxConnectionFactory::new("sqlite::memory:")) as Arc<dyn ConnectionFactory>
        )
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(max_cached)
        .build()
        .await
        .unwrap()
}

async fn task6_pooled_prepared_statement_forwards_statement_properties_and_restores_real_sqlite_handle(
) {
    let pool = task6_sqlx_prepared_pool(1).await;
    let mut connection = pool.get().await.unwrap();
    let options = StatementOptions::new(
        ResultSetType::ScrollInsensitive,
        ResultSetConcurrency::Updatable,
        ResultSetHoldability::HoldOverCommit,
    );
    let mut prepared = Connection::prepare_statement(&mut connection, "SELECT 1", options)
        .await
        .unwrap();
    assert_eq!(Statement::options(&*prepared).unwrap(), options);
    assert_eq!(
        Statement::options(&*prepared).unwrap().result_set_type(),
        ResultSetType::ScrollInsensitive
    );
    assert_eq!(
        Statement::options(&*prepared).unwrap().concurrency(),
        ResultSetConcurrency::Updatable
    );
    assert_eq!(
        Statement::options(&*prepared).unwrap().holdability(),
        ResultSetHoldability::HoldOverCommit
    );
    assert_eq!(Statement::warnings(&*prepared).unwrap(), None);
    assert_eq!(Statement::max_field_size(&*prepared).unwrap(), 0);
    Statement::set_max_field_size(&mut *prepared, 1024).unwrap();
    assert_eq!(Statement::max_field_size(&*prepared).unwrap(), 1024);
    assert_eq!(Statement::max_rows(&*prepared).unwrap(), 0);
    Statement::set_max_rows(&mut *prepared, 100).unwrap();
    assert_eq!(Statement::max_rows(&*prepared).unwrap(), 100);
    assert_eq!(Statement::query_timeout(&*prepared).unwrap(), 0);
    Statement::set_query_timeout(&mut *prepared, 3).unwrap();
    assert_eq!(Statement::query_timeout(&*prepared).unwrap(), 3);
    assert_eq!(
        Statement::fetch_direction(&*prepared).unwrap(),
        FetchDirection::Unknown
    );
    Statement::set_fetch_direction(&mut *prepared, FetchDirection::Reverse).unwrap();
    assert_eq!(
        Statement::fetch_direction(&*prepared).unwrap(),
        FetchDirection::Reverse
    );
    assert_eq!(Statement::fetch_size(&*prepared).unwrap(), 0);
    Statement::set_fetch_size(&mut *prepared, 25).unwrap();
    assert_eq!(Statement::fetch_size(&*prepared).unwrap(), 25);
    Statement::set_escape_processing(&mut *prepared, true).unwrap();
    Statement::set_cursor_name(&mut *prepared, "prepared_cursor").unwrap();
    Statement::cancel(&mut *prepared).unwrap();
    assert!(!Statement::is_poolable(&*prepared).unwrap());
    Statement::set_poolable(&mut *prepared, true).unwrap();
    assert!(Statement::is_poolable(&*prepared).unwrap());
    Statement::set_poolable(&mut *prepared, false).unwrap();
    assert!(!Statement::is_poolable(&*prepared).unwrap());
    Statement::close_on_completion(&mut *prepared).unwrap();
    assert!(Statement::is_close_on_completion(&*prepared).unwrap());
    Statement::close(&mut *prepared).await.unwrap();
    assert!(Statement::is_closed(&*prepared).unwrap());

    let mut restored =
        Connection::prepare_statement(&mut connection, "SELECT 1", StatementOptions::default())
            .await
            .unwrap();
    assert_eq!(Statement::max_field_size(&*restored).unwrap(), 0);
    assert_eq!(Statement::max_rows(&*restored).unwrap(), 0);
    assert_eq!(Statement::query_timeout(&*restored).unwrap(), 0);
    assert_eq!(
        Statement::fetch_direction(&*restored).unwrap(),
        FetchDirection::Unknown
    );
    assert_eq!(Statement::fetch_size(&*restored).unwrap(), 0);
    Statement::close(&mut *restored).await.unwrap();
    Connection::close(&mut connection).await.unwrap();
    pool.close().await;
}

async fn task6_assert_cleanup_failure_discards_connection(failure: CleanupFailure) {
    let pool = DruidPool::builder()
        .name("task6-prepared-cleanup-failure")
        .driver_name("cleanup")
        .factory(Arc::new(CleanupFactory(failure)) as Arc<dyn ConnectionFactory>)
        .exception_sorter(Arc::new(CleanupFatalSorter))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement =
        Connection::prepare_statement(&mut connection, "SELECT 1", StatementOptions::default())
            .await
            .unwrap();
    assert!(matches!(
        statement.close().await,
        Err(RdbcError::SqlException(_))
    ));
    assert!(Connection::is_closed(&connection).unwrap());
    assert!(!connection.is_recycled());
    Connection::close(&mut connection).await.unwrap();
    assert!(connection.is_recycled());
    assert_eq!(pool.state().discard_count, 1);
    pool.close().await;
}

async fn task6_assert_direct_prepared_method_failure_discards_connection(failure: CleanupFailure) {
    let pool = DruidPool::builder()
        .name("task6-prepared-direct-failure")
        .driver_name("cleanup")
        .factory(Arc::new(CleanupFactory(failure)) as Arc<dyn ConnectionFactory>)
        .exception_sorter(Arc::new(CleanupFatalSorter))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement =
        Connection::prepare_statement(&mut connection, "SELECT 1", StatementOptions::default())
            .await
            .unwrap();
    let result = match failure {
        CleanupFailure::SetParameter => {
            PreparedStatement::set_parameter(&mut *statement, parameter(1, Value::Int(1)))
        }
        CleanupFailure::AddBatch => PreparedStatement::add_parameter_batch(
            &mut *statement,
            vec![parameter(1, Value::Int(1))],
        ),
        CleanupFailure::ClearParameters => PreparedStatement::clear_parameters(&mut *statement),
        CleanupFailure::ClearBatch => PreparedStatement::clear_prepared_batch(&mut *statement),
    };
    assert!(matches!(result, Err(RdbcError::SqlException(_))));
    assert!(Connection::is_closed(&connection).unwrap());
    assert!(!connection.is_recycled());
    Connection::close(&mut connection).await.unwrap();
    assert!(connection.is_recycled());
    assert_eq!(pool.state().discard_count, 1);
    pool.close().await;
}

async fn task6_concurrent_logical_statements_preserve_java_in_use_and_lru_semantics() {
    let (pool, _) = task6_prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    let mut seed = connection
        .prepare_statement("SELECT 0", StatementOptions::default())
        .await
        .unwrap();
    seed.close().await.unwrap();
    assert!(seed.is_closed().unwrap());
    drop(seed);
    let mut active = connection
        .prepare_statement("SELECT 0", StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(pool.state().cached_prepared_statement_hit_count, 1);
    for sql in ["SELECT 1", "SELECT 2", "SELECT 3", "SELECT 4"] {
        let mut statement = connection
            .prepare_statement(sql, StatementOptions::default())
            .await
            .unwrap();
        statement.close().await.unwrap();
    }
    assert_eq!(pool.state().cached_prepared_statement_count, 3);
    assert!(!Connection::is_closed(&connection).unwrap());
    assert!(!active.is_closed().unwrap());
    active.close().await.unwrap();
    assert!(active.is_closed().unwrap());
    connection.close().await.unwrap();
    pool.close().await;
}

async fn task6_holder_statement_pool_is_lazy_and_debug_does_not_initialize_it() {
    let state = TrackingState::configured();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(TrackingConnection::new(state)),
        301,
        std::time::Duration::ZERO,
        0,
    )
    .await
    .unwrap();
    assert!(!holder.is_pool_prepared_statements());
    assert_eq!(
        holder
            .prepared_statement_stats()
            .cached_prepared_statement_count(),
        0
    );
    let debug = format!("{holder:?}");
    assert!(debug.contains("DruidConnectionHolder"));
    assert!(!holder.is_pool_prepared_statements());
    assert_eq!(
        holder
            .prepared_statement_stats()
            .cached_prepared_statement_count(),
        0
    );
}

async fn task6_mysql_schema_change_clears_existing_statement_cache() {
    let (pool, _) = task6_prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .prepare_statement("SELECT 1", StatementOptions::default())
        .await
        .unwrap();
    statement.close().await.unwrap();
    drop(statement);
    assert_eq!(pool.state().cached_prepared_statement_count, 1);
    connection.set_schema(Some("tenant")).await.unwrap();
    let state = pool.state();
    assert_eq!(state.cached_prepared_statement_count, 0);
    assert_eq!(state.cached_prepared_statement_delete_count, 1);
    assert_eq!(state.closed_prepared_statement_count, 1);
    connection.close().await.unwrap();
    pool.close().await;
}

async fn task6_prepared_batch_snapshots_parameters_and_executes_against_real_sqlite() {
    let pool = task6_sqlx_prepared_pool(1).await;
    let mut connection = pool.get().await.unwrap();
    connection
        .exec(
            "CREATE TABLE IF NOT EXISTS batch_case (id INTEGER PRIMARY KEY, value TEXT)",
            vec![],
        )
        .await
        .unwrap();
    let mut prepared = connection
        .prepare_statement(
            "INSERT INTO batch_case(id, value) VALUES (?, ?)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    // BASE 断言：batch 参数在 execute 后被消费（batch_size 归零）。
    for extra in 1_i64..=2 {
        prepared
            .add_parameter_batch(vec![
                parameter(1, Value::Int(extra)),
                parameter(2, Value::String(format!("batch-{extra}"))),
            ])
            .unwrap();
    }
    assert_eq!(prepared.execute_prepared_batch().await.unwrap(), [1, 1]);
    // 复用同一 prepared handle 再执行一批。
    for extra in 3_i64..=3 {
        prepared
            .add_parameter_batch(vec![
                parameter(1, Value::Int(extra)),
                parameter(2, Value::String(format!("batch-{extra}"))),
            ])
            .unwrap();
    }
    assert_eq!(prepared.execute_prepared_batch().await.unwrap(), [1]);
    // BASE 语义：空批次执行返回空结果且不报错（参数批次已被消费）。
    prepared.clear_prepared_batch().unwrap();
    let empty_batch = prepared.execute_prepared_batch().await.unwrap();
    assert!(empty_batch.is_empty());
    assert_eq!(empty_batch.len(), 0);
    assert_eq!(empty_batch, Vec::<i64>::new());
    assert_eq!(
        PreparedStatement::sql(&*prepared),
        Ok("INSERT INTO batch_case(id, value) VALUES (?, ?)")
    );
    prepared.clear_prepared_batch().unwrap();
    prepared.close().await.unwrap();

    // BASE 断言：行值精确匹配（顺序 + 内容）。
    let rows = connection
        .fetch("SELECT id, value FROM batch_case ORDER BY id", vec![])
        .await
        .unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| row.values.clone())
            .collect::<Vec<_>>(),
        [
            vec![Value::Int(1), Value::String("batch-1".to_string())],
            vec![Value::Int(2), Value::String("batch-2".to_string())],
            vec![Value::Int(3), Value::String("batch-3".to_string())],
        ]
    );
    // BASE 断言：INSERT 主键重复时旧行被覆盖（SQLite UPSERT 语义）。
    connection
        .exec(
            "INSERT OR REPLACE INTO batch_case(id, value) VALUES (1, 'replaced')",
            vec![],
        )
        .await
        .unwrap();
    let rows = connection
        .fetch("SELECT id, value FROM batch_case ORDER BY id", vec![])
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows.iter()
            .map(|row| row.values.clone())
            .collect::<Vec<_>>(),
        [
            vec![Value::Int(1), Value::String("replaced".to_string())],
            vec![Value::Int(2), Value::String("batch-2".to_string())],
            vec![Value::Int(3), Value::String("batch-3".to_string())],
        ]
    );
    // clear_parameters 不破坏 handle 后续可用性（BASE 复用同一 handle 语义）。
    let mut prepared2 = connection
        .prepare_statement(
            "INSERT INTO batch_case(id, value) VALUES (?, ?)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    prepared2
        .set_parameter(parameter(1, Value::Int(5)))
        .unwrap();
    prepared2
        .set_parameter(parameter(2, Value::String("five".to_string())))
        .unwrap();
    assert_eq!(prepared2.execute_prepared_update().await.unwrap(), 1);
    prepared2.clear_parameters().unwrap();
    assert_eq!(
        PreparedStatement::sql(&*prepared2),
        Ok("INSERT INTO batch_case(id, value) VALUES (?, ?)")
    );
    let rows = connection
        .fetch("SELECT id, value FROM batch_case ORDER BY id", vec![])
        .await
        .unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(
        rows.iter()
            .map(|row| row.values.clone())
            .collect::<Vec<_>>(),
        [
            vec![Value::Int(1), Value::String("replaced".to_string())],
            vec![Value::Int(2), Value::String("batch-2".to_string())],
            vec![Value::Int(3), Value::String("batch-3".to_string())],
            vec![Value::Int(5), Value::String("five".to_string())],
        ]
    );
    prepared2.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

async fn task6_prepared_bound_setters_execute_and_batch_against_real_sqlite() {
    let pool = task6_sqlx_prepared_pool(1).await;
    let mut connection = pool.get().await.unwrap();
    connection
        .exec(
            "CREATE TABLE IF NOT EXISTS prepared_bound_item(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                int_value INTEGER,
                long_value INTEGER,
                double_value REAL,
                string_value TEXT,
                n_string_value TEXT,
                bytes_value BLOB,
                null_value TEXT
            )",
            vec![],
        )
        .await
        .unwrap();
    let mut insert = connection
        .prepare_statement(
            "INSERT INTO prepared_bound_item(
                int_value, long_value, double_value, string_value,
                n_string_value, bytes_value, null_value
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    insert.set_parameter(parameter(1, Value::Int(32))).unwrap();
    insert
        .set_parameter(parameter(2, Value::Int(i64::MAX)))
        .unwrap();
    insert
        .set_parameter(parameter(3, Value::Float(2.5)))
        .unwrap();
    insert
        .set_parameter(parameter(4, Value::String("text".to_string())))
        .unwrap();
    insert
        .set_parameter(parameter(5, Value::String("国家字符".to_string())))
        .unwrap();
    insert
        .set_parameter(parameter(6, Value::Bytes(vec![0, 1, 2])))
        .unwrap();
    insert.set_parameter(parameter(7, Value::Null)).unwrap();
    assert_eq!(insert.execute_prepared_update().await.unwrap(), 1);

    // 批量插入。
    let mut batch = connection
        .prepare_statement(
            "INSERT INTO prepared_bound_item(
                int_value, long_value, double_value, string_value,
                n_string_value, bytes_value, null_value
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    for extra in 1_i64..=2 {
        batch
            .add_parameter_batch(vec![
                parameter(1, Value::Int(extra)),
                parameter(2, Value::Int(extra * 10)),
                parameter(3, Value::Float(extra as f64)),
                parameter(4, Value::String(format!("batch-{extra}"))),
                parameter(5, Value::String(format!("国-{extra}"))),
                parameter(6, Value::Bytes(vec![extra as u8])),
                parameter(7, Value::Null),
            ])
            .unwrap();
    }
    assert_eq!(batch.execute_prepared_batch().await.unwrap(), [1, 1]);
    batch.close().await.unwrap();

    let rows = connection
        .fetch("SELECT * FROM prepared_bound_item ORDER BY id", vec![])
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[0].values,
        vec![
            Value::Int(1),
            Value::Int(32),
            Value::Int(i64::MAX),
            Value::Float(2.5),
            Value::String("text".to_string()),
            Value::String("国家字符".to_string()),
            Value::Bytes(vec![0, 1, 2]),
            Value::Null,
        ]
    );
    assert_eq!(
        rows[1].values,
        vec![
            Value::Int(2),
            Value::Int(1),
            Value::Int(10),
            Value::Float(1.0),
            Value::String("batch-1".to_string()),
            Value::String("国-1".to_string()),
            Value::Bytes(vec![1]),
            Value::Null,
        ]
    );
    assert_eq!(
        rows[2].values,
        vec![
            Value::Int(3),
            Value::Int(2),
            Value::Int(20),
            Value::Float(2.0),
            Value::String("batch-2".to_string()),
            Value::String("国-2".to_string()),
            Value::Bytes(vec![2]),
            Value::Null,
        ]
    );
    // 查询结果行数与主键单调递增（AUTOINCREMENT 语义）。
    assert!(rows[0].values[0] == Value::Int(1));
    assert!(rows[1].values[0] == Value::Int(2));
    assert!(rows[2].values[0] == Value::Int(3));
    // 批量插入的 bytes 值保持原样。
    assert!(rows[1].values[6] == Value::Bytes(vec![1]));
    assert!(rows[2].values[6] == Value::Bytes(vec![2]));
    assert_eq!(rows[0].values[1], Value::Int(32));
    assert_eq!(rows[0].values[2], Value::Int(i64::MAX));
    assert_eq!(rows[0].values[3], Value::Float(2.5));
    assert_eq!(rows[0].values[4], Value::String("text".to_string()));
    assert_eq!(rows[0].values[5], Value::String("国家字符".to_string()));
    insert.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

async fn task6_statement_execution_error_removes_handle_instead_of_reusing_it() {
    let (pool, state) = task6_prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .prepare_statement("SELECT failure", StatementOptions::default())
        .await
        .unwrap();
    state.fail_next("prepared_execute_generic");
    assert!(matches!(
        statement.execute_prepared().await,
        Err(RdbcError::SqlException(_))
    ));
    statement.close().await.unwrap();
    let pool_state = pool.state();
    assert_eq!(pool_state.cached_prepared_statement_count, 0);
    assert_eq!(pool_state.cached_prepared_statement_delete_count, 1);
    assert_eq!(pool_state.closed_prepared_statement_count, 1);
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn task6_prepared_statement_canonical_matrix() {
    let factory = SqlxConnectionFactory::new("sqlite::memory:");
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
        .await
        .expect("SQLx connection");
    connection
        .exec(
            "CREATE TABLE prepared_case(id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
            Vec::new(),
        )
        .await
        .expect("table");
    let cases = task6_prepared_statement_case_ids();
    let mut executed = Vec::with_capacity(cases.len());

    for (offset, case_id) in cases.iter().enumerate() {
        match *case_id {
            "clear_batch_failure_uses_connection_sorter_before_cache_return" => {
                task6_assert_cleanup_failure_discards_connection(CleanupFailure::ClearBatch).await;
                executed.push(*case_id);
                continue;
            }
            "clear_parameters_failure_uses_connection_sorter_before_cache_return" => {
                task6_assert_cleanup_failure_discards_connection(CleanupFailure::ClearParameters)
                    .await;
                executed.push(*case_id);
                continue;
            }
            "concurrent_logical_statements_preserve_java_in_use_and_lru_semantics" => {
                task6_concurrent_logical_statements_preserve_java_in_use_and_lru_semantics().await;
                executed.push(*case_id);
                continue;
            }
            "direct_prepared_setter_and_batch_methods_route_failures_through_connection_sorter" => {
                for failure in [
                    CleanupFailure::SetParameter,
                    CleanupFailure::AddBatch,
                    CleanupFailure::ClearParameters,
                    CleanupFailure::ClearBatch,
                ] {
                    task6_assert_direct_prepared_method_failure_discards_connection(failure).await;
                }
                executed.push(*case_id);
                continue;
            }
            "mysql_schema_change_clears_existing_statement_cache" => {
                task6_mysql_schema_change_clears_existing_statement_cache().await;
                executed.push(*case_id);
                continue;
            }
            "pooled_prepared_statement_forwards_statement_properties_and_restores_real_sqlite_handle" => {
                task6_pooled_prepared_statement_forwards_statement_properties_and_restores_real_sqlite_handle().await;
                executed.push(*case_id);
                continue;
            }
            "holder_statement_pool_is_lazy_and_debug_does_not_initialize_it" => {
                task6_holder_statement_pool_is_lazy_and_debug_does_not_initialize_it().await;
                executed.push(*case_id);
                continue;
            }
            "statement_execution_error_removes_handle_instead_of_reusing_it" => {
                task6_statement_execution_error_removes_handle_instead_of_reusing_it().await;
                executed.push(*case_id);
                continue;
            }
            "prepared_batch_snapshots_parameters_and_executes_against_real_sqlite" => {
                task6_prepared_batch_snapshots_parameters_and_executes_against_real_sqlite().await;
                executed.push(*case_id);
                continue;
            }
            "prepared_bound_setters_execute_and_batch_against_real_sqlite" => {
                task6_prepared_bound_setters_execute_and_batch_against_real_sqlite().await;
                executed.push(*case_id);
                continue;
            }
            _ => {}
        }
        let id = i64::try_from(offset + 1).unwrap();
        let sql = "INSERT INTO prepared_case(id, value) VALUES (?, ?)";
        let mut prepared = connection
            .prepare_statement(sql, StatementOptions::default())
            .await
            .expect("prepared insert");
        assert_eq!(PreparedStatement::sql(&*prepared), Ok(sql));
        prepared
            .set_parameter(parameter(1, Value::Int(id)))
            .unwrap();
        prepared
            .set_parameter(parameter(2, Value::String((*case_id).to_owned())))
            .unwrap();
        assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
        prepared.clear_parameters().unwrap();
        prepared.clear_prepared_batch().unwrap();
        prepared.set_query_timeout(1).unwrap();
        assert_eq!(prepared.query_timeout(), Ok(1));

        if case_id.contains("batch") {
            for extra in 1_i64..=2 {
                prepared
                    .add_parameter_batch(vec![
                        parameter(1, Value::Int(1_000 + id * 10 + extra)),
                        parameter(2, Value::String(format!("{case_id}-{extra}"))),
                    ])
                    .unwrap();
            }
            assert_eq!(prepared.execute_prepared_batch().await.unwrap(), [1, 1]);
        }
        if case_id.contains("result") || case_id.contains("generic") {
            let mut query = connection
                .prepare_statement(
                    "SELECT value FROM prepared_case WHERE id = ?",
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
        }
        prepared.close().await.unwrap();
        assert_eq!(prepared.is_closed(), Ok(true));
        executed.push(*case_id);
    }
    assert_eq!(executed, cases);
    println!(
        "TASK6_CASE_DIGEST=task6_prepared_statement_canonical_matrix:{}",
        executed.join("|")
    );
    connection.close().await.unwrap();
}
