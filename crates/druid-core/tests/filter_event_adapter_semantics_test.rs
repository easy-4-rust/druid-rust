//! Java `FilterEventAdapter` 事件模板与真实 Toasty SQLite 验证。

extern crate druid_core as druid;
use druid::core::{
    AfterFilter, BatchExecContext, BatchExecKind, BeforeFilter, ConnectionEvent, DruidError,
    DruidPooledConnection, ExecContext, ExecOperation, ExecResult, ExtendedFilter, FilterChain,
    FilterEventAdapter, FilterEventListener, PhysicalConnectionFactory, ResultSetFilter,
    ResultSetFilterContext, StatementEvent, WrapperExt,
};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::any::type_name;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct RecordingListener {
    label: &'static str,
    events: Arc<Mutex<Vec<String>>>,
}

impl RecordingListener {
    fn record(&self, event: impl Into<String>) {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{}:{}", self.label, event.into()));
    }
}

#[async_trait::async_trait]
impl FilterEventListener for RecordingListener {
    async fn statement_create_after(&self) -> Result<(), DruidError> {
        self.record("create_after");
        Ok(())
    }

    async fn statement_prepare_after(&self, sql: &str) -> Result<(), DruidError> {
        self.record(format!("prepare_after:{sql}"));
        Ok(())
    }

    async fn statement_prepare_call_after(&self, sql: &str) -> Result<(), DruidError> {
        self.record(format!("prepare_call_after:{sql}"));
        Ok(())
    }

    async fn statement_execute_before(&self, context: &ExecContext<'_>) -> Result<(), DruidError> {
        self.record(format!("execute_before:{}", context.sql));
        Ok(())
    }

    async fn statement_execute_after(
        &self,
        context: &ExecContext<'_>,
        first_result: bool,
    ) -> Result<(), DruidError> {
        self.record(format!("execute_after:{}:{first_result}", context.sql));
        Ok(())
    }

    async fn statement_execute_query_before(
        &self,
        context: &ExecContext<'_>,
    ) -> Result<(), DruidError> {
        self.record(format!("query_before:{}", context.sql));
        Ok(())
    }

    async fn statement_execute_query_after(
        &self,
        context: &ExecContext<'_>,
    ) -> Result<(), DruidError> {
        self.record(format!("query_after:{}", context.sql));
        Ok(())
    }

    async fn statement_execute_update_before(
        &self,
        context: &ExecContext<'_>,
    ) -> Result<(), DruidError> {
        self.record(format!("update_before:{}", context.sql));
        Ok(())
    }

    async fn statement_execute_update_after(
        &self,
        context: &ExecContext<'_>,
        update_count: i32,
    ) -> Result<(), DruidError> {
        self.record(format!("update_after:{}:{update_count}", context.sql));
        Ok(())
    }

    async fn statement_execute_batch_before(
        &self,
        context: &druid::core::BatchExecContext<'_>,
    ) -> Result<(), DruidError> {
        self.record(format!("batch_before:{}", context.sql));
        Ok(())
    }

    async fn statement_execute_batch_after(
        &self,
        context: &druid::core::BatchExecContext<'_>,
        update_counts: &[i32],
    ) -> Result<(), DruidError> {
        self.record(format!("batch_after:{}:{update_counts:?}", context.sql));
        Ok(())
    }

    async fn statement_execute_error_after(
        &self,
        sql: &str,
        error: &DruidError,
    ) -> Result<(), DruidError> {
        self.record(format!("error_after:{sql}:{error}"));
        Ok(())
    }

    fn result_set_open_after(&self, _context: &ResultSetFilterContext) -> Result<(), DruidError> {
        self.record("result_set_open_after");
        Ok(())
    }
}

