//! Connection、Statement、PreparedStatement 与 ResultSet 的 SQLWarning 差分契约。
//!
//! 对应 Java：
//! `DruidPooledConnection#getWarnings/clearWarnings`、
//! `DruidPooledStatement#getWarnings/clearWarnings` 和
//! `DruidPooledResultSet#getWarnings/clearWarnings`。

use druid::core::{
    BeforeFilter, ConnectionWarningFilterChain, DruidError, DruidPooledConnection, ExceptionSorter,
    ExceptionSorterProperties, ExecContext, ExecResult, FilterChain, PhysicalConnection,
    PhysicalConnectionFactory, ResultSetFilter, ResultSetFilterChain, Row, SqlException,
    SqlWarning, StatementWarningFilterChain, Value,
};
use druid::toasty::ToastyConnectionFactory;
use std::sync::{Arc, Mutex};

fn fatal_warning_error() -> DruidError {
    DruidError::SqlException(Box::new(
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

struct WarningPhysicalConnection {
    warning: Option<SqlWarning>,
    fail_get: bool,
    fail_clear: bool,
    discarded: bool,
    closed: bool,
    log: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl PhysicalConnection for WarningPhysicalConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, DruidError> {
        Ok(ExecResult::default())
    }

    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(Vec::new())
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed
    }

    async fn warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        self.log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push("physical-connection-get".to_string());
        if self.fail_get {
            Err(fatal_warning_error())
        } else {
            Ok(self.warning.clone())
        }
    }

    async fn clear_warnings(&mut self) -> Result<(), DruidError> {
        self.log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push("physical-connection-clear".to_string());
        if self.fail_clear {
            Err(fatal_warning_error())
        } else {
            self.warning = None;
            Ok(())
        }
    }

    fn mark_discarded(&mut self) {
        self.discarded = true;
    }

    fn is_discarded(&self) -> bool {
        self.discarded
    }
}

struct WarningAroundFilter {
    name: &'static str,
    log: Arc<Mutex<Vec<String>>>,
    fail_statement_get: bool,
    fail_statement_clear: bool,
}

impl WarningAroundFilter {
    fn record(&self, operation: &str, phase: &str) {
        self.log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{}-{operation}-{phase}", self.name));
    }

    fn replacement(&self) -> SqlWarning {
        SqlWarning::new(self.name, Some("01000".to_string()), 7)
    }
}

#[async_trait::async_trait]
impl BeforeFilter for WarningAroundFilter {
    fn name(&self) -> &str {
        self.name
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }

    async fn connection_get_warnings(
        &self,
        chain: &mut ConnectionWarningFilterChain<'_>,
    ) -> Result<Option<SqlWarning>, DruidError> {
        self.record("connection-get", "before");
        let _ = chain.connection_get_warnings().await?;
        self.record("connection-get", "after");
        Ok(Some(self.replacement()))
    }

    async fn connection_clear_warnings(
        &self,
        chain: &mut ConnectionWarningFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.record("connection-clear", "before");
        chain.connection_clear_warnings().await?;
        self.record("connection-clear", "after");
        Ok(())
    }

    async fn statement_get_warnings(
        &self,
        chain: &mut StatementWarningFilterChain<'_>,
    ) -> Result<Option<SqlWarning>, DruidError> {
        self.record("statement-get", "before");
        if self.fail_statement_get {
            return Err(fatal_warning_error());
        }
        let _ = chain.statement_get_warnings().await?;
        self.record("statement-get", "after");
        Ok(Some(self.replacement()))
    }

    async fn statement_clear_warnings(
        &self,
        chain: &mut StatementWarningFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.record("statement-clear", "before");
        if self.fail_statement_clear {
            return Err(fatal_warning_error());
        }
        chain.statement_clear_warnings().await?;
        self.record("statement-clear", "after");
        Ok(())
    }
}

impl ResultSetFilter for WarningAroundFilter {
    fn result_set_get_warnings(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<Option<SqlWarning>, DruidError> {
        self.record("result-set-get", "before");
        let _ = chain.result_set_get_warnings()?;
        self.record("result-set-get", "after");
        Ok(Some(self.replacement()))
    }

    fn result_set_clear_warnings(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.record("result-set-clear", "before");
        chain.result_set_clear_warnings()?;
        self.record("result-set-clear", "after");
        Ok(())
    }
}

fn warning_connection(
    warning: Option<SqlWarning>,
    fail_get: bool,
    fail_clear: bool,
    filter_chain: Option<Arc<FilterChain>>,
    log: Arc<Mutex<Vec<String>>>,
) -> DruidPooledConnection {
    DruidPooledConnection::with_context(
        Box::new(WarningPhysicalConnection {
            warning,
            fail_get,
            fail_clear,
            discarded: false,
            closed: false,
            log,
        }),
        90,
        "warning-probe".to_string(),
        filter_chain,
        Box::new(|_, _| {}),
    )
    .with_exception_sorter(Arc::new(AlwaysFatalSorter))
}

#[tokio::test]
async fn connection_warning_chain_preserves_order_rewrite_and_clear() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let outer = Arc::new(WarningAroundFilter {
        name: "outer",
        log: Arc::clone(&log),
        fail_statement_get: false,
        fail_statement_clear: false,
    });
    let inner = Arc::new(WarningAroundFilter {
        name: "inner",
        log: Arc::clone(&log),
        fail_statement_get: false,
        fail_statement_clear: false,
    });
    let mut chain = FilterChain::new();
    chain.add_before(outer);
    chain.add_before(inner);
    let mut connection = warning_connection(
        Some(SqlWarning::new("physical", Some("01001".to_string()), 8)),
        false,
        false,
        Some(Arc::new(chain)),
        Arc::clone(&log),
    );

    let warning = connection
        .warnings()
        .await
        .expect("warning getter 必须成功")
        .expect("外层 Filter 必须返回警告");
    assert_eq!(warning.message(), "outer");
    connection
        .clear_warnings()
        .await
        .expect("warning clear 必须成功");

    assert_eq!(
        *log.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        vec![
            "outer-connection-get-before",
            "inner-connection-get-before",
            "physical-connection-get",
            "inner-connection-get-after",
            "outer-connection-get-after",
            "outer-connection-clear-before",
            "inner-connection-clear-before",
            "physical-connection-clear",
            "inner-connection-clear-after",
            "outer-connection-clear-after",
        ]
    );
}

#[tokio::test]
async fn connection_get_warning_error_skips_sorter_but_clear_uses_sorter() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut connection = warning_connection(None, true, true, None, log);

    assert!(matches!(
        connection.warnings().await,
        Err(DruidError::SqlException(_))
    ));
    assert!(
        !connection.is_discarded(),
        "Java getWarnings 不调用 handleException，不能提前 discard"
    );

    assert!(matches!(
        connection.clear_warnings().await,
        Err(DruidError::SqlException(_))
    ));
    assert!(
        connection.is_discarded(),
        "Java clearWarnings 调用 handleException，fatal 错误必须 discard"
    );
}

