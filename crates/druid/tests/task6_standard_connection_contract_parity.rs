//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! `DruidPooledConnection -> Connection` 垂直切片合同测试。
//!
//! 语义来源：
//! - `DruidPooledConnection#close/recycle`
//! - `DruidDataSource#getConnectionInternal/recycle`
//! - `FilterChainImpl` 的前向进入、逆向退出调用链

use druid::core::{
    AfterFilter, BeforeFilter, Connection, ConnectionFactory, DruidConnectionHolder, DruidError,
    ExecContext, ExecResult, FilterChain, PreparedInputParameter, PreparedStatement, RdbcError,
    ResultSetColumnMeta, ResultSetMetaData, ResultSetMetaDataSnapshot, ResultSetNullability, Row,
    StatementGeneratedKeys, Value,
};
use druid::pool::DruidPool;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::statement::{
    ExecutionResult, ParameterIndex, PreparedParameter, Statement, StatementOptions,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{Notify, Semaphore};

struct ContractConnection;

struct ContractStatement;

druid_rdbc::impl_standard_wrapper!(ContractStatement => Statement);

#[async_trait::async_trait]
impl Statement for ContractStatement {
    async fn execute_query(&mut self, _sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        Ok(Box::new(ContractResultSet::new(vec![
            Row::new(vec![Value::Int(1)]),
            Row::new(vec![Value::Int(2)]),
        ])))
    }

    fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        Ok(None)
    }
}

struct ContractResultSet {
    rows: VecDeque<Row>,
    current: Option<Row>,
    closed: bool,
}

impl ContractResultSet {
    fn new(rows: Vec<Row>) -> Self {
        Self {
            rows: rows.into(),
            current: None,
            closed: false,
        }
    }
}

druid_rdbc::impl_standard_wrapper!(ContractResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for ContractResultSet {
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
            ResultSetColumnMeta::new("id", "BIGINT", ResultSetNullability::NoNulls),
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

druid_rdbc::impl_standard_wrapper!(ContractConnection => Connection);

#[async_trait::async_trait]
impl Connection for ContractConnection {
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        Ok(Box::new(ContractStatement))
    }

    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        Ok(ExecResult {
            rows_affected: 1,
            last_insert_id: Some(7),
            row_count: None,
        })
    }

    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        Ok(vec![
            Row::new(vec![Value::Int(1)]),
            Row::new(vec![Value::Int(2)]),
        ])
    }

    async fn begin(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("contract".to_owned())
    }
}

struct ContractFactory;

#[async_trait::async_trait]
impl ConnectionFactory for ContractFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(ContractConnection))
    }

    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        connection.ping().await
    }
}

#[derive(Clone)]
struct BeforeSnapshot {
    sql: String,
    params: Vec<Value>,
    data_source: String,
    start: Instant,
}

struct ContextContractFilter {
    before: Mutex<Option<BeforeSnapshot>>,
    after_count: AtomicUsize,
    row_counts: Mutex<Vec<Option<u64>>>,
}

impl ContextContractFilter {
    fn new() -> Self {
        Self {
            before: Mutex::new(None),
            after_count: AtomicUsize::new(0),
            row_counts: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl BeforeFilter for ContextContractFilter {
    fn name(&self) -> &'static str {
        "context-contract"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        *self.before.lock().expect("before snapshot lock poisoned") = Some(BeforeSnapshot {
            sql: context.sql.clone(),
            params: context.params.to_vec(),
            data_source: context.data_source.to_string(),
            start: context.start,
        });
        context.fingerprint = Some(0xD2_01_D2_01);
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for ContextContractFilter {
    fn name(&self) -> &'static str {
        "context-contract"
    }

    async fn after(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        // 主调用必须 await after；主动让出一次调度可捕获“创建 future 但未轮询”的回归。
        tokio::task::yield_now().await;
        let before = self
            .before
            .lock()
            .expect("before snapshot lock poisoned")
            .clone()
            .expect("after must observe a preceding before");
        assert_eq!(context.sql, before.sql);
        assert_eq!(context.params, before.params);
        assert_eq!(context.data_source, before.data_source);
        assert_eq!(context.start, before.start);
        assert_eq!(context.fingerprint, Some(0xD2_01_D2_01));
        assert_eq!(context.data_source, "contract-data-source");

        let row_count = result.as_ref().ok().and_then(|value| value.row_count);
        self.row_counts
            .lock()
            .expect("row count lock poisoned")
            .push(row_count);
        self.after_count.fetch_add(1, Ordering::Release);
        Ok(())
    }
}

async fn contract_pool(filter: Arc<ContextContractFilter>) -> DruidPool {
    let mut filter_chain = FilterChain::new();
    filter_chain.add_before(filter.clone());
    filter_chain.add_after(filter);
    DruidPool::builder()
        .name("contract-data-source")
        .driver_name("contract")
        .factory(Arc::new(ContractFactory))
        .max_open(2)
        .max_idle(2)
        .filter_chain(Arc::new(filter_chain))
        .build()
        .await
        .expect("contract pool must build")
}

struct GatedFactory {
    entered: AtomicUsize,
    entered_notify: Notify,
    release: Semaphore,
}

#[async_trait::async_trait]
impl ConnectionFactory for GatedFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        self.entered.fetch_add(1, Ordering::AcqRel);
        self.entered_notify.notify_one();
        self.release
            .acquire()
            .await
            .expect("release semaphore must remain open")
            .forget();
        Ok(Box::new(ContractConnection))
    }

    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        connection.ping().await
    }
}