fn events(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    log.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

#[tokio::test]
async fn default_anonymous_adapter_preserves_every_java_no_op_template() {
    let mut adapter = FilterEventAdapter::default();
    assert_eq!(BeforeFilter::name(&adapter), "FilterEventAdapter");
    assert_eq!(AfterFilter::name(&adapter), "FilterEventAdapter");
    assert_eq!(adapter.listener(), &());

    // SOURCE_PARITY / V0_STATIC + V1_RUST_LOCAL：
    // Java 13 个 protected/public 模板默认都为空操作。匿名子类不覆盖任何方法
    // 时，所有入口必须放行且不得伪造事件结果。
    BeforeFilter::init(&adapter).await.unwrap();
    ExtendedFilter::config_from_properties(&mut adapter, &HashMap::new())
        .await
        .unwrap();
    BeforeFilter::destroy(&adapter).await.unwrap();
    assert!(ExtendedFilter::is_wrapper_for(
        &adapter,
        type_name::<FilterEventAdapter>()
    ));
    assert!(adapter.is_wrapper_for_type::<FilterEventAdapter>());

    let start = Instant::now();
    let mut context = ExecContext {
        connection_id: 7,
        statement_id: Some(20_001),
        sql: "SELECT 1".to_owned(),
        params: &[],
        prepared_parameters: None,
        data_source: "default-event-adapter",
        start,
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Execute,
    };
    let query_result = Ok(ExecResult {
        rows_affected: 0,
        last_insert_id: None,
        row_count: Some(1),
    });
    BeforeFilter::before(&adapter, &mut context).await.unwrap();
    AfterFilter::after(&adapter, &context, &query_result, Duration::ZERO)
        .await
        .unwrap();

    context.operation = ExecOperation::Query;
    BeforeFilter::before(&adapter, &mut context).await.unwrap();
    AfterFilter::after(&adapter, &context, &query_result, Duration::ZERO)
        .await
        .unwrap();

    context.operation = ExecOperation::Update;
    let update_result = Ok(ExecResult {
        rows_affected: 3,
        last_insert_id: None,
        row_count: None,
    });
    BeforeFilter::before(&adapter, &mut context).await.unwrap();
    AfterFilter::after(&adapter, &context, &update_result, Duration::ZERO)
        .await
        .unwrap();

    context.operation = ExecOperation::Batch;
    BeforeFilter::before(&adapter, &mut context).await.unwrap();
    let execution_error = Err(DruidError::DriverError("driver".to_string()));
    AfterFilter::after(&adapter, &context, &execution_error, Duration::ZERO)
        .await
        .unwrap();

    let batch_statements = vec!["UPDATE a".to_string(), "UPDATE b".to_string()];
    let mut batch_context = BatchExecContext {
        connection_id: 7,
        statement_id: Some(20_002),
        sql: "UPDATE a\n;\nUPDATE b",
        statements: &batch_statements,
        parameter_sets: &[],
        prepared_parameter_sets: None,
        kind: BatchExecKind::Statement,
        data_source: "default-event-adapter",
        start,
        fingerprint: None,
        in_transaction: false,
    };
    BeforeFilter::before_batch(&adapter, &mut batch_context)
        .await
        .unwrap();
    AfterFilter::after_batch(&adapter, &batch_context, &Ok(vec![1, 2]), Duration::ZERO)
        .await
        .unwrap();
    AfterFilter::after_batch(
        &adapter,
        &batch_context,
        &Err(DruidError::DriverError("batch".to_string())),
        Duration::ZERO,
    )
    .await
    .unwrap();

    BeforeFilter::on_connection_event(&adapter, &ConnectionEvent::Connect)
        .await
        .unwrap();
    BeforeFilter::on_connection_event(&adapter, &ConnectionEvent::Close)
        .await
        .unwrap();
    AfterFilter::after_connection_event(&adapter, &ConnectionEvent::Connect, Duration::ZERO)
        .await
        .unwrap();
    AfterFilter::after_connection_event(&adapter, &ConnectionEvent::Close, Duration::ZERO)
        .await
        .unwrap();

    for event in [
        StatementEvent::CreateStatement,
        StatementEvent::PrepareStatement("SELECT ?1".to_string()),
        StatementEvent::PrepareCall("CALL P(?)".to_string()),
        StatementEvent::Execute("SELECT 1".to_string()),
        StatementEvent::ExecuteQuery("SELECT 1".to_string()),
        StatementEvent::ExecuteUpdate("UPDATE t".to_string()),
        StatementEvent::Close,
        StatementEvent::ExecuteBatch,
    ] {
        BeforeFilter::on_statement_event(&adapter, &event)
            .await
            .unwrap();
    }

    ResultSetFilter::result_set_open_after(&adapter, &ResultSetFilterContext::new()).unwrap();

    // RUST_OBLIGATION / V1_RUST_LOCAL：
    // Rust 物理 SPI 的 u64 更新计数若超出 RDBC int，必须成为可分类错误，而不是
    // 截断；默认 error-after 仍放行，原范围错误保持主错误。
    context.operation = ExecOperation::Update;
    let overflow = Ok(ExecResult {
        rows_affected: u64::MAX,
        last_insert_id: None,
        row_count: None,
    });
    assert!(matches!(
        AfterFilter::after(&adapter, &context, &overflow, Duration::ZERO).await,
        Err(DruidError::InvalidArgument(message))
            if message == "update count exceeds RDBC int range: 18446744073709551615"
    ));
}

#[tokio::test]
async fn statement_creation_events_unwind_in_java_filter_order() {
    let event_log = Arc::new(Mutex::new(Vec::new()));
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(FilterEventAdapter::with_listener(
        RecordingListener {
            label: "outer",
            events: Arc::clone(&event_log),
        },
    )));
    chain.add_filter(Arc::new(FilterEventAdapter::with_listener(
        RecordingListener {
            label: "inner",
            events: Arc::clone(&event_log),
        },
    )));

    // SOURCE_PARITY / V0_STATIC + V1_RUST_LOCAL：
    // Java 三组 connection_create/prepare 重载均先进入下游链，成功后才执行
    // protected after；两个 Filter 因而按 inner -> outer 逆序展开。
    chain
        .after_statement_event(&StatementEvent::CreateStatement)
        .await
        .unwrap();
    chain
        .after_statement_event(&StatementEvent::PrepareStatement("SELECT ?1".to_string()))
        .await
        .unwrap();
    chain
        .after_statement_event(&StatementEvent::PrepareCall("CALL P(?)".to_string()))
        .await
        .unwrap();

    assert_eq!(
        events(&event_log),
        [
            "inner:create_after",
            "outer:create_after",
            "inner:prepare_after:SELECT ?1",
            "outer:prepare_after:SELECT ?1",
            "inner:prepare_call_after:CALL P(?)",
            "outer:prepare_call_after:CALL P(?)",
        ]
    );
}

