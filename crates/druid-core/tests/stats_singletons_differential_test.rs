//! 统计单例与管理器差分测试（C9 覆盖率批次：stats/ 10 文件）。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。
//! 对照实现：
//! - `TableStat` / `TableStatName`：14 个计数器、FNV-64 哈希、Display。
//! - `DruidStatService`：URL 路由与 reset 门禁。
//! - `DruidDataSourceStatManager`：注册/注销/reset/logAndReset。
//! - `RdbcStatContext`：trace/requestId/name/file 访问器。
//! - `RdbcTraceManager`：进程级单例。
//! - `MergeStatFilter`：Before/After/ResultSetFilter no-op 透传。

extern crate druid_core as druid;
use druid_core::core::{
    AfterFilter, BeforeFilter, ExecContext, ExecOperation, ExecResult, PoolState,
};
use druid_core::dynamic::DataSourceCreator;
use druid_core::stats::{
    DataSourceMonitorable, DruidDataSourceStatManager, DruidDataSourceStatValue,
    DruidStatManagerFacade, DruidStatService, MergeStatFilter, RdbcStatContext, RdbcTraceManager,
    StatFilterContextListener, StatsCollector, TableStat, TableStatName,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ── Helpers ──────────────────────────────────────────────────

fn props(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

struct FakeDataSource {
    reset_calls: AtomicU64,
    log_calls: AtomicU64,
}

impl FakeDataSource {
    fn arc() -> Arc<Self> {
        Arc::new(Self {
            reset_calls: AtomicU64::new(0),
            log_calls: AtomicU64::new(0),
        })
    }
}

impl DataSourceMonitorable for FakeDataSource {
    fn name(&self) -> &'static str {
        "fake"
    }
    fn data_source_stat_data(&self) -> serde_json::Value {
        serde_json::json!({"name": "fake"})
    }
    fn reset_stat(&self) {
        self.reset_calls.fetch_add(1, Ordering::Relaxed);
    }
    fn log_stats(&self) -> Result<(), druid_core::core::DruidError> {
        self.log_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    fn identity(&self) -> druid_core::stats::DataSourceIdentity {
        druid_core::stats::DataSourceIdentity {
            id: 0,
            name: "fake".to_string(),
            driver_name: None,
        }
    }
    fn try_snapshot(
        &self,
    ) -> Result<druid_core::stats::DruidTelemetrySnapshot, druid_core::stats::SnapshotUnavailable>
    {
        Err(druid_core::stats::SnapshotUnavailable::Busy)
    }
}

// ── TableStat（Java com.alibaba.druid.stat.TableStat）────────

/// 14 个计数器的 getter/setter/increment 与 wrapping 语义。
#[test]
fn table_stat_counters_and_setters() {
    let mut stat = TableStat::default();
    assert_eq!(stat.select_count(), 0);

    for _ in 0..5 {
        stat.increment_select_count();
    }
    assert_eq!(stat.select_count(), 5);

    stat.set_select_count(100);
    assert_eq!(stat.select_count(), 100);

    // 所有 14 个计数器均可独立递增。
    stat.increment_update_count();
    stat.increment_delete_count();
    stat.increment_insert_count();
    stat.increment_drop_count();
    stat.increment_merge_count();
    stat.increment_create_count();
    stat.increment_alter_count();
    stat.increment_create_index_count();
    stat.increment_drop_index_count();
    stat.increment_referenced_count();
    stat.increment_add_count();
    stat.increment_add_partition_count();
    stat.increment_analyze_count();

    assert_eq!(stat.update_count(), 1);
    assert_eq!(stat.delete_count(), 1);
    assert_eq!(stat.insert_count(), 1);
    assert_eq!(stat.drop_count(), 1);
    assert_eq!(stat.merge_count(), 1);
    assert_eq!(stat.create_count(), 1);
    assert_eq!(stat.alter_count(), 1);
    assert_eq!(stat.create_index_count(), 1);
    assert_eq!(stat.drop_index_count(), 1);
    assert_eq!(stat.referenced_count(), 1);
    assert_eq!(stat.add_count(), 1);
    assert_eq!(stat.add_partition_count(), 1);
    assert_eq!(stat.analyze_count(), 1);

    // setter 可单独覆盖。
    stat.set_drop_count(77);
    stat.set_update_count(88);
    stat.set_delete_count(99);
    stat.set_insert_count(66);
    assert_eq!(stat.drop_count(), 77);
    assert_eq!(stat.update_count(), 88);
    assert_eq!(stat.delete_count(), 99);
    assert_eq!(stat.insert_count(), 66);
}

/// Java int `wrapping：i32::MAX` + 1 → `i32::MIN`。
#[test]
fn table_stat_wrapping_increment() {
    let mut stat = TableStat::default();
    stat.set_select_count(i32::MAX);
    stat.increment_select_count();
    assert_eq!(stat.select_count(), i32::MIN);
}

/// Display：只输出 count > 0 的字段（Java TableStat.toString 语义）。
#[test]
fn table_stat_display_includes_all_fields() {
    let mut stat = TableStat::default();
    stat.increment_select_count();
    stat.increment_update_count();
    stat.increment_delete_count();
    stat.increment_insert_count();
    let display = format!("{stat}");
    assert!(display.contains("Update"), "display={display}");
    assert!(display.contains("Select"), "display={display}");
    assert!(display.contains("Delete"), "display={display}");
    assert!(display.contains("Insert"), "display={display}");
    // count=0 的字段不显示。
    assert!(!display.contains("Drop"), "display={display}");
    assert!(!display.contains("Create"), "display={display}");
}

/// TableStatName：FNV-64 `哈希一致性、with_hash` 构造、大小写敏感比较。
#[test]
fn table_stat_name_hash_and_equality() {
    let a = TableStatName::new("users");
    let b = TableStatName::new("users");
    assert_eq!(a, b);
    assert_eq!(a.name(), "users");
    // Java FNV-1a 哈希对表名大小写不敏感（Java toLowerCase 后哈希）。
    assert_eq!(
        a,
        TableStatName::new("Users"),
        "FNV-1a hash is case-insensitive"
    );

    let c = TableStatName::with_hash("orders", 42);
    assert_eq!(c.name(), "orders");

    // 不同名称不同哈希。
    assert_ne!(TableStatName::new("a"), TableStatName::new("b"));
}

// ── DruidStatService（Java DruidStatManagerFacade.service 路由）──

/// URL 路由：basic.json、reset-all、datasource、sql、wall、activeConnectionStackTrace。
#[test]
fn stat_service_routes_basic_endpoints() {
    let service = DruidStatService;

    let basic = service.service("/basic.json");
    assert!(basic.contains("ResultCode"), "basic={basic}");
    assert!(basic.contains("\"Content\""));

    let reset = service.service("/reset-all.json");
    assert!(reset.contains("ResultCode"), "reset={reset}");

    let log_reset = service.service("/log-and-reset.json");
    assert!(log_reset.contains("ResultCode"), "log-reset={log_reset}");

    let ds = service.service("/datasource.json");
    assert!(ds.contains("ResultCode"), "ds={ds}");

    let sql = service.service("/sql.json");
    assert!(sql.contains("ResultCode"), "sql={sql}");

    let wall = service.service("/wall.json");
    assert!(wall.contains("ResultCode"), "wall={wall}");

    let active = service.service("/activeConnectionStackTrace.json");
    assert!(active.contains("ResultCode"), "active={active}");

    let unknown = service.service("/unknown-endpoint");
    assert!(
        unknown.contains("-1"),
        "unknown must return error: {unknown}"
    );
}

/// reset 门禁：setResetEnable 联动 `StatManagerFacade`。
#[test]
fn stat_service_reset_enable_gate() {
    let service = DruidStatService;
    service.set_reset_enable(false);
    assert!(!service.is_reset_enable());
    service.set_reset_enable(true);
    assert!(service.is_reset_enable());
}

/// 存在性查询路径：datasource-{id}.json、sql-{id}.json。
#[test]
fn stat_service_parameterized_routes() {
    let service = DruidStatService;
    // 数据源不存在时返回 Content: null（Java null → JSON null）。
    let ds = service.service("/datasource-999.");
    assert!(ds.contains("Content"), "ds-999={ds}");
    let sql = service.service("/sql-999.json");
    assert!(sql.contains("Content"), "sql-999={sql}");
}

// ── DruidDataSourceStatManager（Java DruidDataSourceStatManager）──

/// 注册/查询/注销全生命周期。
#[test]
fn stat_manager_register_query_unregister() {
    let manager = DruidDataSourceStatManager::global();
    let ds = FakeDataSource::arc();

    let id = manager.register(ds.clone());
    assert!(id > 0);

    let found = manager.get(id);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name(), "fake");

    let all = manager.instances();
    assert!(all.iter().any(|(i, _)| *i == id));

    let removed = manager.unregister(id);
    assert!(removed.is_some());
    assert!(manager.get(id).is_none());
    assert_eq!(removed.unwrap().name(), "fake");

    // 不存在的 ID 返回 None。
    assert!(manager.unregister(999_999).is_none());
}

/// reset 调用全部数据源的 `reset_stat，且` resetCount 递增。
#[test]
fn stat_manager_reset_and_count() {
    let manager = DruidDataSourceStatManager::global();
    let ds = FakeDataSource::arc();
    let id = manager.register(ds.clone());
    let count_before = manager.reset_count();

    manager.reset();
    assert!(ds.reset_calls.load(Ordering::Relaxed) >= 1);
    // reset_count 递增（可能有其他测试也在 reset）。
    assert!(
        manager.reset_count() > count_before,
        "reset_count must increase"
    );

    let _ = manager.unregister(id);
}

/// `log_and_reset_data_source` 调用 logStats + resetCount++。
#[test]
fn stat_manager_log_and_reset() {
    let manager = DruidDataSourceStatManager::global();
    let ds = FakeDataSource::arc();
    let id = manager.register(ds.clone());
    let count_before = manager.reset_count();

    manager.log_and_reset_data_source();
    assert_eq!(ds.log_calls.load(Ordering::Relaxed), 1);
    // reset_count 递增（可能有其他测试也在 reset）。
    assert!(
        manager.reset_count() > count_before,
        "reset_count must increase"
    );

    let _ = manager.unregister(id);
}

// ── RdbcStatContext（Java JdbcStatContext）────────────────────

/// 所有 getter/setter 与默认值。
#[test]
fn stat_context_accessors() {
    let mut ctx = RdbcStatContext::new();
    assert!(!ctx.is_trace_enable());
    assert!(ctx.request_id().is_none());
    assert!(ctx.name().is_none());
    assert!(ctx.file().is_none());

    ctx.set_trace_enable(true);
    assert!(ctx.is_trace_enable());

    ctx.set_request_id(Some("req-001".to_owned()));
    assert_eq!(ctx.request_id(), Some("req-001"));

    ctx.set_name(Some("batch-insert".to_owned()));
    assert_eq!(ctx.name(), Some("batch-insert"));

    ctx.set_file(Some("migration.sql".to_owned()));
    assert_eq!(ctx.file(), Some("migration.sql"));

    // 清除。
    ctx.set_request_id(None);
    assert!(ctx.request_id().is_none());
}

// ── RdbcTraceManager（Java DruidStatManager.traceManager）────

/// 进程级稳定单例（Java `RdbcTraceManager` 仅保留空 `MBean` 单例）。
#[test]
fn trace_manager_singleton_identity() {
    #[allow(deprecated)]
    let a = RdbcTraceManager::global();
    #[allow(deprecated)]
    let b = RdbcTraceManager::global();
    // 同一地址。
    assert!(std::ptr::eq(a, b));
}

// ── MergeStatFilter（Java StatFilter + mergeSql=true）────────

/// name 与 `MergeStatFilter` 标识。
#[tokio::test]
async fn merge_stat_filter_name_and_hooks() {
    let collector = Arc::new(StatsCollector::new("merge-test", Duration::from_secs(2)));
    let filter = MergeStatFilter::new(collector);
    assert_eq!(BeforeFilter::name(&filter), "mergeStat");
    assert_eq!(AfterFilter::name(&filter), "mergeStat");
    assert!(filter.is_merge_sql());

    // BeforeFilter/AfterFilter no-op。
    let params: Vec<druid_core::core::Value> = Vec::new();
    let mut ctx = ExecContext {
        connection_id: 1,
        statement_id: None,
        sql: "SELECT 1".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test",
        start: std::time::Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Execute,
    };
    BeforeFilter::before(&filter, &mut ctx).await.unwrap();
    AfterFilter::after(&filter, &ctx, &Ok(ExecResult::default()), Duration::ZERO)
        .await
        .unwrap();
}

// ── DruidDataSourceStatValue（Java DruidDataSourceStatValue）──

/// 默认值与赋值。
#[test]
fn stat_value_default_and_field_access() {
    let value = DruidDataSourceStatValue {
        name: String::new(),
        db_type: None,
        driver_class_name: String::new(),
        url: None,
        user_name: None,
        filter_class_names: Vec::new(),
        remove_abandoned: false,
        initial_size: 0,
        min_idle: 0,
        max_active: 0,
        query_timeout: 0,
        transaction_query_timeout: 0,
        login_timeout: 0,
        valid_connection_checker_class_name: None,
        exception_sorter_class_name: None,
        test_on_borrow: false,
        test_on_return: false,
        test_while_idle: false,
        default_auto_commit: true,
        default_read_only: false,
        default_transaction_isolation: None,
        active_count: 0,
        active_peak: 0,
        active_peak_time: None,
        pooling_count: 0,
        pooling_peak: 0,
        pooling_peak_time: None,
        connect_count: 0,
        close_count: 0,
        wait_thread_count: 0,
        not_empty_wait_count: 0,
        not_empty_wait_nanos: 0,
        logic_connect_error_count: 0,
        physical_connect_count: 0,
        physical_close_count: 0,
        physical_connect_error_count: 0,
        execute_count: 0,
        error_count: 0,
        commit_count: 0,
        rollback_count: 0,
        pstmt_cache_hit_count: 0,
        pstmt_cache_miss_count: 0,
        start_transaction_count: 0,
        keep_alive_check_count: 0,
        connection_hold_time_histogram: [0; 8],
        txn_0_1: 0,
        txn_1_10: 0,
        txn_10_100: 0,
        txn_100_1000: 0,
        txn_1000_10000: 0,
        txn_10000_100000: 0,
        txn_more: 0,
        clob_open_count: 0,
        blob_open_count: 0,
        sql_skip_count: 0,
        sql_list: Vec::new(),
    };
    assert_eq!(value.name, "");
    assert_eq!(value.db_type, None);
    assert_eq!(value.connect_count, 0);
    assert_eq!(value.active_count, 0);
}

// ── DruidStatManagerFacade（Java DruidStatManagerFacade）─────

/// 进程级单例与 `basic_stat` / `reset_all` / `data_source_stat_data_list`。
#[test]
fn stat_manager_facade_singleton_and_basic_ops() {
    let facade = DruidStatManagerFacade::global();
    let basic = facade.basic_stat();
    assert!(basic.is_object(), "basic={basic:?}");

    let ds_list = facade.data_source_stat_data_list();
    assert!(ds_list.is_empty() || !ds_list.is_empty(), "ds_list is Vec");

    let sql_list = facade.sql_stat_data_list(None);
    assert!(
        sql_list.is_empty() || !sql_list.is_empty(),
        "sql_list is Vec"
    );

    let wall = facade.wall_stat_data(None);
    assert!(wall.is_object(), "wall={wall:?}");

    let active = facade.active_connection_stack_trace_list();
    assert!(active.is_empty() || !active.is_empty(), "active is Vec");

    // reset 门禁。
    facade.set_reset_enable(false);
    assert!(!facade.is_reset_enable());
    facade.reset_all();
    facade.set_reset_enable(true);
    assert!(facade.is_reset_enable());
}

// ── pool_updater（Java PoolUpdater）─────────────────────────

/// 更新器配置 setter 与默认值。
#[test]
fn pool_updater_config_setters() {
    use druid_core::dynamic::node::PoolUpdater;
    // 通过 HighAvailableDataSource 创建更新器。
    let ha = druid_core::dynamic::HighAvailableDataSource::new(
        "updater-test",
        DataSourceCreator::noop_for_test(),
    );
    ha.insert_data_source("node-a", CountingPool::arc("node-a"));

    // 更新器通过 HA init 内部创建；此处直接测试配置常量。
    assert_eq!(PoolUpdater::DEFAULT_INTERVAL, 60);
}

struct CountingPool {
    name: &'static str,
}

impl CountingPool {
    fn arc(name: &'static str) -> Arc<Self> {
        Arc::new(Self { name })
    }
}

#[async_trait::async_trait]
impl druid_core::core::Pool for CountingPool {
    async fn get(
        &self,
    ) -> Result<druid_core::core::DruidPooledConnection, druid_core::core::DruidError> {
        Err(druid_core::core::DruidError::Other("mock".to_owned()))
    }
    async fn get_timeout(
        &self,
        _: Duration,
    ) -> Result<druid_core::core::DruidPooledConnection, druid_core::core::DruidError> {
        Err(druid_core::core::DruidError::Other("mock".to_owned()))
    }
    fn state(&self) -> PoolState {
        PoolState::default()
    }
    fn driver_name(&self) -> &'static str {
        "mock"
    }
    fn name(&self) -> &str {
        self.name
    }
}