#[allow(dead_code)]
async fn task6_public_standard_statement(
) -> Result<Box<dyn druid_rdbc::statement::Statement>, druid_rdbc::error::RdbcError> {
    let factory = druid_wrapper::sqlx::SqlxConnectionFactory::new("sqlite::memory:");
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory).await?;
    druid_rdbc::connection::Connection::create_statement(
        &mut *connection,
        druid_rdbc::statement::StatementOptions::default(),
    )
    .await
}

#[allow(dead_code)]
async fn task6_public_standard_prepared_statement(
    sql: &str,
) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, druid_rdbc::error::RdbcError> {
    let factory = druid_wrapper::sqlx::SqlxConnectionFactory::new("sqlite::memory:");
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory).await?;
    if sql.contains("prepared_item") {
        druid_rdbc::connection::Connection::exec(
            &mut *connection,
            "CREATE TABLE prepared_item(value TEXT)",
            Vec::new(),
        )
        .await?;
    }
    let statement = druid_rdbc::connection::Connection::prepare_statement(
        &mut *connection,
        sql,
        druid_rdbc::statement::StatementOptions::default(),
    )
    .await?;
    Ok(statement)
}

#[tokio::test]
async fn task6_standard_connection_contract_exec_and_fetch_preserve_one_filter_context() {
    let filter = Arc::new(ContextContractFilter::new());
    let pool = contract_pool(filter.clone()).await;
    let mut connection = pool
        .get()
        .await
        .expect("connection acquisition must succeed");

    assert_eq!(
        connection.warnings().await,
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_warnings"
        })
    );
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .expect("default physical statement must be created");
    assert_eq!(statement.warnings().unwrap(), None);
    statement.clear_warnings().unwrap();

    let result = connection
        .exec("UPDATE account SET balance = ?", vec![Value::Int(7)])
        .await
        .expect("exec must succeed");
    assert_eq!(result.rows_affected, 1);
    assert_eq!(filter.after_count.load(Ordering::Acquire), 1);

    let rows = connection
        .fetch(
            "SELECT id FROM account WHERE owner = ?",
            vec![Value::String("alice".to_string())],
        )
        .await
        .expect("fetch must succeed");
    assert_eq!(rows.len(), 2);
    assert_eq!(filter.after_count.load(Ordering::Acquire), 2);

    let mut result_set = statement
        .execute_query("SELECT id FROM account WHERE owner = 'alice'")
        .await
        .expect("默认 Connection 必须回退为物理 ResultSet");
    assert!(result_set.next().await.unwrap());
    assert_eq!(result_set.long(ColumnIndex::new(1).unwrap()).unwrap(), 1);
    result_set.close().await.unwrap();
    assert_eq!(filter.after_count.load(Ordering::Acquire), 3);
    assert_eq!(
        *filter.row_counts.lock().expect("row count lock poisoned"),
        vec![None, Some(2), None]
    );
}