struct FailingSuccessListener {
    events: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl FilterEventListener for FailingSuccessListener {
    async fn statement_execute_after(
        &self,
        _context: &ExecContext<'_>,
        _first_result: bool,
    ) -> Result<(), DruidError> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push("success_after".to_string());
        Err(DruidError::Other("success callback failed".to_string()))
    }

    async fn statement_execute_error_after(
        &self,
        sql: &str,
        error: &DruidError,
    ) -> Result<(), DruidError> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("error_after:{sql}:{error}"));
        Ok(())
    }
}

#[tokio::test]
async fn success_callback_failure_enters_error_after_and_remains_primary() {
    let event_log = Arc::new(Mutex::new(Vec::new()));
    let adapter = FilterEventAdapter::with_listener(FailingSuccessListener {
        events: Arc::clone(&event_log),
    });
    let context = ExecContext {
        connection_id: 7,
        statement_id: Some(20_003),
        sql: "SELECT 1".to_owned(),
        params: &[],
        prepared_parameters: None,
        data_source: "event-test",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Execute,
    };
    let result = Ok(ExecResult {
        rows_affected: 0,
        last_insert_id: None,
        row_count: Some(1),
    });

    // RUST_OBLIGATION / V1_RUST_LOCAL：
    // Java success-after 位于 try 内；它抛出的 RuntimeException 会再进入
    // statement_executeErrorAfter，随后原 success-after 异常继续向上传播。
    let error = AfterFilter::after(&adapter, &context, &result, Duration::ZERO)
        .await
        .unwrap_err();
    assert_eq!(
        error,
        DruidError::Other("success callback failed".to_string())
    );
    assert_eq!(
        events(&event_log),
        [
            "success_after",
            "error_after:SELECT 1:success callback failed"
        ]
    );
}

