//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Canonical pooled `Statement` query/update/batch/multi-result/lifecycle semantics.

use druid::core::{
    Connection, ConnectionFactory, DruidConnectionHolder, DruidPooledConnection, ExecResult,
    RdbcError, Row, Value,
};
use druid::pool::DruidPool;
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, ResultSetMetaData, ResultSetMetaDataSnapshot,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::statement::{
    CurrentResultDisposition, ExecutionResult, FetchDirection, GeneratedKeys, ResultSetConcurrency,
    ResultSetHoldability, ResultSetType, Statement, StatementOptions,
};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper, WrapperExt};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::any::TypeId;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

async fn make_connection(id: u64) -> DruidPooledConnection {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory");
    pooled(factory.create().await.unwrap(), id).await
}

async fn pooled(raw: Box<dyn Connection>, id: u64) -> DruidPooledConnection {
    let holder = DruidConnectionHolder::with_standard_connection(raw, id, Duration::ZERO, 0)
        .await
        .expect("holder");
    DruidPooledConnection::with_standard_holder(
        holder,
        "statement-semantics".to_owned(),
        None,
        false,
        false,
        Box::new(|_, _| true),
    )
}

async fn create_statement(
    connection: &mut DruidPooledConnection,
    options: StatementOptions,
) -> Box<dyn Statement> {
    Connection::create_statement(connection, options)
        .await
        .expect("statement")
}

struct RowsResultSet {
    rows: VecDeque<Row>,
    current: Option<Row>,
    closed: bool,
}

impl RowsResultSet {
    fn new(rows: Vec<Row>) -> Self {
        Self {
            rows: rows.into(),
            current: None,
            closed: false,
        }
    }
}

druid_rdbc::impl_standard_wrapper!(RowsResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for RowsResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.current = self.rows.pop_front();
        Ok(self.current.is_some())
    }

    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.current
            .as_ref()
            .and_then(|row| row.get(column.get() - 1))
            .cloned()
            .ok_or_else(|| RdbcError::InvalidArgument("no current row".to_owned()))
    }

    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(vec![
            ColumnDescriptor::new("value", "BIGINT", ColumnNullability::NoNulls),
        ])))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

struct MultiResultStatement {
    options: StatementOptions,
    closed: bool,
    generated_keys: Arc<Mutex<Option<GeneratedKeys>>>,
}

druid_rdbc::impl_standard_wrapper!(MultiResultStatement => Statement);

#[async_trait::async_trait]
impl Statement for MultiResultStatement {
    fn options(&self) -> Result<StatementOptions, RdbcError> {
        Ok(self.options)
    }