async fn sqlite_connection(filter_chain: Option<Arc<FilterChain>>) -> DruidPooledConnection {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 SQLite 物理连接");
    DruidPooledConnection::with_context(
        physical,
        91,
        "sqlite-warning".to_string(),
        filter_chain,
        Box::new(|_, _| {}),
    )
}

#[tokio::test]
async fn real_sqlite_warning_contract_covers_all_four_rdbc_objects() {
    let mut connection = sqlite_connection(None).await;
    assert_eq!(connection.warnings().await.unwrap(), None);
    connection.clear_warnings().await.unwrap();

    let mut statement = connection.create_statement().await.unwrap();
    assert_eq!(statement.warnings(&mut connection).await.unwrap(), None);
    statement.clear_warnings(&mut connection).await.unwrap();

    let mut prepared = connection.prepare_statement("SELECT ?").await.unwrap();
    assert_eq!(prepared.warnings(&mut connection).await.unwrap(), None);
    prepared.clear_warnings(&mut connection).await.unwrap();

    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1")
        .await
        .unwrap();
    assert_eq!(result_set.warnings(&mut connection).unwrap(), None);
    result_set.clear_warnings(&mut connection).unwrap();
}

#[tokio::test]
async fn statement_prepared_and_result_set_use_java_filter_order() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let outer = Arc::new(WarningAroundFilter {
        name: "outer",
        log: Arc::clone(&log),
        fail_statement_get: false,
        fail_statement_clear: false,
    });
    let inner = Arc::new(WarningAroundFilter {
        name: "inner",
        log: Arc::clone(&log),
        fail_statement_get: false,
        fail_statement_clear: false,
    });
    let mut chain = FilterChain::new();
    chain.add_before(outer.clone());
    chain.add_before(inner.clone());
    chain.add_result_set(outer);
    chain.add_result_set(inner);
    let mut connection = sqlite_connection(Some(Arc::new(chain))).await;

    let mut statement = connection.create_statement().await.unwrap();
    assert_eq!(
        statement
            .warnings(&mut connection)
            .await
            .unwrap()
            .unwrap()
            .message(),
        "outer"
    );
    statement.clear_warnings(&mut connection).await.unwrap();

    let mut prepared = connection.prepare_statement("SELECT ?").await.unwrap();
    assert_eq!(
        prepared
            .warnings(&mut connection)
            .await
            .unwrap()
            .unwrap()
            .message(),
        "outer"
    );
    prepared.clear_warnings(&mut connection).await.unwrap();

    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1")
        .await
        .unwrap();
    assert_eq!(
        result_set
            .warnings(&mut connection)
            .unwrap()
            .unwrap()
            .message(),
        "outer"
    );
    result_set.clear_warnings(&mut connection).unwrap();

    let entries = log
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    for operation in [
        "statement-get",
        "statement-clear",
        "result-set-get",
        "result-set-clear",
    ] {
        let relevant = entries
            .iter()
            .filter(|entry| entry.contains(operation))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            relevant,
            vec![
                format!("outer-{operation}-before"),
                format!("inner-{operation}-before"),
                format!("inner-{operation}-after"),
                format!("outer-{operation}-after"),
                format!("outer-{operation}-before"),
                format!("inner-{operation}-before"),
                format!("inner-{operation}-after"),
                format!("outer-{operation}-after"),
            ]
            .into_iter()
            .take(if operation.starts_with("statement") {
                8
            } else {
                4
            })
            .collect::<Vec<_>>()
        );
    }
}

