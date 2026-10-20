//! 对应 Java: DruidPooledCallableStatementTest 的 wasNull 过滤器拒绝及委托。
use druid::core::{
    BeforeFilter, DruidError, ExecContext, FilterChain, StandardCallableWasNullFilterChain,
};
use druid::pool::{config::PoolInnerConfig, DruidPool};
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::statement::{
    CallableStatement, ExecutionResult, PreparedStatement, Statement, StatementOptions,
};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

#[derive(Default)]
struct State {
    creates: AtomicUsize,
    reads: AtomicUsize,
    closes: AtomicUsize,
    fail: AtomicBool,
    trace: Mutex<Vec<&'static str>>,
}
struct Factory(Arc<State>);
#[async_trait::async_trait]
impl ConnectionFactory for Factory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        self.0.creates.fetch_add(1, Ordering::AcqRel);
        Ok(Box::new(RawConnection(Arc::clone(&self.0))))
    }
}
struct RawConnection(Arc<State>);
druid_rdbc::impl_standard_wrapper!(RawConnection => Connection);
#[async_trait::async_trait]
impl Connection for RawConnection {
    async fn prepare_call(
        &mut self,
        sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        assert_eq!(sql, "select 1");
        Ok(Box::new(RawCallable {
            state: Arc::clone(&self.0),
            closed: false,
        }))
    }
    fn auto_commit(&self) -> Result<bool, RdbcError> {
        Ok(true)
    }
    fn read_only(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
}
struct RawCallable {
    state: Arc<State>,
    closed: bool,
}
druid_rdbc::impl_standard_wrapper!(RawCallable => CallableStatement);
#[async_trait::async_trait]
impl Statement for RawCallable {
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        if !self.closed {
            self.state.closes.fetch_add(1, Ordering::AcqRel);
            self.closed = true;
        }
        Ok(())
    }
}
#[async_trait::async_trait]
impl PreparedStatement for RawCallable {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok("select 1")
    }
    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        Ok(Vec::new())
    }
}
#[async_trait::async_trait]
impl CallableStatement for RawCallable {
    fn was_null(&self) -> Result<bool, RdbcError> {
        assert!(!self.closed);
        self.state.reads.fetch_add(1, Ordering::AcqRel);
        self.state.trace.lock().unwrap().push("raw");
        if self.state.fail.load(Ordering::Acquire) {
            Err(RdbcError::SqlException(Box::new(SqlException::new(
                73,
                Some("HY111".to_owned()),
                Some("raw wasNull".to_owned()),
            ))))
        } else {
            Ok(false)
        }
    }
}
struct Gate {
    mode: AtomicUsize,
    calls: AtomicUsize,
    events: Mutex<Vec<&'static str>>,
}
fn rejection() -> DruidError {
    DruidError::SqlException(Box::new(SqlException::new(
        41,
        Some("HY123".to_owned()),
        Some("filtered wasNull".to_owned()),
    )))
}
#[async_trait::async_trait]
impl BeforeFilter for Gate {
    fn name(&self) -> &str {
        "callable-was-null-gate"
    }
    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }
    fn standard_callable_was_null(
        &self,
        chain: &mut StandardCallableWasNullFilterChain<'_>,
    ) -> Result<bool, DruidError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        self.events.lock().unwrap().push("before");
        let mode = self.mode.load(Ordering::Acquire);
        if mode == 0 {
            return Err(rejection());
        }
        let value = chain.callable_statement_was_null()?;
        self.events.lock().unwrap().push("after");
        if mode == 3 {
            Err(rejection())
        } else if mode == 2 {
            Ok(!value)
        } else {
            Ok(value)
        }
    }
}
fn pool(state: Arc<State>, gate: Option<Arc<Gate>>) -> DruidPool {
    let mut filters = FilterChain::new();
    if let Some(gate) = gate {
        filters.add_before(gate);
    }
    DruidPool::new_standard(
        "callable-was-null".to_owned(),
        "mock".to_owned(),
        Arc::new(Factory(state)),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(filters)),
    )
}