struct ReplacingErrorListener;

#[async_trait::async_trait]
impl FilterEventListener for ReplacingErrorListener {
    async fn statement_execute_error_after(
        &self,
        _sql: &str,
        _error: &DruidError,
    ) -> Result<(), DruidError> {
        Err(DruidError::Other("error callback failed".to_string()))
    }
}

#[tokio::test]
async fn error_callback_failure_replaces_the_original_execution_error() {
    let adapter = FilterEventAdapter::with_listener(ReplacingErrorListener);
    let context = ExecContext {
        connection_id: 7,
        statement_id: Some(20_004),
        sql: "BROKEN SQL".to_owned(),
        params: &[],
        prepared_parameters: None,
        data_source: "event-test",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Query,
    };
    let result = Err(DruidError::DriverError("physical failure".to_string()));

    // SOURCE_PARITY / V0_STATIC + V1_RUST_LOCAL：
    // Java catch 分支调用 error-after；若 error-after 自己抛出异常，它会替代正在
    // 重抛的物理异常。Java 仓测试只断言“存在异常”，没有覆盖替换值。
    assert_eq!(
        AfterFilter::after(&adapter, &context, &result, Duration::ZERO)
            .await
            .unwrap_err(),
        DruidError::Other("error callback failed".to_string())
    );
}