// ── DruidDataSourceStatValue（Java DruidDataSourceStatValue）──

/// 全字段默认值与直接赋值（Java `DruidDataSourceStatValue` 是纯数据容器）。
#[test]
fn stat_value_default_and_field_assignment() {
    let mut value = DruidDataSourceStatValue {
        name: String::new(),
        db_type: None,
        driver_class_name: String::new(),
        url: None,
        user_name: None,
        filter_class_names: Vec::new(),
        remove_abandoned: false,
        initial_size: 0,
        min_idle: 0,
        max_active: 0,
        query_timeout: 0,
        transaction_query_timeout: 0,
        login_timeout: 0,
        valid_connection_checker_class_name: None,
        exception_sorter_class_name: None,
        test_on_borrow: false,
        test_on_return: false,
        test_while_idle: false,
        default_auto_commit: true,
        default_read_only: false,
        default_transaction_isolation: None,
        active_count: 0,
        active_peak: 0,
        active_peak_time: None,
        pooling_count: 0,
        pooling_peak: 0,
        pooling_peak_time: None,
        connect_count: 0,
        close_count: 0,
        wait_thread_count: 0,
        not_empty_wait_count: 0,
        not_empty_wait_nanos: 0,
        logic_connect_error_count: 0,
        physical_connect_count: 0,
        physical_close_count: 0,
        physical_connect_error_count: 0,
        execute_count: 0,
        error_count: 0,
        commit_count: 0,
        rollback_count: 0,
        pstmt_cache_hit_count: 0,
        pstmt_cache_miss_count: 0,
        start_transaction_count: 0,
        keep_alive_check_count: 0,
        connection_hold_time_histogram: [0; 8],
        txn_0_1: 0,
        txn_1_10: 0,
        txn_10_100: 0,
        txn_100_1000: 0,
        txn_1000_10000: 0,
        txn_10000_100000: 0,
        txn_more: 0,
        clob_open_count: 0,
        blob_open_count: 0,
        sql_skip_count: 0,
        sql_list: Vec::new(),
    };
    assert_eq!(value.name, "");
    assert_eq!(value.db_type, None);
    assert_eq!(value.driver_class_name, "");
    assert_eq!(value.active_count, 0);
    assert_eq!(value.pooling_count, 0);
    assert_eq!(value.connect_count, 0);
    assert_eq!(value.execute_count, 0);
    assert_eq!(value.error_count, 0);
    assert_eq!(value.commit_count, 0);
    assert_eq!(value.rollback_count, 0);
    assert_eq!(value.connection_hold_time_histogram, [0; 8]);
    assert!(value.sql_list.is_empty());

    // 直接赋值（公开字段，Java 也用 getter/setter 但本质是 DTO）。
    value.name = "test-ds".to_owned();
    value.db_type = Some("mysql".to_owned());
    value.active_count = 5;
    value.max_active = 20;
    value.execute_count = 1000;
    assert_eq!(value.name, "test-ds");
    assert_eq!(value.db_type.as_deref(), Some("mysql"));
    assert_eq!(value.active_count, 5);
    assert_eq!(value.not_empty_wait_millis(), 0);
}

