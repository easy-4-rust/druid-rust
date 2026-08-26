//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Canonical Connection/Statement/PreparedStatement/ResultSet SQLWarning semantics.

use druid::core::{
    AfterFilter, BeforeFilter, Connection, ConnectionEvent, ConnectionFactory,
    ConnectionRecycleDisposition, DruidConnectionHolder, DruidError, DruidPooledConnection,
    ExceptionSorter, ExceptionSorterProperties, ExecContext, ExecResult, FilterChain, RdbcError,
    SqlException, SqlWarning,
};
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, ResultSetMetaData, ResultSetMetaDataSnapshot,
};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{PreparedStatement, Statement, StatementOptions};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn warning_chain() -> SqlWarning {
    let mut outer = SqlWarning::new("outer", Some("01000".to_owned()), 7);
    outer.set_next_warning(SqlWarning::new("inner", Some("01001".to_owned()), 8));
    outer
}

fn fatal_warning_error() -> RdbcError {
    RdbcError::SqlException(Box::new(
        SqlException::driver(17_002, "Io exception: Connection reset")
            .with_sql_state("08006")
            .with_class_name("java.sql.SQLRecoverableException"),
    ))
}

struct AlwaysFatalSorter;
impl ExceptionSorter for AlwaysFatalSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        true
    }
    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

struct WarningEventFilter {
    name: &'static str,
    log: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl BeforeFilter for WarningEventFilter {
    fn name(&self) -> &str {
        self.name
    }
    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }
    async fn on_connection_event(&self, event: &ConnectionEvent) -> Result<(), DruidError> {
        if matches!(event, ConnectionEvent::ClearWarnings) {
            self.log
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!("{}-before", self.name));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for WarningEventFilter {
    fn name(&self) -> &str {
        self.name
    }
    async fn after(
        &self,
        _context: &ExecContext<'_>,
        _result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        Ok(())
    }
    async fn after_connection_event(
        &self,
        event: &ConnectionEvent,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        if matches!(event, ConnectionEvent::ClearWarnings) {
            self.log
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!("{}-after", self.name));
        }
        Ok(())
    }
}

#[derive(Clone)]
struct WarningState {
    warning: Arc<Mutex<Option<SqlWarning>>>,
    fail_get: Arc<AtomicBool>,
    fail_clear: Arc<AtomicBool>,
}

impl WarningState {
    fn new(fail_get: bool, fail_clear: bool) -> Self {
        Self {
            warning: Arc::new(Mutex::new(Some(warning_chain()))),
            fail_get: Arc::new(AtomicBool::new(fail_get)),
            fail_clear: Arc::new(AtomicBool::new(fail_clear)),
        }
    }

    fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        if self.fail_get.load(Ordering::Acquire) {
            return Err(fatal_warning_error());
        }
        Ok(self
            .warning
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    fn clear(&self) -> Result<(), RdbcError> {
        if self.fail_clear.load(Ordering::Acquire) {
            return Err(fatal_warning_error());
        }
        *self
            .warning
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        Ok(())
    }
}

struct WarningResultSet {
    state: WarningState,
    closed: bool,
}
druid_rdbc::impl_standard_wrapper!(WarningResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for WarningResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        self.state.warnings()
    }
    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.state.clear()
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

struct WarningStatement {
    state: WarningState,
    closed: bool,
}
druid_rdbc::impl_standard_wrapper!(WarningStatement => Statement);

#[async_trait::async_trait]
impl Statement for WarningStatement {
    fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        self.state.warnings()
    }
    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.state.clear()
    }
    async fn execute_query(&mut self, _sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        Ok(Box::new(WarningResultSet {
            state: self.state.clone(),
            closed: false,
        }))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

struct WarningPrepared {
    state: WarningState,
    closed: bool,
}
druid_rdbc::impl_standard_wrapper!(WarningPrepared => PreparedStatement);

#[async_trait::async_trait]
impl Statement for WarningPrepared {
    fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        self.state.warnings()
    }
    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.state.clear()
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
impl PreparedStatement for WarningPrepared {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok("SELECT ?")
    }
}

struct WarningConnection {
    connection_state: WarningState,
    statement_state: WarningState,
    closed: bool,
}
druid_rdbc::impl_standard_wrapper!(WarningConnection => Connection);

#[async_trait::async_trait]
impl Connection for WarningConnection {
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        Ok(Box::new(WarningStatement {
            state: self.statement_state.clone(),
            closed: false,
        }))
    }
    async fn prepare_statement(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        Ok(Box::new(WarningPrepared {
            state: self.statement_state.clone(),
            closed: false,
        }))
    }
    async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        self.connection_state.warnings()
    }
    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.connection_state.clear()
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