#[tokio::test]
async fn real_toasty_sqlite_preserves_operation_specific_success_and_error_events() {
    let event_log = Arc::new(Mutex::new(Vec::new()));
    let listener = RecordingListener {
        label: "sqlite",
        events: Arc::clone(&event_log),
    };
    let mut filter_chain = FilterChain::new();
    filter_chain.add_filter(Arc::new(FilterEventAdapter::with_listener(listener)));

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 Toasty SQLite 物理连接");
    let mut connection = DruidPooledConnection::with_context(
        physical,
        101,
        "sqlite-filter-event-adapter".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();

    let create_sql = "CREATE TABLE event_item(id INTEGER PRIMARY KEY, value TEXT)";
    statement
        .execute_update(&mut connection, create_sql)
        .await
        .unwrap();

    let insert_sql = "INSERT INTO event_item(id, value) VALUES (1, '一')";
    assert_eq!(
        statement
            .execute_update(&mut connection, insert_sql)
            .await
            .unwrap()
            .rows_affected,
        1
    );

    let query_sql = "SELECT value FROM event_item ORDER BY id";
    let mut result_set = statement
        .execute_query_result_set(&mut connection, query_sql)
        .await
        .unwrap();
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(
        result_set.n_string(&mut connection, 1).unwrap(),
        Some("一".to_string())
    );
    result_set.close_with_connection(&mut connection).unwrap();

    let generic_query_sql = "SELECT 7";
    assert!(statement
        .execute(&mut connection, generic_query_sql)
        .await
        .unwrap());
    let mut generic_result_set = statement
        .result_set(&mut connection)
        .unwrap()
        .expect("generic query 必须产生结果集");
    assert!(generic_result_set.next(&mut connection).unwrap());
    generic_result_set
        .close_with_connection(&mut connection)
        .unwrap();

    let generic_update_sql = "INSERT INTO event_item(id, value) VALUES (2, '二')";
    assert!(!statement
        .execute(&mut connection, generic_update_sql)
        .await
        .unwrap());

    let batch_one = "INSERT INTO event_item(id, value) VALUES (3, '三')";
    let batch_two = "INSERT INTO event_item(id, value) VALUES (4, '四')";
    statement.add_batch(&mut connection, batch_one).unwrap();
    statement.add_batch(&mut connection, batch_two).unwrap();
    assert_eq!(
        statement.execute_batch(&mut connection).await.unwrap(),
        [1, 1]
    );

    let prepared_sql = "INSERT INTO event_item(id, value) VALUES (?1, ?2)";
    let mut prepared = connection.prepare_statement(prepared_sql).await.unwrap();
    prepared.set_int(&mut connection, 1, 5).unwrap();
    prepared
        .set_n_string(&mut connection, 2, Some("五".to_string()))
        .unwrap();
    assert_eq!(
        prepared
            .execute_update_bound(&mut connection)
            .await
            .unwrap()
            .rows_affected,
        1
    );
    prepared.close_with_connection(&mut connection).unwrap();

    let invalid_sql = "SELECT * FROM missing_event_table";
    let error = statement
        .execute_query(&mut connection, invalid_sql)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        DruidError::SqlException(_) | DruidError::DriverError(_)
    ));

    let recorded = events(&event_log);

    // VALUE_ADD / V5_HOST：
    // 真实 SQLite 证明创建、三类 execute、batch、prepared 与 ResultSet open
    // 均经过生产调用链，而不是仅直接调用 listener。
    assert!(recorded.iter().any(|event| event == "sqlite:create_after"));
    assert!(recorded
        .iter()
        .any(|event| event == &format!("sqlite:update_before:{insert_sql}")));
    assert!(recorded
        .iter()
        .any(|event| event == &format!("sqlite:update_after:{insert_sql}:1")));
    assert!(recorded
        .iter()
        .any(|event| event == &format!("sqlite:query_before:{query_sql}")));
    assert!(recorded
        .iter()
        .any(|event| event == &format!("sqlite:query_after:{query_sql}")));
    assert!(
        recorded
            .iter()
            .filter(|event| event.as_str() == "sqlite:result_set_open_after")
            .count()
            >= 2
    );
    assert!(recorded
        .iter()
        .any(|event| { event == &format!("sqlite:execute_after:{generic_query_sql}:true") }));
    assert!(recorded
        .iter()
        .any(|event| { event == &format!("sqlite:execute_after:{generic_update_sql}:false") }));
    let merged_batch = format!("{batch_one}\n;\n{batch_two}");
    assert!(recorded
        .iter()
        .any(|event| event == &format!("sqlite:batch_before:{merged_batch}")));
    assert!(recorded
        .iter()
        .any(|event| event == &format!("sqlite:batch_after:{merged_batch}:[1, 1]")));
    assert!(recorded
        .iter()
        .any(|event| event == &format!("sqlite:prepare_after:{prepared_sql}")));
    assert!(recorded
        .iter()
        .any(|event| event == &format!("sqlite:update_after:{prepared_sql}:1")));

    // SOURCE_PARITY / V0_STATIC + V1_RUST_LOCAL + V5_HOST：
    // Java RdbcFilterEventAdapterTest 的 SQLException 分支要求错误原样传播。
    // 该 Java 测试没有校验事件次数，且本用例没有复制其全部 45 个异常分支，
    // 因此不冒充 V2_MIRRORED；这里只证明真实 SQLite 查询错误同时触发 error-after。
    assert!(recorded
        .iter()
        .any(|event| { event.starts_with(&format!("sqlite:error_after:{invalid_sql}:")) }));

    // SOURCE_PARITY / V0_STATIC + V5_HOST：
    // Java 查询成功顺序固定为 before -> query-after -> resultSet-open；错误路径
    // 固定为 before -> error-after，且不能误发 query-after。
    let query_before = recorded
        .iter()
        .position(|event| event == &format!("sqlite:query_before:{query_sql}"))
        .unwrap();
    assert_eq!(
        recorded[query_before + 1],
        format!("sqlite:query_after:{query_sql}")
    );
    assert_eq!(recorded[query_before + 2], "sqlite:result_set_open_after");
    let invalid_before = recorded
        .iter()
        .position(|event| event == &format!("sqlite:query_before:{invalid_sql}"))
        .unwrap();
    assert!(recorded[invalid_before + 1].starts_with(&format!("sqlite:error_after:{invalid_sql}:")));
    assert!(!recorded
        .iter()
        .any(|event| event == &format!("sqlite:query_after:{invalid_sql}")));
}
