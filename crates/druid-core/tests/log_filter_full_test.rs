//! LogFilter 全量覆盖测试（Java `LogFilter.java` 差分对照）。
//!
//! 覆盖目标：
//! - 全部 is_xxx_log_enabled / set_xxx_log_enabled 开关（含父级开关联动）
//! - config_from_properties 七键精确语义
//! - operation_success_enabled 四种 ExecOperation 分支
//! - after / after_batch 成功与错误日志分支
//! - after_connection_event 六种 ConnectionEvent 分支
//! - on_statement_event / on_statement_event_context 四种 StatementEvent 分支
//! - on_statement_close_context
//! - on_connection_event / on_connection_event_context no-op
//! - ResultSetFilter 三方法（open_after / next / close）成功与错误分支
//! - Default / statement_sql_format_option / statement_sql_pretty_format
//! - set_statement_parameter_log_enabled 旧 API 兼容

extern crate druid_core as druid;
use druid_core::core::{
    AfterFilter, BatchExecContext, BatchExecKind, BeforeFilter, ConnectionEvent,
    ConnectionEventContext, DruidError, ExecContext, ExecOperation, ExecResult, LogFilter,
    PhysicalResultSet, ResultSetFilter, ResultSetFilterChain, ResultSetFilterContext,
    ResultSetMetaData, StatementEvent, StatementEventContext, Value,
};
use druid_core::sql::SqlFormatOption;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn props(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

fn exec_context(operation: ExecOperation) -> (Vec<Value>, ExecContext<'static>) {
    // Leak the params vec so we can return a context with a static lifetime.
    // This is acceptable in tests; we never mutate the vec after creation.
    let params: &'static [Value] = &[];
    let ctx = ExecContext {
        connection_id: 1,
        statement_id: Some(2),
        sql: "SELECT 1".to_owned(),
        params,
        prepared_parameters: None,
        data_source: "test-ds",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation,
    };
    (vec![], ctx)
}

// ── 1. 全量开关 getter/setter ─────────────────────────────────

/// 全部 30+ getter 在默认构造下符合 Java 默认值。
#[test]
fn log_filter_all_getters_default_values() {
    let f = LogFilter::new();

    // 数据源级
    assert!(f.is_data_source_log_enabled());
    // 连接级
    assert!(f.is_connection_log_enabled());
    assert!(f.is_connection_log_error_enabled());
    assert!(f.is_connection_connect_before_log_enabled());
    assert!(f.is_connection_connect_after_log_enabled());
    assert!(f.is_connection_close_after_log_enabled());
    assert!(f.is_connection_commit_after_log_enabled());
    assert!(f.is_connection_rollback_after_log_enabled());
    // 语句级
    assert!(f.is_statement_log_enabled());
    assert!(f.is_statement_log_error_enabled());
    assert!(f.is_statement_create_after_log_enabled());
    assert!(f.is_statement_prepare_after_log_enabled());
    assert!(f.is_statement_prepare_call_after_log_enabled());
    assert!(f.is_statement_execute_after_log_enabled());
    assert!(f.is_statement_execute_query_after_log_enabled());
    assert!(f.is_statement_execute_update_after_log_enabled());
    assert!(f.is_statement_execute_batch_after_log_enabled());
    assert!(f.is_statement_close_after_log_enabled());
    assert!(f.is_statement_parameter_set_log_enabled());
    assert!(f.is_statement_parameter_clear_log_enabled());
    // executableSql 默认关闭（Java 默认 false）
    assert!(!f.is_statement_executable_sql_log_enabled());
    // 结果集级
    assert!(f.is_result_set_log_enabled());
    assert!(f.is_result_set_log_error_enabled());
    assert!(f.is_result_set_open_after_log_enabled());
    assert!(f.is_result_set_next_after_log_enabled());
    assert!(f.is_result_set_close_after_log_enabled());
    // SQL 格式
    assert!(!f.is_statement_sql_pretty_format());
}

/// 全部 setter 可正确翻转 getter 值。
#[test]
fn log_filter_all_setters_toggle_getters() {
    let f = LogFilter::new();

    f.set_data_source_log_enabled(false);
    assert!(!f.is_data_source_log_enabled());

    f.set_connection_log_enabled(false);
    assert!(!f.is_connection_log_enabled());
    // 子开关依赖 connection_log_enabled，父关则子关
    assert!(!f.is_connection_connect_before_log_enabled());
    assert!(!f.is_connection_connect_after_log_enabled());
    assert!(!f.is_connection_close_after_log_enabled());
    assert!(!f.is_connection_commit_after_log_enabled());
    assert!(!f.is_connection_rollback_after_log_enabled());

    f.set_connection_log_error_enabled(false);
    assert!(!f.is_connection_log_error_enabled());

    f.set_connection_connect_before_log_enabled(false);
    assert!(!f.is_connection_connect_before_log_enabled());
    f.set_connection_connect_after_log_enabled(false);
    assert!(!f.is_connection_connect_after_log_enabled());
    f.set_connection_close_after_log_enabled(false);
    assert!(!f.is_connection_close_after_log_enabled());
    f.set_connection_commit_after_log_enabled(false);
    assert!(!f.is_connection_commit_after_log_enabled());
    f.set_connection_rollback_after_log_enabled(false);
    assert!(!f.is_connection_rollback_after_log_enabled());

    // 恢复父级，子级仍为手动关闭
    f.set_connection_log_enabled(true);
    assert!(f.is_connection_log_enabled());
    assert!(!f.is_connection_connect_before_log_enabled());
    assert!(!f.is_connection_close_after_log_enabled());

    f.set_statement_log_enabled(false);
    assert!(!f.is_statement_log_enabled());
    assert!(!f.is_statement_create_after_log_enabled());
    assert!(!f.is_statement_prepare_after_log_enabled());
    assert!(!f.is_statement_prepare_call_after_log_enabled());
    assert!(!f.is_statement_execute_after_log_enabled());
    assert!(!f.is_statement_execute_query_after_log_enabled());
    assert!(!f.is_statement_execute_update_after_log_enabled());
    assert!(!f.is_statement_execute_batch_after_log_enabled());
    assert!(!f.is_statement_close_after_log_enabled());
    assert!(!f.is_statement_parameter_set_log_enabled());
    assert!(!f.is_statement_parameter_clear_log_enabled());

    f.set_statement_log_error_enabled(false);
    assert!(!f.is_statement_log_error_enabled());

    f.set_statement_create_after_log_enabled(false);
    f.set_statement_prepare_after_log_enabled(false);
    f.set_statement_prepare_call_after_log_enabled(false);
    f.set_statement_execute_after_log_enabled(false);
    f.set_statement_execute_query_after_log_enabled(false);
    f.set_statement_execute_update_after_log_enabled(false);
    f.set_statement_execute_batch_after_log_enabled(false);
    f.set_statement_close_after_log_enabled(false);
    f.set_statement_parameter_set_log_enabled(false);
    f.set_statement_parameter_clear_log_enabled(false);

    // 恢复父级，子级仍关闭
    f.set_statement_log_enabled(true);
    assert!(!f.is_statement_create_after_log_enabled());

    f.set_statement_executable_sql_log_enabled(true);
    assert!(f.is_statement_executable_sql_log_enabled());
    f.set_statement_executable_sql_log_enabled(false);
    assert!(!f.is_statement_executable_sql_log_enabled());

    f.set_result_set_log_enabled(false);
    assert!(!f.is_result_set_log_enabled());
    assert!(!f.is_result_set_open_after_log_enabled());
    assert!(!f.is_result_set_next_after_log_enabled());
    assert!(!f.is_result_set_close_after_log_enabled());

    f.set_result_set_log_error_enabled(false);
    assert!(!f.is_result_set_log_error_enabled());

    f.set_result_set_open_after_log_enabled(false);
    f.set_result_set_next_after_log_enabled(false);
    f.set_result_set_close_after_log_enabled(false);

    f.set_result_set_log_enabled(true);
    assert!(!f.is_result_set_open_after_log_enabled());

    f.set_statement_sql_pretty_format(true);
    assert!(f.is_statement_sql_pretty_format());
}

/// 旧 API set_statement_parameter_log_enabled 委托给 set_statement_parameter_set_log_enabled。
#[test]
fn log_filter_legacy_parameter_log_setter_delegates() {
    let f = LogFilter::new();
    assert!(f.is_statement_parameter_set_log_enabled());
    f.set_statement_parameter_log_enabled(false);
    assert!(!f.is_statement_parameter_set_log_enabled());
    f.set_statement_parameter_log_enabled(true);
    assert!(f.is_statement_parameter_set_log_enabled());
}

// ── 2. config_from_properties 七键精确语义 ─────────────────────

/// 七键 "true"/"false" 值精确匹配，非布尔值不改变状态。
#[test]
fn log_filter_config_from_properties_exact_true_false() {
    let f = LogFilter::new();

    // "true" 开启 executableSql（默认关闭），其余关闭
    f.config_from_properties(&props(&[
        ("druid.log.conn", "false"),
        ("druid.log.stmt", "false"),
        ("druid.log.rs", "false"),
        ("druid.log.stmt.executableSql", "true"),
        ("druid.log.conn.logError", "false"),
        ("druid.log.stmt.logError", "false"),
        ("druid.log.rs.logError", "false"),
    ]));
    assert!(!f.is_connection_log_enabled());
    assert!(!f.is_statement_log_enabled());
    assert!(!f.is_result_set_log_enabled());
    assert!(f.is_statement_executable_sql_log_enabled());
    assert!(!f.is_connection_log_error_enabled());
    assert!(!f.is_statement_log_error_enabled());
    assert!(!f.is_result_set_log_error_enabled());

    // 非 "true"/"false" 值（如 "yes"、""、"1"）不改变当前状态
    f.config_from_properties(&props(&[
        ("druid.log.conn", "yes"),
        ("druid.log.stmt", ""),
        ("druid.log.rs", "1"),
        ("druid.log.stmt.executableSql", "TRUE"),
    ]));
    assert!(!f.is_connection_log_enabled()); // unchanged (was false)
    assert!(!f.is_statement_log_enabled()); // unchanged (was false)
    assert!(!f.is_result_set_log_enabled()); // unchanged (was false)
    assert!(f.is_statement_executable_sql_log_enabled()); // unchanged (was true)

    // "TRUE" / "FALSE" 也不匹配（Java Boolean.parseBoolean 大小写敏感）
    f.config_from_properties(&props(&[
        ("druid.log.conn", "TRUE"),
        ("druid.log.stmt", "FALSE"),
    ]));
    assert!(!f.is_connection_log_enabled()); // "TRUE" doesn't match
    assert!(!f.is_statement_log_enabled()); // "FALSE" doesn't match

    // 恢复正确值
    f.config_from_properties(&props(&[
        ("druid.log.conn", "true"),
        ("druid.log.stmt", "true"),
        ("druid.log.rs", "true"),
    ]));
    assert!(f.is_connection_log_enabled());
    assert!(f.is_statement_log_enabled());
    assert!(f.is_result_set_log_enabled());
}

/// 缺失键不改变对应开关。
#[test]
fn log_filter_config_from_properties_missing_keys_preserve_state() {
    let f = LogFilter::new();
    // 默认全部开启
    assert!(f.is_connection_log_enabled());
    assert!(f.is_statement_log_enabled());

    // 仅传一个键
    f.config_from_properties(&props(&[("druid.log.conn", "false")]));
    assert!(!f.is_connection_log_enabled());
    assert!(f.is_statement_log_enabled()); // 未提及，保持原值

    // 空 HashMap
    f.config_from_properties(&HashMap::new());
    assert!(!f.is_connection_log_enabled()); // unchanged
    assert!(f.is_statement_log_enabled()); // unchanged
}

/// 操作无关的键不产生任何副作用。
#[test]
fn log_filter_config_from_properties_irrelevant_keys_ignored() {
    let f = LogFilter::new();
    f.config_from_properties(&props(&[
        ("druid.log.unrelated", "false"),
        ("some.other.key", "true"),
        ("druid.log.conn.extra", "false"),
    ]));
    // 全部默认值不变
    assert!(f.is_connection_log_enabled());
    assert!(f.is_statement_log_enabled());
    assert!(f.is_result_set_log_enabled());
}

// ── 3. SQL 格式选项 ────────────────────────────────────────────

/// SqlFormatOption getter/setter 往返。
#[test]
fn log_filter_sql_format_option_round_trip() {
    let f = LogFilter::new();
    let default_opt = f.statement_sql_format_option();
    assert!(!default_opt.is_ucase());
    assert!(default_opt.is_pretty_format());
    assert!(!default_opt.is_parameterized());

    let custom = SqlFormatOption::new(true, false, true);
    f.set_statement_sql_format_option(custom);
    let opt = f.statement_sql_format_option();
    assert!(opt.is_ucase());
    assert!(!opt.is_pretty_format());
    assert!(opt.is_parameterized());
}

// ── 4. operation_success_enabled 四分支（间接通过 after） ───────

/// Execute 操作在 after 中应触发 execute_after 开关。
#[tokio::test]
async fn log_filter_after_execute_operation_uses_execute_switch() {
    let f = LogFilter::new();
    f.set_statement_execute_after_log_enabled(true);

    let (_, ctx) = exec_context(ExecOperation::Execute);
    let result = Ok(ExecResult {
        rows_affected: 1,
        last_insert_id: None,
        row_count: Some(1),
    });
    // 不 panic 即通过，日志通过 tracing 发出
    AfterFilter::after(&f, &ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
}

/// Query 操作在 after 中应触发 execute_query_after 开关。
#[tokio::test]
async fn log_filter_after_query_operation_uses_query_switch() {
    let f = LogFilter::new();
    f.set_statement_execute_query_after_log_enabled(true);

    let (_, ctx) = exec_context(ExecOperation::Query);
    let result = Ok(ExecResult {
        rows_affected: 0,
        last_insert_id: None,
        row_count: Some(1),
    });
    AfterFilter::after(&f, &ctx, &result, Duration::from_millis(1))
        .await
        .unwrap();
}

/// Update 操作在 after 中应触发 execute_update_after 开关。
#[tokio::test]
async fn log_filter_after_update_operation_uses_update_switch() {
    let f = LogFilter::new();
    f.set_statement_execute_update_after_log_enabled(true);

    let (_, ctx) = exec_context(ExecOperation::Update);
    let result = Ok(ExecResult {
        rows_affected: 3,
        last_insert_id: None,
        row_count: None,
    });
    AfterFilter::after(&f, &ctx, &result, Duration::from_millis(2))
        .await
        .unwrap();
}

/// Batch 操作在 after 中不触发任何成功日志（走 after_batch 通道）。
#[tokio::test]
async fn log_filter_after_batch_operation_is_noop_for_success() {
    let f = LogFilter::new();
    let (_, ctx) = exec_context(ExecOperation::Batch);
    let result = Ok(ExecResult::default());
    AfterFilter::after(&f, &ctx, &result, Duration::ZERO)
        .await
        .unwrap();
}

/// 对应开关关闭时，成功日志不发出。
#[tokio::test]
async fn log_filter_after_success_suppressed_when_switch_off() {
    let f = LogFilter::new();
    f.set_statement_execute_after_log_enabled(false);
    f.set_statement_execute_query_after_log_enabled(false);
    f.set_statement_execute_update_after_log_enabled(false);

    for op in [
        ExecOperation::Execute,
        ExecOperation::Query,
        ExecOperation::Update,
    ] {
        let (_, ctx) = exec_context(op);
        let result = Ok(ExecResult::default());
        AfterFilter::after(&f, &ctx, &result, Duration::ZERO)
            .await
            .unwrap();
    }
}

/// 错误路径：statement_log_error_enabled 开启时发出 error 日志。
#[tokio::test]
async fn log_filter_after_error_emitted_when_error_switch_on() {
    let f = LogFilter::new();
    f.set_statement_log_error_enabled(true);

    let (_, ctx) = exec_context(ExecOperation::Execute);
    let error = Err(DruidError::DriverError("boom".to_string()));
    AfterFilter::after(&f, &ctx, &error, Duration::from_millis(10))
        .await
        .unwrap();
}

/// 错误路径：statement_log_error_enabled 关闭时静默。
#[tokio::test]
async fn log_filter_after_error_suppressed_when_error_switch_off() {
    let f = LogFilter::new();
    f.set_statement_log_error_enabled(false);

    let (_, ctx) = exec_context(ExecOperation::Query);
    let error = Err(DruidError::DriverError("silence".to_string()));
    AfterFilter::after(&f, &ctx, &error, Duration::ZERO)
        .await
        .unwrap();
}

// ── 5. after_batch 成功与错误 ──────────────────────────────────

/// batch 成功 + 开关开：发出日志。
#[tokio::test]
async fn log_filter_after_batch_success_when_enabled() {
    let f = LogFilter::new();
    f.set_statement_execute_batch_after_log_enabled(true);

    let statements = vec!["INSERT INTO t VALUES (1)".to_string()];
    let ctx = BatchExecContext {
        connection_id: 1,
        statement_id: Some(3),
        sql: "INSERT INTO t VALUES (1)",
        statements: &statements,
        parameter_sets: &[],
        prepared_parameter_sets: None,
        kind: BatchExecKind::Statement,
        data_source: "test-ds",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
    };
    AfterFilter::after_batch(&f, &ctx, &Ok(vec![1, 1]), Duration::from_millis(3))
        .await
        .unwrap();
}

/// batch 成功 + 开关关：静默。
#[tokio::test]
async fn log_filter_after_batch_success_suppressed_when_disabled() {
    let f = LogFilter::new();
    f.set_statement_execute_batch_after_log_enabled(false);

    let statements = vec!["INSERT INTO t VALUES (1)".to_string()];
    let ctx = BatchExecContext {
        connection_id: 1,
        statement_id: Some(3),
        sql: "INSERT INTO t VALUES (1)",
        statements: &statements,
        parameter_sets: &[],
        prepared_parameter_sets: None,
        kind: BatchExecKind::Statement,
        data_source: "test-ds",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
    };
    AfterFilter::after_batch(&f, &ctx, &Ok(vec![1]), Duration::ZERO)
        .await
        .unwrap();
}

/// batch 错误 + error 开关开：发出 error 日志。
#[tokio::test]
async fn log_filter_after_batch_error_when_error_enabled() {
    let f = LogFilter::new();
    f.set_statement_log_error_enabled(true);

    let statements = vec!["BAD SQL".to_string()];
    let ctx = BatchExecContext {
        connection_id: 1,
        statement_id: Some(4),
        sql: "BAD SQL",
        statements: &statements,
        parameter_sets: &[],
        prepared_parameter_sets: None,
        kind: BatchExecKind::Statement,
        data_source: "test-ds",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
    };
    let error = Err(DruidError::DriverError("batch error".to_string()));
    AfterFilter::after_batch(&f, &ctx, &error, Duration::from_millis(7))
        .await
        .unwrap();
}

/// batch 错误 + error 开关关：静默。
#[tokio::test]
async fn log_filter_after_batch_error_suppressed_when_error_disabled() {
    let f = LogFilter::new();
    f.set_statement_log_error_enabled(false);

    let statements = vec!["BAD SQL".to_string()];
    let ctx = BatchExecContext {
        connection_id: 1,
        statement_id: Some(4),
        sql: "BAD SQL",
        statements: &statements,
        parameter_sets: &[],
        prepared_parameter_sets: None,
        kind: BatchExecKind::Statement,
        data_source: "test-ds",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
    };
    let error = Err(DruidError::DriverError("batch error".to_string()));
    AfterFilter::after_batch(&f, &ctx, &error, Duration::ZERO)
        .await
        .unwrap();
}

// ── 6. after_connection_event 六种 ConnectionEvent ─────────────

/// Connect 事件始终静默（Java LogFilter 不覆盖 connectBefore）。
#[tokio::test]
async fn log_filter_after_connection_event_connect_always_silent() {
    let f = LogFilter::new();
    AfterFilter::after_connection_event(&f, &ConnectionEvent::Connect, Duration::from_millis(1))
        .await
        .unwrap();
}

/// Commit 事件：开关控制。
#[tokio::test]
async fn log_filter_after_connection_event_commit_controlled() {
    let f = LogFilter::new();
    // 默认开启
    f.set_connection_commit_after_log_enabled(true);
    AfterFilter::after_connection_event(&f, &ConnectionEvent::Commit, Duration::from_millis(2))
        .await
        .unwrap();

    f.set_connection_commit_after_log_enabled(false);
    AfterFilter::after_connection_event(&f, &ConnectionEvent::Commit, Duration::ZERO)
        .await
        .unwrap();
}

/// Rollback 事件：开关控制。
#[tokio::test]
async fn log_filter_after_connection_event_rollback_controlled() {
    let f = LogFilter::new();
    f.set_connection_rollback_after_log_enabled(true);
    AfterFilter::after_connection_event(&f, &ConnectionEvent::Rollback, Duration::from_millis(3))
        .await
        .unwrap();

    f.set_connection_rollback_after_log_enabled(false);
    AfterFilter::after_connection_event(&f, &ConnectionEvent::Rollback, Duration::ZERO)
        .await
        .unwrap();
}

/// Close 事件：开关控制。
#[tokio::test]
async fn log_filter_after_connection_event_close_controlled() {
    let f = LogFilter::new();
    f.set_connection_close_after_log_enabled(true);
    AfterFilter::after_connection_event(&f, &ConnectionEvent::Close, Duration::from_millis(4))
        .await
        .unwrap();

    f.set_connection_close_after_log_enabled(false);
    AfterFilter::after_connection_event(&f, &ConnectionEvent::Close, Duration::ZERO)
        .await
        .unwrap();
}

/// SetAutoCommit 等其他事件：由 connection_log_enabled 总开关控制。
#[tokio::test]
async fn log_filter_after_connection_event_other_events_use_connection_log_switch() {
    let f = LogFilter::new();
    f.set_connection_log_enabled(true);
    AfterFilter::after_connection_event(
        &f,
        &ConnectionEvent::SetAutoCommit(true),
        Duration::from_millis(1),
    )
    .await
    .unwrap();
    AfterFilter::after_connection_event(
        &f,
        &ConnectionEvent::GetAutoCommit,
        Duration::from_millis(1),
    )
    .await
    .unwrap();
    AfterFilter::after_connection_event(
        &f,
        &ConnectionEvent::SetReadOnly(false),
        Duration::from_millis(1),
    )
    .await
    .unwrap();

    f.set_connection_log_enabled(false);
    AfterFilter::after_connection_event(&f, &ConnectionEvent::SetAutoCommit(false), Duration::ZERO)
        .await
        .unwrap();
}

// ── 7. after_connection_event_context ──────────────────────────

/// 带 connection_id 的事件上下文，走相同分支逻辑。
#[tokio::test]
async fn log_filter_after_connection_event_context_with_identity() {
    let f = LogFilter::new();
    f.set_connection_commit_after_log_enabled(true);
    let event = ConnectionEvent::Commit;
    let ctx = ConnectionEventContext {
        connection_id: 42,
        event: &event,
    };
    AfterFilter::after_connection_event_context(&f, &ctx, Duration::from_millis(5))
        .await
        .unwrap();

    f.set_connection_commit_after_log_enabled(false);
    AfterFilter::after_connection_event_context(&f, &ctx, Duration::ZERO)
        .await
        .unwrap();

    // Connect 始终静默
    let connect = ConnectionEvent::Connect;
    let connect_ctx = ConnectionEventContext {
        connection_id: 42,
        event: &connect,
    };
    AfterFilter::after_connection_event_context(&f, &connect_ctx, Duration::ZERO)
        .await
        .unwrap();
}

// ── 8. on_statement_event 四种 StatementEvent ──────────────────

/// CreateStatement 事件：由 create_after 开关控制。
#[tokio::test]
async fn log_filter_on_statement_event_create() {
    let f = LogFilter::new();
    f.set_statement_create_after_log_enabled(true);
    BeforeFilter::on_statement_event(&f, &StatementEvent::CreateStatement)
        .await
        .unwrap();

    f.set_statement_create_after_log_enabled(false);
    BeforeFilter::on_statement_event(&f, &StatementEvent::CreateStatement)
        .await
        .unwrap();
}

/// PrepareStatement 事件：由 prepare_after 开关控制。
#[tokio::test]
async fn log_filter_on_statement_event_prepare() {
    let f = LogFilter::new();
    f.set_statement_prepare_after_log_enabled(true);
    BeforeFilter::on_statement_event(
        &f,
        &StatementEvent::PrepareStatement("SELECT ?".to_string()),
    )
    .await
    .unwrap();

    f.set_statement_prepare_after_log_enabled(false);
    BeforeFilter::on_statement_event(
        &f,
        &StatementEvent::PrepareStatement("SELECT ?".to_string()),
    )
    .await
    .unwrap();
}

/// PrepareCall 事件：由 prepare_call_after 开关控制。
#[tokio::test]
async fn log_filter_on_statement_event_prepare_call() {
    let f = LogFilter::new();
    f.set_statement_prepare_call_after_log_enabled(true);
    BeforeFilter::on_statement_event(&f, &StatementEvent::PrepareCall("CALL p()".to_string()))
        .await
        .unwrap();

    f.set_statement_prepare_call_after_log_enabled(false);
    BeforeFilter::on_statement_event(&f, &StatementEvent::PrepareCall("CALL p()".to_string()))
        .await
        .unwrap();
}

/// Close 事件：由 close_after 开关控制。
#[tokio::test]
async fn log_filter_on_statement_event_close() {
    let f = LogFilter::new();
    f.set_statement_close_after_log_enabled(true);
    BeforeFilter::on_statement_event(&f, &StatementEvent::Close)
        .await
        .unwrap();

    f.set_statement_close_after_log_enabled(false);
    BeforeFilter::on_statement_event(&f, &StatementEvent::Close)
        .await
        .unwrap();
}

/// Execute/ExecuteQuery/ExecuteUpdate/ExecuteBatch 事件走 default 分支，静默。
#[tokio::test]
async fn log_filter_on_statement_event_execute_variants_are_silent() {
    let f = LogFilter::new();
    for event in [
        StatementEvent::Execute("SELECT 1".to_string()),
        StatementEvent::ExecuteQuery("SELECT 1".to_string()),
        StatementEvent::ExecuteUpdate("UPDATE t".to_string()),
        StatementEvent::ExecuteBatch,
    ] {
        BeforeFilter::on_statement_event(&f, &event).await.unwrap();
    }
}

// ── 9. on_statement_event_context ──────────────────────────────

/// 带身份的 Statement 事件上下文。
#[tokio::test]
async fn log_filter_on_statement_event_context_with_identity() {
    let f = LogFilter::new();

    f.set_statement_create_after_log_enabled(true);
    let event = StatementEvent::CreateStatement;
    let ctx = StatementEventContext {
        connection_id: 10,
        statement_id: 20,
        event: &event,
    };
    BeforeFilter::on_statement_event_context(&f, &ctx)
        .await
        .unwrap();

    f.set_statement_prepare_after_log_enabled(true);
    let prepare = StatementEvent::PrepareStatement("INSERT ?".to_string());
    let prepare_ctx = StatementEventContext {
        connection_id: 10,
        statement_id: 21,
        event: &prepare,
    };
    BeforeFilter::on_statement_event_context(&f, &prepare_ctx)
        .await
        .unwrap();

    f.set_statement_prepare_call_after_log_enabled(true);
    let call = StatementEvent::PrepareCall("CALL".to_string());
    let call_ctx = StatementEventContext {
        connection_id: 10,
        statement_id: 22,
        event: &call,
    };
    BeforeFilter::on_statement_event_context(&f, &call_ctx)
        .await
        .unwrap();

    f.set_statement_close_after_log_enabled(true);
    let close = StatementEvent::Close;
    let close_ctx = StatementEventContext {
        connection_id: 10,
        statement_id: 23,
        event: &close,
    };
    BeforeFilter::on_statement_event_context(&f, &close_ctx)
        .await
        .unwrap();

    // Execute 等 default 分支
    let exec = StatementEvent::Execute("SELECT 1".to_string());
    let exec_ctx = StatementEventContext {
        connection_id: 10,
        statement_id: 24,
        event: &exec,
    };
    BeforeFilter::on_statement_event_context(&f, &exec_ctx)
        .await
        .unwrap();
}

// ── 10. on_statement_close_context ─────────────────────────────

/// on_statement_close_context 同步方法：开关开时发出日志。
#[test]
fn log_filter_on_statement_close_context_enabled() {
    let f = LogFilter::new();
    f.set_statement_close_after_log_enabled(true);
    let event = StatementEvent::Close;
    let ctx = StatementEventContext {
        connection_id: 5,
        statement_id: 6,
        event: &event,
    };
    BeforeFilter::on_statement_close_context(&f, &ctx).unwrap();
}

/// on_statement_close_context 同步方法：开关关时静默。
#[test]
fn log_filter_on_statement_close_context_disabled() {
    let f = LogFilter::new();
    f.set_statement_close_after_log_enabled(false);
    let event = StatementEvent::Close;
    let ctx = StatementEventContext {
        connection_id: 5,
        statement_id: 6,
        event: &event,
    };
    BeforeFilter::on_statement_close_context(&f, &ctx).unwrap();
}

// ── 11. on_connection_event / on_connection_event_context no-op ─

#[tokio::test]
async fn log_filter_on_connection_event_is_noop() {
    let f = LogFilter::new();
    BeforeFilter::on_connection_event(&f, &ConnectionEvent::Connect)
        .await
        .unwrap();
    BeforeFilter::on_connection_event(&f, &ConnectionEvent::Close)
        .await
        .unwrap();
}

#[tokio::test]
async fn log_filter_on_connection_event_context_is_noop() {
    let f = LogFilter::new();
    let event = ConnectionEvent::Commit;
    let ctx = ConnectionEventContext {
        connection_id: 1,
        event: &event,
    };
    BeforeFilter::on_connection_event_context(&f, &ctx)
        .await
        .unwrap();
}

// ── 12. before 方法（参数日志） ────────────────────────────────

/// 参数非空 + 开关开：发出参数日志。
#[tokio::test]
async fn log_filter_before_with_params_emits_log_when_enabled() {
    let f = LogFilter::new();
    f.set_statement_parameter_set_log_enabled(true);
    let params = vec![Value::Int(1), Value::String("hello".to_string())];
    let mut ctx = ExecContext {
        connection_id: 1,
        statement_id: Some(2),
        sql: "SELECT * FROM t WHERE id = ? AND name = ?".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test-ds",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Execute,
    };
    BeforeFilter::before(&f, &mut ctx).await.unwrap();
}

/// 参数为空：不发出参数日志。
#[tokio::test]
async fn log_filter_before_with_empty_params_no_log() {
    let f = LogFilter::new();
    f.set_statement_parameter_set_log_enabled(true);
    let params: Vec<Value> = vec![];
    let mut ctx = ExecContext {
        connection_id: 1,
        statement_id: None,
        sql: "SELECT 1".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test-ds",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Query,
    };
    BeforeFilter::before(&f, &mut ctx).await.unwrap();
}

/// 参数开关关闭：不发出参数日志。
#[tokio::test]
async fn log_filter_before_params_switch_off() {
    let f = LogFilter::new();
    f.set_statement_parameter_set_log_enabled(false);
    let params = vec![Value::Int(42)];
    let mut ctx = ExecContext {
        connection_id: 1,
        statement_id: None,
        sql: "SELECT ?".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test-ds",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Query,
    };
    BeforeFilter::before(&f, &mut ctx).await.unwrap();
}

// ── 13. ResultSetFilter 三个钩子 ───────────────────────────────

/// result_set_open_after：开关开时发出日志。
#[test]
fn log_filter_result_set_open_after_enabled() {
    let f = LogFilter::new();
    f.set_result_set_open_after_log_enabled(true);
    let ctx = ResultSetFilterContext::new();
    ResultSetFilter::result_set_open_after(&f, &ctx).unwrap();
}

/// result_set_open_after：开关关时静默。
#[test]
fn log_filter_result_set_open_after_disabled() {
    let f = LogFilter::new();
    f.set_result_set_open_after_log_enabled(false);
    let ctx = ResultSetFilterContext::new();
    ResultSetFilter::result_set_open_after(&f, &ctx).unwrap();
}

// ── PhysicalResultSet mocks ───────────────────────────────────

#[derive(Debug)]
struct StubResultSet;
impl PhysicalResultSet for StubResultSet {
    fn next(&self) -> Result<bool, DruidError> {
        Ok(true)
    }
    fn close(&self) -> Result<(), DruidError> {
        Ok(())
    }
    fn is_closed(&self) -> bool {
        false
    }
    fn meta_data(&self) -> Result<ResultSetMetaData, DruidError> {
        Ok(ResultSetMetaData::new(vec![]))
    }
}

#[derive(Debug)]
struct ErrorResultSet;
impl PhysicalResultSet for ErrorResultSet {
    fn next(&self) -> Result<bool, DruidError> {
        Err(DruidError::DriverError("rs next error".to_string()))
    }
    fn close(&self) -> Result<(), DruidError> {
        Err(DruidError::DriverError("close error".to_string()))
    }
    fn is_closed(&self) -> bool {
        false
    }
    fn meta_data(&self) -> Result<ResultSetMetaData, DruidError> {
        Ok(ResultSetMetaData::new(vec![]))
    }
}

#[derive(Debug)]
struct NoMoreRowsResultSet;
impl PhysicalResultSet for NoMoreRowsResultSet {
    fn next(&self) -> Result<bool, DruidError> {
        Ok(false)
    }
    fn close(&self) -> Result<(), DruidError> {
        Ok(())
    }
    fn is_closed(&self) -> bool {
        false
    }
    fn meta_data(&self) -> Result<ResultSetMetaData, DruidError> {
        Ok(ResultSetMetaData::new(vec![]))
    }
}

/// result_set_next：成功(true) + 开关开时发出日志。
#[test]
fn log_filter_result_set_next_success_enabled() {
    let f = LogFilter::new();
    f.set_result_set_next_after_log_enabled(true);
    let filters: Vec<Arc<dyn ResultSetFilter>> = vec![Arc::new(f)];
    let ctx = ResultSetFilterContext::new();
    let mut chain = ResultSetFilterChain::new(&filters, &StubResultSet, &ctx);
    assert!(chain.result_set_next().unwrap());
}

/// result_set_next：成功(true) + 开关关时静默。
#[test]
fn log_filter_result_set_next_success_disabled() {
    let f = LogFilter::new();
    f.set_result_set_next_after_log_enabled(false);
    let filters: Vec<Arc<dyn ResultSetFilter>> = vec![Arc::new(f)];
    let ctx = ResultSetFilterContext::new();
    let mut chain = ResultSetFilterChain::new(&filters, &StubResultSet, &ctx);
    assert!(chain.result_set_next().unwrap());
}

/// result_set_next：错误 + error 开关开时发出 error 日志。
#[test]
fn log_filter_result_set_next_error_enabled() {
    let f = LogFilter::new();
    f.set_result_set_log_error_enabled(true);
    let filters: Vec<Arc<dyn ResultSetFilter>> = vec![Arc::new(f)];
    let ctx = ResultSetFilterContext::new();
    let mut chain = ResultSetFilterChain::new(&filters, &ErrorResultSet, &ctx);
    assert!(chain.result_set_next().is_err());
}

/// result_set_next：错误 + error 开关关时静默传播错误。
#[test]
fn log_filter_result_set_next_error_disabled() {
    let f = LogFilter::new();
    f.set_result_set_log_error_enabled(false);
    let filters: Vec<Arc<dyn ResultSetFilter>> = vec![Arc::new(f)];
    let ctx = ResultSetFilterContext::new();
    let mut chain = ResultSetFilterChain::new(&filters, &ErrorResultSet, &ctx);
    assert!(chain.result_set_next().is_err());
}

/// result_set_close：成功 + 开关开时发出日志。
#[test]
fn log_filter_result_set_close_success_enabled() {
    let f = LogFilter::new();
    f.set_result_set_close_after_log_enabled(true);
    let filters: Vec<Arc<dyn ResultSetFilter>> = vec![Arc::new(f)];
    let ctx = ResultSetFilterContext::new();
    let mut chain = ResultSetFilterChain::new(&filters, &StubResultSet, &ctx);
    chain.result_set_close().unwrap();
}

/// result_set_close：成功 + 开关关时静默。
#[test]
fn log_filter_result_set_close_success_disabled() {
    let f = LogFilter::new();
    f.set_result_set_close_after_log_enabled(false);
    let filters: Vec<Arc<dyn ResultSetFilter>> = vec![Arc::new(f)];
    let ctx = ResultSetFilterContext::new();
    let mut chain = ResultSetFilterChain::new(&filters, &StubResultSet, &ctx);
    chain.result_set_close().unwrap();
}

/// result_set_close：错误 + error 开关开时发出 error 日志。
#[test]
fn log_filter_result_set_close_error_enabled() {
    let f = LogFilter::new();
    f.set_result_set_log_error_enabled(true);
    let filters: Vec<Arc<dyn ResultSetFilter>> = vec![Arc::new(f)];
    let ctx = ResultSetFilterContext::new();
    let mut chain = ResultSetFilterChain::new(&filters, &ErrorResultSet, &ctx);
    assert!(chain.result_set_close().is_err());
}

/// result_set_close：错误 + error 开关关时静默传播错误。
#[test]
fn log_filter_result_set_close_error_disabled() {
    let f = LogFilter::new();
    f.set_result_set_log_error_enabled(false);
    let filters: Vec<Arc<dyn ResultSetFilter>> = vec![Arc::new(f)];
    let ctx = ResultSetFilterContext::new();
    let mut chain = ResultSetFilterChain::new(&filters, &ErrorResultSet, &ctx);
    assert!(chain.result_set_close().is_err());
}

/// result_set_next 返回 false（无更多行）时不发出日志，即使开关开。
#[test]
fn log_filter_result_set_next_false_no_log() {
    let f = LogFilter::new();
    f.set_result_set_next_after_log_enabled(true);
    let filters: Vec<Arc<dyn ResultSetFilter>> = vec![Arc::new(f)];
    let ctx = ResultSetFilterContext::new();
    let mut chain = ResultSetFilterChain::new(&filters, &NoMoreRowsResultSet, &ctx);
    assert!(!chain.result_set_next().unwrap());
}

// ── 14. Default trait ──────────────────────────────────────────

#[test]
fn log_filter_default_trait_matches_new() {
    let f = LogFilter::default();
    assert!(f.is_connection_log_enabled());
    assert!(f.is_statement_log_enabled());
    assert!(f.is_result_set_log_enabled());
    assert!(!f.is_statement_executable_sql_log_enabled());
}

// ── 15. config_from_properties trait 路径 ──────────────────────

/// BeforeFilter::config_from_properties 委托给 LogFilter::config_from_properties。
#[test]
fn log_filter_before_filter_config_from_properties_trait_path() {
    let f = LogFilter::new();
    let p = props(&[("druid.log.conn", "false")]);
    BeforeFilter::config_from_properties(&f, &p).unwrap();
    assert!(!f.is_connection_log_enabled());
}

// ── 16. before_execute_error no-op 路径 ───────────────────────

/// before_execute_error 是 BeforeFilter 默认 no-op。
#[tokio::test]
async fn log_filter_before_execute_error_is_noop() {
    let f = LogFilter::new();
    let params: Vec<Value> = vec![];
    let ctx = ExecContext {
        connection_id: 1,
        statement_id: None,
        sql: "SELECT 1".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Execute,
    };
    let error = DruidError::DriverError("test".to_string());
    BeforeFilter::before_execute_error(&f, &ctx, &error)
        .await
        .unwrap();
}

// ── 17. 全开关开/关矩阵（组合覆盖） ──────────────────────────

/// 所有开关关闭后，所有异步 hook 均不 panic。
#[tokio::test]
async fn log_filter_all_switches_off_no_panic() {
    let f = LogFilter::new();
    f.set_data_source_log_enabled(false);
    f.set_connection_log_enabled(false);
    f.set_connection_log_error_enabled(false);
    f.set_connection_connect_before_log_enabled(false);
    f.set_connection_connect_after_log_enabled(false);
    f.set_connection_close_after_log_enabled(false);
    f.set_connection_commit_after_log_enabled(false);
    f.set_connection_rollback_after_log_enabled(false);
    f.set_statement_log_enabled(false);
    f.set_statement_log_error_enabled(false);
    f.set_statement_create_after_log_enabled(false);
    f.set_statement_prepare_after_log_enabled(false);
    f.set_statement_prepare_call_after_log_enabled(false);
    f.set_statement_execute_after_log_enabled(false);
    f.set_statement_execute_query_after_log_enabled(false);
    f.set_statement_execute_update_after_log_enabled(false);
    f.set_statement_execute_batch_after_log_enabled(false);
    f.set_statement_close_after_log_enabled(false);
    f.set_statement_parameter_set_log_enabled(false);
    f.set_statement_parameter_clear_log_enabled(false);
    f.set_statement_executable_sql_log_enabled(false);
    f.set_result_set_log_enabled(false);
    f.set_result_set_log_error_enabled(false);
    f.set_result_set_open_after_log_enabled(false);
    f.set_result_set_next_after_log_enabled(false);
    f.set_result_set_close_after_log_enabled(false);

    let params: Vec<Value> = vec![];
    let mut ctx = ExecContext {
        connection_id: 1,
        statement_id: None,
        sql: "SELECT 1".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Execute,
    };
    BeforeFilter::before(&f, &mut ctx).await.unwrap();
    AfterFilter::after(
        &f,
        &ctx,
        &Ok(ExecResult::default()),
        Duration::from_millis(1),
    )
    .await
    .unwrap();

    for event in [
        ConnectionEvent::Connect,
        ConnectionEvent::Commit,
        ConnectionEvent::Rollback,
        ConnectionEvent::Close,
        ConnectionEvent::SetAutoCommit(true),
    ] {
        AfterFilter::after_connection_event(&f, &event, Duration::ZERO)
            .await
            .unwrap();
    }

    for event in [
        StatementEvent::CreateStatement,
        StatementEvent::PrepareStatement("SELECT ?".to_string()),
        StatementEvent::PrepareCall("CALL".to_string()),
        StatementEvent::Close,
        StatementEvent::Execute("SELECT 1".to_string()),
    ] {
        BeforeFilter::on_statement_event(&f, &event).await.unwrap();
    }

    let rs_ctx = ResultSetFilterContext::new();
    ResultSetFilter::result_set_open_after(&f, &rs_ctx).unwrap();
}

// ── 18. name 方法 ─────────────────────────────────────────────

#[test]
fn log_filter_name_returns_log() {
    let f = LogFilter::new();
    assert_eq!(BeforeFilter::name(&f), "log");
    assert_eq!(AfterFilter::name(&f), "log");
}