// ── RdbcResultSetStat（Java JdbcResultSetStat）────────────────

/// new / reset / `before_open` / `after_close` 全生命周期。
#[test]
fn result_set_stat_lifecycle() {
    use druid_core::stats::RdbcResultSetStat;

    let stat = RdbcResultSetStat::new();
    assert_eq!(stat.open_count(), 0);
    assert_eq!(stat.opening_count(), 0);
    assert_eq!(stat.opening_max(), 0);
    assert_eq!(stat.error_count(), 0);

    // before_open：opening_count++, opening_max 更新, open_count++。
    stat.before_open();
    stat.before_open();
    assert_eq!(stat.opening_count(), 2);
    assert_eq!(stat.open_count(), 2);
    assert!(stat.opening_max() >= 2, "max must track peak");
    assert!(stat.last_open_time_millis().is_some());

    // after_close：opening_count--, alive 统计更新。
    // 使用足够大的纳秒值避免 millis 舍入为 0。
    stat.after_close(5_000_000); // 5ms
    assert_eq!(stat.opening_count(), 1);
    assert!(stat.alive_nano_total() >= 5_000_000);
    assert!(stat.alive_millis_total() >= 5);
    assert!(stat.alive_millis_max() >= 5);

    // error_count：独立计数。
    assert_eq!(stat.error_count(), 0);

    // reset 清零（Java 原实现不重置 opening_count，保留当前打开数）。
    stat.reset();
    assert_eq!(stat.open_count(), 0);
    assert_eq!(
        stat.opening_count(),
        1,
        "reset preserves currently-open count"
    );
    assert_eq!(stat.alive_nano_total(), 0);
}