async fn task6_warning_connection(
    connection_fail_get: bool,
    connection_fail_clear: bool,
    statement_fail_get: bool,
    statement_fail_clear: bool,
) -> (DruidPooledConnection, Arc<AtomicBool>) {
    let connection_state = WarningState::new(false, false);
    let connection_state_control = connection_state.clone();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(WarningConnection {
            connection_state,
            statement_state: WarningState::new(statement_fail_get, statement_fail_clear),
            closed: false,
        }),
        91,
        Duration::ZERO,
        0,
    )
    .await
    .unwrap();
    connection_state_control
        .fail_get
        .store(connection_fail_get, Ordering::Release);
    connection_state_control
        .fail_clear
        .store(connection_fail_clear, Ordering::Release);
    let discarded = Arc::new(AtomicBool::new(false));
    let observer = Arc::clone(&discarded);
    let connection = DruidPooledConnection::with_standard_holder_and_disposition_observer(
        holder,
        "warning".to_owned(),
        None,
        false,
        false,
        Box::new(move |disposition| {
            observer.store(
                matches!(disposition, ConnectionRecycleDisposition::Discard { .. }),
                Ordering::Release,
            );
        }),
        Box::new(move |_, _| false),
    )
    .with_exception_sorter(Arc::new(AlwaysFatalSorter));
    (connection, discarded)
}

async fn sqlite_connection() -> DruidPooledConnection {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let raw = factory.create().await.unwrap();
    let holder = DruidConnectionHolder::with_standard_connection(raw, 92, Duration::ZERO, 0)
        .await
        .unwrap();
    DruidPooledConnection::with_standard_holder(
        holder,
        "sqlite-warning".to_owned(),
        None,
        false,
        false,
        Box::new(|_, _| true),
    )
}

async fn task6_warning_connection_with_filter_log(
) -> (DruidPooledConnection, Arc<Mutex<Vec<String>>>) {
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(WarningConnection {
            connection_state: WarningState::new(false, false),
            statement_state: WarningState::new(false, false),
            closed: false,
        }),
        93,
        Duration::ZERO,
        0,
    )
    .await
    .unwrap();
    let log = Arc::new(Mutex::new(Vec::new()));
    let outer = Arc::new(WarningEventFilter {
        name: "outer",
        log: Arc::clone(&log),
    });
    let inner = Arc::new(WarningEventFilter {
        name: "inner",
        log: Arc::clone(&log),
    });
    let mut chain = FilterChain::new();
    chain.add_before(outer.clone() as Arc<dyn BeforeFilter>);
    chain.add_before(inner.clone() as Arc<dyn BeforeFilter>);
    chain.add_after(outer as Arc<dyn AfterFilter>);
    chain.add_after(inner as Arc<dyn AfterFilter>);
    let connection = DruidPooledConnection::with_standard_holder(
        holder,
        "warning-filter-order".to_owned(),
        Some(Arc::new(chain)),
        false,
        false,
        Box::new(|_, _| true),
    );
    (connection, log)
}

#[tokio::test]
async fn task6_sql_warning_semantics_test_connection_warning_chain_preserves_order_rewrite_and_clear(
) {
    let (mut connection, log) = task6_warning_connection_with_filter_log().await;
    let warning = Connection::warnings(&mut connection)
        .await
        .unwrap()
        .unwrap();
    let cloned = warning.clone();
    assert_eq!(warning.message(), "outer");
    assert_eq!(warning.sql_state(), Some("01000"));
    assert_eq!(warning.error_code(), 7);
    assert_eq!(warning.next_warning().unwrap().message(), "inner");
    assert_eq!(cloned, warning);
    Connection::clear_warnings(&mut connection).await.unwrap();
    assert!(Connection::warnings(&mut connection)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        *log.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        ["outer-before", "inner-before", "inner-after", "outer-after"]
    );
}