#[tokio::test]
async fn was_null_filter_rejects_rewrites_and_counts_raw_or_filter_errors_once() {
    let state = Arc::new(State::default());
    let gate = Arc::new(Gate {
        mode: AtomicUsize::new(0),
        calls: AtomicUsize::new(0),
        events: Mutex::new(Vec::new()),
    });
    let pool = pool(Arc::clone(&state), Some(Arc::clone(&gate)));
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .prepare_call("select 1", StatementOptions::default())
        .await
        .unwrap();
    statement.execute_prepared().await.unwrap();
    assert_eq!(pool.stat_value_and_reset().error_count, 0);
    let error = statement.was_null().unwrap_err();
    let sql = error.sql_exception().unwrap();
    assert_eq!(
        (sql.error_code(), sql.sql_state(), sql.message()),
        (41, Some("HY123"), Some("filtered wasNull"))
    );
    assert_eq!(pool.stat_value_and_reset().error_count, 1);
    assert_eq!(state.reads.load(Ordering::Acquire), 0);
    gate.mode.store(1, Ordering::Release);
    assert!(!statement.was_null().unwrap());
    gate.mode.store(2, Ordering::Release);
    assert!(statement.was_null().unwrap());
    assert_eq!(
        *gate.events.lock().unwrap(),
        ["before", "before", "after", "before", "after"]
    );
    assert_eq!(pool.stat_value_and_reset().error_count, 0);
    gate.mode.store(3, Ordering::Release);
    assert_eq!(
        statement
            .was_null()
            .unwrap_err()
            .sql_exception()
            .unwrap()
            .error_code(),
        41
    );
    assert_eq!(pool.stat_value_and_reset().error_count, 1);
    state.fail.store(true, Ordering::Release);
    gate.mode.store(1, Ordering::Release);
    let error = statement.was_null().unwrap_err();
    let sql = error.sql_exception().unwrap();
    assert_eq!(
        (sql.error_code(), sql.sql_state(), sql.message()),
        (73, Some("HY111"), Some("raw wasNull"))
    );
    assert_eq!(pool.stat_value_and_reset().error_count, 1);
    assert_eq!(state.reads.load(Ordering::Acquire), 4);
    statement.close().await.unwrap();
    let calls = gate.calls.load(Ordering::Acquire);
    assert!(statement.was_null().is_err());
    assert_eq!(gate.calls.load(Ordering::Acquire), calls);
    assert_eq!(state.reads.load(Ordering::Acquire), 4);
    assert_eq!(state.closes.load(Ordering::Acquire), 1);
    connection.close().await.unwrap();
    let mut next = pool.get().await.unwrap();
    assert_eq!(state.creates.load(Ordering::Acquire), 1);
    next.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn no_filter_keeps_the_original_driver_boolean() {
    let state = Arc::new(State::default());
    let pool = pool(Arc::clone(&state), None);
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .prepare_call("select 1", StatementOptions::default())
        .await
        .unwrap();
    assert!(!statement.was_null().unwrap());
    assert_eq!(state.reads.load(Ordering::Acquire), 1);
    assert_eq!(pool.stat_value_and_reset().error_count, 0);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

struct OrderedFilter {
    state: Arc<State>,
    enter: &'static str,
    leave: &'static str,
}
#[async_trait::async_trait]
impl BeforeFilter for OrderedFilter {
    fn name(&self) -> &str {
        self.enter
    }
    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }
    fn standard_callable_was_null(
        &self,
        chain: &mut StandardCallableWasNullFilterChain<'_>,
    ) -> Result<bool, DruidError> {
        self.state.trace.lock().unwrap().push(self.enter);
        let result = chain.callable_statement_was_null();
        self.state.trace.lock().unwrap().push(self.leave);
        result
    }
}

#[tokio::test]
async fn multiple_filters_keep_registration_and_unwind_order() {
    let state = Arc::new(State::default());
    let mut filters = FilterChain::new();
    for (enter, leave) in [("a+", "a-"), ("b+", "b-")] {
        filters.add_before(Arc::new(OrderedFilter {
            state: Arc::clone(&state),
            enter,
            leave,
        }));
    }
    let pool = DruidPool::new_standard(
        "was-null-order".to_owned(),
        "mock".to_owned(),
        Arc::new(Factory(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(filters)),
    );
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .prepare_call("select 1", StatementOptions::default())
        .await
        .unwrap();
    assert!(!statement.was_null().unwrap());
    assert_eq!(
        *state.trace.lock().unwrap(),
        ["a+", "b+", "raw", "b-", "a-"]
    );
    assert_eq!(state.reads.load(Ordering::Acquire), 1);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}