#[tokio::test]
async fn task6_standard_connection_contract_standard_connection_default_generic_execute_is_explicitly_unsupported(
) {
    let mut connection = ContractConnection;
    assert_eq!(
        connection.warnings().await,
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_warnings"
        })
    );
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .expect("canonical statement must be created");
    assert!(matches!(
        statement
            .execute("SELECT 1", StatementGeneratedKeys::None)
            .await,
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_execute"
        })
    ));
    let mut prepared = task6_public_standard_prepared_statement("SELECT ?1")
        .await
        .expect("public factory must return canonical prepared statement");
    let mut result_set = statement
        .execute_query("SELECT ?1")
        .await
        .expect("canonical statement 必须返回 owned ResultSet");
    assert!(result_set.next().await.unwrap());
    assert_eq!(
        result_set.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(1)
    );
    prepared
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::Int(1),
        })
        .unwrap();
    let mut prepared_result_set = prepared
        .execute_prepared_query()
        .await
        .expect("canonical prepared ResultSet 必须拥有 cursor");
    assert!(prepared_result_set.next().await.unwrap());
    assert_eq!(
        prepared_result_set
            .value(ColumnIndex::new(1).unwrap())
            .unwrap(),
        Value::Int(1)
    );
    prepared.clear_parameters().unwrap();
    prepared
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::RustValue(Value::Int(1)),
        )
        .unwrap();
    let mut descriptor_result_set = prepared
        .execute_prepared_query()
        .await
        .expect("canonical descriptor prepared ResultSet 必须保留参数值");
    assert!(descriptor_result_set.next().await.unwrap());
    assert_eq!(
        descriptor_result_set
            .value(ColumnIndex::new(1).unwrap())
            .unwrap(),
        Value::Int(1)
    );
    assert_eq!(prepared.warnings().unwrap(), None);
    prepared.clear_warnings().unwrap();
    let results = prepared.execute_prepared().await.unwrap();
    assert!(matches!(
        results.first(),
        Some(ExecutionResult::ResultSet(_))
    ));
    prepared.close().await.unwrap();
    assert!(matches!(
        prepared.warnings(),
        Err(RdbcError::ConnectionDiscarded)
    ));
    assert!(matches!(
        prepared.clear_warnings(),
        Err(RdbcError::ConnectionDiscarded)
    ));
}

#[tokio::test]
async fn task6_standard_connection_contract_explicit_close_and_drop_recycle_exactly_once() {
    let filter = Arc::new(ContextContractFilter::new());
    let pool = contract_pool(filter).await;
    let mut connection = pool
        .get()
        .await
        .expect("connection acquisition must succeed");

    connection.close().await.expect("first close must recycle");
    connection
        .close()
        .await
        .expect("duplicate close must be idempotent");
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 1);
    assert_eq!(pool.state().recycle_count, 1);

    drop(connection);
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 1);
    assert_eq!(pool.state().recycle_count, 1);
}

#[tokio::test]
async fn task6_standard_connection_contract_unwind_drop_recycles_exactly_once() {
    let recycle_count = Arc::new(AtomicUsize::new(0));
    let observed_recycle_count = recycle_count.clone();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(ContractConnection),
        41,
        Duration::ZERO,
        0,
    )
    .await
    .expect("canonical holder must capture the connection defaults");

    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _connection = druid::core::DruidPooledConnection::with_standard_holder(
            holder,
            "contract-data-source".to_owned(),
            None,
            false,
            false,
            Box::new(move |holder, _disposition| {
                assert_eq!(holder.connection_id(), 41);
                observed_recycle_count.fetch_add(1, Ordering::Relaxed);
                true
            }),
        );
        panic!("contract panic");
    }));

    assert!(unwind.is_err());
    assert_eq!(recycle_count.load(Ordering::Relaxed), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn task6_standard_connection_contract_concurrent_creation_never_exceeds_max_open() {
    let factory = Arc::new(GatedFactory {
        entered: AtomicUsize::new(0),
        entered_notify: Notify::new(),
        release: Semaphore::new(0),
    });
    let pool = Arc::new(
        DruidPool::builder()
            .name("capacity-contract")
            .driver_name("contract")
            .factory(factory.clone())
            .max_open(2)
            .max_idle(2)
            .acquire_timeout(Duration::from_secs(2))
            .build()
            .await
            .expect("capacity pool must build"),
    );

    let mut tasks = Vec::new();
    for _ in 0..32 {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async move {
            let connection = pool.get().await.expect("acquisition must succeed");
            drop(connection);
        }));
    }

    tokio::time::timeout(Duration::from_secs(1), async {
        while factory.entered.load(Ordering::Acquire) < 2 {
            factory.entered_notify.notified().await;
        }
    })
    .await
    .expect("two physical creations must enter");
    assert_eq!(factory.entered.load(Ordering::Acquire), 2);
    factory.release.add_permits(2);

    for task in tasks {
        task.await.expect("acquisition task must not panic");
    }

    assert_eq!(factory.entered.load(Ordering::Acquire), 2);
    assert_eq!(pool.state().create_count, 2);
    assert_eq!(pool.state().active_count, 0);
}