#[tokio::test]
async fn task6_sql_warning_semantics_test_connection_get_warning_error_skips_sorter_but_clear_uses_sorter(
) {
    let (mut get_connection, get_discarded) =
        task6_warning_connection(true, false, false, false).await;
    assert!(matches!(
        get_connection.warnings().await,
        Err(RdbcError::SqlException(_))
    ));
    assert!(!get_discarded.load(Ordering::Acquire));
    assert!(!get_connection.is_recycled());
    assert!(!get_connection.is_closed().unwrap());
    let (mut clear_connection, clear_discarded) =
        task6_warning_connection(false, true, false, false).await;
    assert!(!clear_discarded.load(Ordering::Acquire));
    assert!(matches!(
        clear_connection.clear_warnings().await,
        Err(RdbcError::SqlException(_))
    ));
    assert!(clear_discarded.load(Ordering::Acquire));
    assert!(!clear_connection.is_recycled());
    assert!(clear_connection.is_closed().unwrap());
    clear_connection.close().await.unwrap();
    assert!(clear_connection.is_recycled());
}

#[tokio::test]
async fn task6_sql_warning_semantics_test_real_sqlite_warning_contract_covers_all_four_rdbc_objects(
) {
    let mut connection = sqlite_connection().await;
    assert!(connection.warnings().await.unwrap().is_none());
    connection.clear_warnings().await.unwrap();
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .unwrap();
    assert!(statement.warnings().unwrap().is_none());
    statement.clear_warnings().unwrap();
    let mut prepared =
        Connection::prepare_statement(&mut connection, "SELECT ?", StatementOptions::default())
            .await
            .unwrap();
    assert!(prepared.warnings().unwrap().is_none());
    prepared.clear_warnings().unwrap();
    let mut result_set = statement.execute_query("SELECT 1").await.unwrap();
    assert!(result_set.warnings().unwrap().is_none());
    result_set.clear_warnings().unwrap();
    result_set.close().await.unwrap();
    prepared.close().await.unwrap();
    statement.close().await.unwrap();
}

#[tokio::test]
async fn task6_sql_warning_semantics_test_statement_prepared_and_result_set_use_java_filter_order()
{
    let (mut connection, _) = task6_warning_connection(false, false, false, false).await;
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .unwrap();
    let statement_warning = statement.warnings().unwrap().unwrap();
    assert_eq!(statement_warning.message(), "outer");
    assert_eq!(statement_warning.next_warning().unwrap().message(), "inner");
    statement.clear_warnings().unwrap();
    assert!(statement.warnings().unwrap().is_none());
    let mut prepared =
        Connection::prepare_statement(&mut connection, "SELECT ?", StatementOptions::default())
            .await
            .unwrap();
    assert!(prepared.warnings().unwrap().is_none());
    prepared.clear_warnings().unwrap();
    let mut result_set = statement.execute_query("SELECT 1").await.unwrap();
    assert!(result_set.warnings().unwrap().is_none());
    result_set.clear_warnings().unwrap();
    assert!(result_set.warnings().unwrap().is_none());
}

#[tokio::test]
async fn task6_sql_warning_semantics_test_statement_warning_filter_error_enters_fatal_sorter() {
    let (mut connection, discarded) = task6_warning_connection(false, false, true, false).await;
    let statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .unwrap();
    assert!(matches!(
        statement.warnings(),
        Err(RdbcError::SqlException(_))
    ));
    assert!(discarded.load(Ordering::Acquire));
}

#[tokio::test]
async fn task6_sql_warning_semantics_test_prepared_warning_filter_errors_enter_fatal_sorter_for_get_and_clear(
) {
    for (fail_get, fail_clear, operation) in
        [(true, false, "getWarnings"), (false, true, "clearWarnings")]
    {
        let (mut connection, discarded) =
            task6_warning_connection(false, false, fail_get, fail_clear).await;
        let mut prepared =
            Connection::prepare_statement(&mut connection, "SELECT ?", StatementOptions::default())
                .await
                .unwrap();
        let result = if fail_get {
            prepared.warnings().map(|_| ())
        } else {
            prepared.clear_warnings()
        };
        assert!(
            matches!(result, Err(RdbcError::SqlException(_))),
            "{operation}"
        );
        assert!(
            discarded.load(Ordering::Acquire),
            "{operation} must discard"
        );
    }
}