#[tokio::test]
async fn statement_warning_filter_error_enters_fatal_sorter() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let filter = Arc::new(WarningAroundFilter {
        name: "fatal",
        log,
        fail_statement_get: true,
        fail_statement_clear: false,
    });
    let mut chain = FilterChain::new();
    chain.add_before(filter);
    let mut connection = sqlite_connection(Some(Arc::new(chain)))
        .await
        .with_exception_sorter(Arc::new(AlwaysFatalSorter));
    let mut statement = connection.create_statement().await.unwrap();

    assert!(matches!(
        statement.warnings(&mut connection).await,
        Err(DruidError::SqlException(_))
    ));
    assert!(connection.is_discarded());
    assert_eq!(statement.exception_count(), 1);
}

#[tokio::test]
async fn prepared_warning_filter_errors_enter_fatal_sorter_for_get_and_clear() {
    for (fail_get, fail_clear, operation) in
        [(true, false, "getWarnings"), (false, true, "clearWarnings")]
    {
        let filter = Arc::new(WarningAroundFilter {
            name: "fatal-prepared",
            log: Arc::new(Mutex::new(Vec::new())),
            fail_statement_get: fail_get,
            fail_statement_clear: fail_clear,
        });
        let mut chain = FilterChain::new();
        chain.add_before(filter);
        let mut connection = sqlite_connection(Some(Arc::new(chain)))
            .await
            .with_exception_sorter(Arc::new(AlwaysFatalSorter));
        let mut prepared = connection.prepare_statement("SELECT ?").await.unwrap();

        let result = if fail_get {
            prepared.warnings(&mut connection).await.map(|_| ())
        } else {
            prepared.clear_warnings(&mut connection).await
        };
        assert!(
            matches!(result, Err(DruidError::SqlException(_))),
            "{operation} 必须保留结构化 SQL 异常"
        );
        assert!(
            connection.is_discarded(),
            "{operation} fatal 错误必须丢弃物理连接"
        );
    }
}