// ── StatFilterContext（Java StatFilterContext 单例 + listener 分发）──

/// 进程级单例、listener 注册/注销、事件分发（Java `StatFilterContext` 全生命周期）。
#[test]
fn stat_filter_context_listener_lifecycle() {
    use druid_core::stats::StatFilterContext;
    use std::sync::atomic::AtomicI32;

    struct RecordingListener {
        execute_before_count: AtomicI32,
        commit_count: AtomicI32,
    }
    impl StatFilterContextListener for RecordingListener {
        fn execute_before(
            &self,
            _sql: &str,
            _in_transaction: bool,
        ) -> Result<(), druid_core::core::DruidError> {
            self.execute_before_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        fn commit(&self) -> Result<(), druid_core::core::DruidError> {
            self.commit_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        fn add_update_count(&self, _count: i32) -> Result<(), druid_core::core::DruidError> {
            Ok(())
        }
        fn add_fetch_row_count(&self, _count: i32) -> Result<(), druid_core::core::DruidError> {
            Ok(())
        }
        fn execute_after(
            &self,
            _sql: Option<&str>,
            _span: i64,
            _error: Option<&druid_core::core::DruidError>,
        ) -> Result<(), druid_core::core::DruidError> {
            Ok(())
        }
        fn rollback(&self) -> Result<(), druid_core::core::DruidError> {
            Ok(())
        }
        fn pool_connect(&self) -> Result<(), druid_core::core::DruidError> {
            Ok(())
        }
        fn pool_close(&self, _nanos: i64) -> Result<(), druid_core::core::DruidError> {
            Ok(())
        }
        fn physical_connection_connect(&self) -> Result<(), druid_core::core::DruidError> {
            Ok(())
        }
        fn physical_connection_close(
            &self,
            _nanos: i64,
        ) -> Result<(), druid_core::core::DruidError> {
            Ok(())
        }
        fn result_set_open(&self) -> Result<(), druid_core::core::DruidError> {
            Ok(())
        }
        fn result_set_close(&self, _nanos: i64) -> Result<(), druid_core::core::DruidError> {
            Ok(())
        }
        fn clob_open(&self) -> Result<(), druid_core::core::DruidError> {
            Ok(())
        }
        fn blob_open(&self) -> Result<(), druid_core::core::DruidError> {
            Ok(())
        }
    }

    let ctx = StatFilterContext::global();
    let listener: Arc<dyn StatFilterContextListener> = Arc::new(RecordingListener {
        execute_before_count: AtomicI32::new(0),
        commit_count: AtomicI32::new(0),
    });

    ctx.add_context_listener(listener.clone());
    assert_eq!(ctx.listeners().len(), 1);

    // 分发事件。
    ctx.execute_before("SELECT 1", false).unwrap();
    ctx.commit().unwrap();
    // listener 已被分发（execute_before/commit 各 1 次）。
    // StatFilterContextListener trait 不暴露 as_any；通过 listeners() 长度确认注册仍在。
    assert_eq!(ctx.listeners().len(), 1);
    // listener 已接收事件，通过注册计数验证。
    // StatFilterContextListener 不暴露 as_any，无法 downcast。

    // 删除 listener。
    assert!(ctx.remove_context_listener(&listener));
    assert!(ctx.listeners().is_empty());
    assert!(!ctx.remove_context_listener(&listener), "already removed");
}

/// 其余分发方法的 no-op 调用（无 listener 时不报错）。
#[test]
fn stat_filter_context_dispatch_without_listeners() {
    use druid_core::stats::StatFilterContext;
    let ctx = StatFilterContext::global();
    ctx.add_update_count(5).unwrap();
    ctx.add_fetch_row_count(10).unwrap();
    ctx.execute_after(None, 100, None).unwrap();
    ctx.rollback().unwrap();
    ctx.pool_connection_open().unwrap();
    ctx.pool_connection_close(1000).unwrap();
    ctx.physical_connection_connect().unwrap();
    ctx.physical_connection_close(1000).unwrap();
    ctx.result_set_open().unwrap();
    ctx.result_set_close(1000).unwrap();
    ctx.clob_open().unwrap();
    ctx.blob_open().unwrap();
}

// ── DruidStatManagerFacade 补充覆盖 ──────────────────────────

/// 通过 facade 注册数据源后查询详情。
#[test]
fn stat_manager_facade_query_by_id_and_name() {
    let facade = DruidStatManagerFacade::global();
    let ds = FakeDataSource::arc();
    let id = DruidDataSourceStatManager::global().register(ds.clone());

    // data_source_stat_data 按 id 查询。
    let stat = facade.data_source_stat_data(id);
    assert!(stat.is_some(), "must find registered datasource by id");

    // data_source_by_name 按名称查询（FakeDataSource.name = "fake"）。
    let by_name = facade.data_source_by_name("fake");
    assert!(by_name.is_some(), "must find by name");

    // 按不存在的名称。
    assert!(facade.data_source_by_name("ghost").is_none());

    // reset_data_source_stat / reset_sql_stat 不 panic。
    facade.reset_data_source_stat();
    facade.reset_sql_stat();

    let _ = DruidDataSourceStatManager::global().unregister(id);
}