    async fn execute(
        &mut self,
        _sql: &str,
        keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        *self
            .generated_keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(keys);
        Ok(vec![
            ExecutionResult::ResultSet(Box::new(RowsResultSet::new(vec![Row::new(vec![
                Value::Int(10),
            ])]))),
            ExecutionResult::UpdateCount(3),
            ExecutionResult::ResultSet(Box::new(RowsResultSet::new(vec![Row::new(vec![
                Value::Int(20),
            ])]))),
        ])
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

struct MultiResultConnection {
    generated_keys: Arc<Mutex<Option<GeneratedKeys>>>,
}
druid_rdbc::impl_standard_wrapper!(MultiResultConnection => Connection);

#[async_trait::async_trait]
impl Connection for MultiResultConnection {
    async fn create_statement(
        &mut self,
        options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        Ok(Box::new(MultiResultStatement {
            options,
            closed: false,
            generated_keys: Arc::clone(&self.generated_keys),
        }))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

#[allow(dead_code)]
async fn task6_public_standard_statement() -> Result<Box<dyn Statement>, RdbcError> {
    let factory = druid_wrapper::sqlx::SqlxConnectionFactory::new("sqlite::memory:");
    let mut connection = ConnectionFactory::create(&factory).await?;
    Connection::create_statement(&mut *connection, StatementOptions::default()).await
}

#[allow(dead_code)]
async fn task6_public_standard_prepared_statement(
    sql: &str,
) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
    let factory = druid_wrapper::sqlx::SqlxConnectionFactory::new("sqlite::memory:");
    let mut connection = ConnectionFactory::create(&factory).await?;
    Connection::prepare_statement(&mut *connection, sql, StatementOptions::default()).await
}

#[tokio::test]
async fn task6_pooled_statement_semantics_test_sqlite_statement_executes_query_update_and_batch_on_same_lease(
) {
    let mut connection = make_connection(7).await;
    let mut statement = create_statement(&mut connection, StatementOptions::default()).await;
    assert_eq!(
        statement.execute_update("CREATE TABLE statement_event(id INTEGER PRIMARY KEY AUTOINCREMENT, label TEXT NOT NULL)", GeneratedKeys::None).await.unwrap(),
        0
    );
    assert_eq!(
        statement
            .execute_update(
                "INSERT INTO statement_event(label) VALUES ('single')",
                GeneratedKeys::None
            )
            .await
            .unwrap(),
        1
    );
    assert_eq!(statement.get_update_count().unwrap(), Some(1));
    statement
        .add_batch("INSERT INTO statement_event(label) VALUES ('batch-1')".to_owned())
        .unwrap();
    statement
        .add_batch("INSERT INTO statement_event(label) VALUES ('batch-2')".to_owned())
        .unwrap();
    let batch_counts = statement.execute_batch().await.unwrap();
    assert_eq!(batch_counts, vec![1, 1]);
    assert_eq!(batch_counts[0], 1);
    assert_eq!(batch_counts[1], 1);
    statement.clear_batch().unwrap();
    assert_eq!(statement.batch().unwrap(), Vec::<String>::new());
    let mut rows = statement
        .execute_query("SELECT id FROM statement_event ORDER BY id")
        .await
        .unwrap();
    let mut ids = Vec::new();
    while rows.next().await.unwrap() {
        ids.push(rows.long(ColumnIndex::new(1).unwrap()).unwrap());
    }
    assert_eq!(ids, vec![1, 2, 3]);
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[0], 1);
    assert_eq!(ids[1], 2);
    assert_eq!(ids[2], 3);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_semantics_test_sqlite_generic_execute_preserves_first_result_generated_keys_and_more_results(
) {
    let mut connection = make_connection(8).await;
    let mut statement = create_statement(&mut connection, StatementOptions::default()).await;
    statement
        .execute_update(
            "CREATE TABLE generic_event(id INTEGER PRIMARY KEY AUTOINCREMENT, label TEXT)",
            GeneratedKeys::None,
        )
        .await
        .unwrap();

    let mut query_results = statement
        .execute("SELECT 41", GeneratedKeys::None)
        .await
        .unwrap();
    assert_eq!(query_results.len(), 1);
    let ExecutionResult::ResultSet(mut current) = query_results.remove(0) else {
        panic!("SELECT must yield ResultSet");
    };
    assert!(current.next().await.unwrap());
    assert_eq!(current.long(ColumnIndex::new(1).unwrap()).unwrap(), 41);
    assert_eq!(current.meta_data().unwrap().column_count(), 1);
    assert_eq!(statement.get_update_count().unwrap(), None);
    assert!(!statement
        .get_more_results(CurrentResultDisposition::Default)
        .await
        .unwrap());
    assert!(current.is_closed().unwrap());

    // 对应 Java 1.2.28 StatementTest#test_stmt：SELECT 后使用 CLOSE_CURRENT_RESULT
    // 推进到无后续结果，返回 false，且当前游标被关闭。
    let mut java_shape_results = statement
        .execute("SELECT 42", GeneratedKeys::None)
        .await
        .unwrap();
    let ExecutionResult::ResultSet(java_shape_cursor) = java_shape_results.remove(0) else {
        panic!("SELECT must yield ResultSet");
    };
    assert!(!statement
        .get_more_results_current(druid::core::DruidPooledStatement::CLOSE_CURRENT_RESULT)
        .await
        .unwrap());
    assert!(java_shape_cursor.is_closed().unwrap());

    let plain_results = statement
        .execute(
            "INSERT INTO generic_event(label) VALUES ('plain')",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    assert!(matches!(
        plain_results.first(),
        Some(ExecutionResult::UpdateCount(1))
    ));
    assert_eq!(statement.get_update_count().unwrap(), Some(1));
    let mut plain_keys = statement.generated_keys().await.unwrap();
    assert!(plain_keys.next().await.unwrap());
    assert!(plain_keys.long(ColumnIndex::new(1).unwrap()).unwrap() > 0);
    plain_keys.close().await.unwrap();

    let results = statement
        .execute(
            "INSERT INTO generic_event(label) VALUES ('g')",
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .unwrap();
    assert!(matches!(
        results.first(),
        Some(ExecutionResult::UpdateCount(1))
    ));
    assert_eq!(statement.get_update_count().unwrap(), Some(1));
    let mut keys = statement.generated_keys().await.unwrap();
    assert!(keys.next().await.unwrap());
    assert!(keys.long(ColumnIndex::new(1).unwrap()).unwrap() > 0);
    keys.close().await.unwrap();

    assert!(statement
        .execute(
            "INSERT INTO generic_event(label) VALUES ('bad-index')",
            GeneratedKeys::ColumnIndexes(vec![ColumnIndex::new(1).unwrap()]),
        )
        .await
        .is_err());
    assert_eq!(statement.get_update_count().unwrap(), None);
    let mut empty_keys = statement.generated_keys().await.unwrap();
    assert!(!empty_keys.next().await.unwrap());
    empty_keys.close().await.unwrap();

    assert!(statement
        .execute(
            "INSERT INTO generic_event(label) VALUES ('bad-name')",
            GeneratedKeys::ColumnNames(vec!["id".to_owned()]),
        )
        .await
        .is_err());
    assert_eq!(statement.get_update_count().unwrap(), None);
    let mut empty_named_keys = statement.generated_keys().await.unwrap();
    assert!(!empty_named_keys.next().await.unwrap());
    empty_named_keys.close().await.unwrap();

    assert!(!statement
        .get_more_results(CurrentResultDisposition::CloseAll)
        .await
        .unwrap());
    assert_eq!(statement.get_update_count().unwrap(), None);
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_semantics_test_generic_execute_preserves_overload_arguments_and_ordered_multi_results(
) {
    let generated_keys = Arc::new(Mutex::new(None));
    let mut connection = pooled(
        Box::new(MultiResultConnection {
            generated_keys: Arc::clone(&generated_keys),
        }),
        9,
    )
    .await;
    let mut statement = create_statement(&mut connection, StatementOptions::default()).await;
    let mut results = statement
        .execute(
            "CALL multi",
            GeneratedKeys::ColumnNames(vec!["id".to_owned()]),
        )
        .await
        .unwrap();
    assert_eq!(statement.options().unwrap(), StatementOptions::default());
    assert_eq!(results.len(), 3);
    assert!(matches!(
        results.first(),
        Some(ExecutionResult::ResultSet(_))
    ));
    assert!(matches!(
        results.get(1),
        Some(ExecutionResult::UpdateCount(3))
    ));
    assert!(matches!(
        results.get(2),
        Some(ExecutionResult::ResultSet(_))
    ));
    let ExecutionResult::ResultSet(mut first) = results.remove(0) else {
        panic!("first result set");
    };
    assert!(first.next().await.unwrap());
    assert_eq!(first.long(ColumnIndex::new(1).unwrap()).unwrap(), 10);
    assert!(!first.next().await.unwrap());
    assert_eq!(
        *generated_keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        Some(GeneratedKeys::ColumnNames(vec!["id".to_owned()]))
    );
    assert!(matches!(results.remove(0), ExecutionResult::UpdateCount(3)));
    let ExecutionResult::ResultSet(mut last) = results.remove(0) else {
        panic!("last result set");
    };
    assert!(last.next().await.unwrap());
    assert_eq!(last.long(ColumnIndex::new(1).unwrap()).unwrap(), 20);
    assert!(!last.next().await.unwrap());
    assert!(!statement
        .get_more_results(CurrentResultDisposition::Default)
        .await
        .unwrap());
    assert!(first.is_closed().unwrap());
    assert_eq!(statement.get_update_count().unwrap(), Some(3));
    assert!(statement
        .get_more_results(CurrentResultDisposition::KeepCurrent)
        .await
        .unwrap());
    assert_eq!(statement.get_update_count().unwrap(), None);
    assert!(!last.is_closed().unwrap());
    assert!(!statement
        .get_more_results(CurrentResultDisposition::CloseAll)
        .await
        .unwrap());
    assert!(last.is_closed().unwrap());
    assert_eq!(statement.get_update_count().unwrap(), None);
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_semantics_test_statement_overloads_preserve_result_set_options_and_properties(
) {
    let mut connection = make_connection(10).await;
    let options = StatementOptions::new(
        ResultSetType::ScrollInsensitive,
        ResultSetConcurrency::Updatable,
        ResultSetHoldability::HoldOverCommit,
    );
    let mut statement = create_statement(&mut connection, options).await;
    assert_eq!(statement.options().unwrap(), options);
    assert_eq!(
        statement.options().unwrap().result_set_type(),
        ResultSetType::ScrollInsensitive
    );
    assert_eq!(
        statement.options().unwrap().concurrency(),
        ResultSetConcurrency::Updatable
    );
    assert_eq!(
        statement.options().unwrap().holdability(),
        ResultSetHoldability::HoldOverCommit
    );
    assert_eq!(statement.max_field_size().unwrap(), 0);
    assert_eq!(statement.max_rows().unwrap(), 0);
    assert_eq!(statement.query_timeout().unwrap(), 0);
    assert_eq!(
        statement.fetch_direction().unwrap(),
        FetchDirection::Unknown
    );
    assert_eq!(statement.fetch_size().unwrap(), 0);
    statement.set_max_field_size(64).unwrap();
    statement.set_max_rows(7).unwrap();
    statement.set_query_timeout(3).unwrap();
    statement
        .set_fetch_direction(FetchDirection::Reverse)
        .unwrap();
    statement.set_fetch_size(5).unwrap();
    assert_eq!(statement.max_field_size().unwrap(), 64);
    assert_eq!(statement.max_rows().unwrap(), 7);
    assert_eq!(statement.query_timeout().unwrap(), 3);
    assert_eq!(
        statement.fetch_direction().unwrap(),
        FetchDirection::Reverse
    );
    assert_eq!(statement.fetch_size().unwrap(), 5);
    statement.set_escape_processing(true).unwrap();
    statement.set_cursor_name("typed-options").unwrap();
    statement.cancel().unwrap();
    assert!(!statement.is_poolable().unwrap());
    statement.set_poolable(true).unwrap();
    assert!(statement.is_poolable().unwrap());
    statement.set_poolable(false).unwrap();
    assert!(!statement.is_poolable().unwrap());
    statement.close_on_completion().unwrap();
    assert!(statement.is_close_on_completion().unwrap());
    assert!(statement.warnings().unwrap().is_none());
    statement.clear_warnings().unwrap();
    assert!(statement.warnings().unwrap().is_none());
    assert!(Wrapper::is_wrapper_for(&*statement, Some(TypeId::of::<dyn Statement>())).unwrap());
    assert!(!Wrapper::is_wrapper_for(&*statement, Some(TypeId::of::<String>())).unwrap());
    assert!(Wrapper::unwrap(&*statement, Some(TypeId::of::<String>())).is_err());
    statement.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_statement_semantics_test_statement_close_and_old_lease_rejection_match_java_lifecycle(
) {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let pool = DruidPool::builder()
        .name("stale-statement-lifecycle")
        .driver_name("toasty-sqlite")
        .factory(Arc::new(factory) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();
    let mut first = pool.get().await.unwrap();
    let mut statement = create_statement(&mut first, StatementOptions::default()).await;
    first.close().await.unwrap();
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 1);

    let mut second = pool.get().await.unwrap();
    assert_eq!(pool.state().active_count, 1);
    assert_eq!(second.fetch("SELECT 1", Vec::new()).await.unwrap().len(), 1);
    assert!(matches!(
        statement.execute_query("SELECT 1").await,
        Err(RdbcError::ConnectionDiscarded)
    ));
    assert!(matches!(
        statement
            .execute_update("CREATE TABLE stale(id INTEGER)", GeneratedKeys::None)
            .await,
        Err(RdbcError::ConnectionDiscarded)
    ));
    assert!(matches!(
        statement.execute_query("SELECT 2").await,
        Err(RdbcError::ConnectionDiscarded)
    ));
    statement.close().await.unwrap();
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
    assert!(matches!(
        statement.warnings(),
        Err(RdbcError::ConnectionDiscarded)
    ));
    second.close().await.unwrap();
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 1);
    assert_eq!(pool.state().recycle_count, 2);
    pool.close().await;
}

#[tokio::test]
async fn task6_pooled_statement_semantics_test_statement_errors_batch_failure_and_wrapper_branches_are_observable(
) {
    let mut connection = make_connection(12).await;
    let mut statement = create_statement(&mut connection, StatementOptions::default()).await;
    let query_error = match statement.execute_query("NOT A SQLITE QUERY").await {
        Ok(_) => panic!("invalid query must fail"),
        Err(error) => error,
    };
    assert!(query_error.sql_exception().is_some());
    assert_eq!(
        query_error.class_name(),
        "toasty_core::error::DriverOperationFailed"
    );
    let update_error = statement
        .execute_update("NOT A SQLITE UPDATE", GeneratedKeys::None)
        .await
        .unwrap_err();
    assert!(update_error.sql_exception().is_some());
    statement
        .add_batch("CREATE TABLE batch_ok(id INTEGER)".to_owned())
        .unwrap();
    statement
        .add_batch("NOT A SQLITE BATCH".to_owned())
        .unwrap();
    let error = statement.execute_batch().await.unwrap_err();
    assert!(matches!(error, RdbcError::BatchUpdateException { .. }));
    assert_eq!(error.batch_update_counts().unwrap().large(), &[0]);
    assert!(error.sql_exception().is_some());
    assert_eq!(error.sql_exception().unwrap().sql_state(), None);
    assert_eq!(statement.get_update_count().unwrap(), None);
    assert!(matches!(
        statement.get_result_set().await,
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_get_result_set"
        })
    ));
    assert!(Wrapper::is_wrapper_for(&*statement, None).is_err());
    assert!(Wrapper::is_wrapper_for(&*statement, Some(TypeId::of::<dyn Statement>())).unwrap());
    assert!(!Wrapper::is_wrapper_for(&*statement, Some(TypeId::of::<String>())).unwrap());
    assert!(Wrapper::unwrap(&*statement, Some(TypeId::of::<String>())).is_err());
    assert!(matches!(
        Wrapper::unwrap(&*statement, Some(TypeId::of::<dyn Statement>())),
        Ok(StandardUnwrapped::Statement(_))
    ));
    assert!(Wrapper::unwrap(&*statement, None).is_err());
    let concrete = MultiResultStatement {
        options: StatementOptions::default(),
        closed: false,
        generated_keys: Arc::new(Mutex::new(None)),
    };
    assert!(concrete.is_wrapper_for_type::<MultiResultStatement>());
    assert!(concrete.unwrap_ref::<String>().is_err());
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_semantics_test_standard_statement_rejects_invalid_values_and_closed_access(
) {
    let mut statement = task6_public_standard_statement().await.unwrap();
    assert!(matches!(
        statement.set_max_field_size(-1),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(matches!(
        statement.set_max_rows(-1),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(matches!(
        statement.set_query_timeout(-1),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(matches!(
        statement.set_fetch_size(-1),
        Err(RdbcError::InvalidArgument(_))
    ));
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
    assert!(matches!(
        statement.execute_query("SELECT 1").await,
        Err(RdbcError::ConnectionDiscarded)
    ));
    assert!(matches!(
        statement.add_batch("SELECT 1".to_owned()),
        Err(RdbcError::ConnectionDiscarded)
    ));
}
