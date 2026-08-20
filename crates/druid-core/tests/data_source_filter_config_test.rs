//! Java `DruidAbstractDataSource` Filter 配置与生产装配语义测试。

extern crate druid_core as druid;
use druid::core::{
    AfterFilter, BeforeFilter, DruidError, ExecContext, ExecResult, FilterManager,
    PhysicalConnection, PhysicalConnectionFactory, ResultSetFilter,
};
use druid::pool::DruidPool;
use druid::toasty::ToastyConnectionFactory;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

const STAT_CLASS: &str = "com.alibaba.druid.filter.stat.StatFilter";
const ENCODING_CLASS: &str = "com.alibaba.druid.filter.encoding.EncodingConvertFilter";
const ERROR_CLASS: &str = "example.filter.ConstructorErrorFilter";

struct LifecycleProbeFilter {
    name: &'static str,
    init_count: Arc<AtomicUsize>,
    destroy_count: Arc<AtomicUsize>,
    execute_count: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl BeforeFilter for LifecycleProbeFilter {
    fn name(&self) -> &str {
        self.name
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        self.execute_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn init(&self) -> Result<(), DruidError> {
        self.init_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn destroy(&self) -> Result<(), DruidError> {
        self.destroy_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for LifecycleProbeFilter {
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
}

impl ResultSetFilter for LifecycleProbeFilter {}

async fn sqlite_factory() -> Arc<ToastyConnectionFactory> {
    Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .expect("必须创建真实 Toasty SQLite 工厂"),
    )
}

fn register_probe(
    manager: &FilterManager,
    class_name: &'static str,
    probe_name: &'static str,
    init_count: &Arc<AtomicUsize>,
    destroy_count: &Arc<AtomicUsize>,
    execute_count: &Arc<AtomicUsize>,
) {
    let init_count = Arc::clone(init_count);
    let destroy_count = Arc::clone(destroy_count);
    let execute_count = Arc::clone(execute_count);
    manager.register_filter(class_name, move || {
        Ok(LifecycleProbeFilter {
            name: probe_name,
            init_count: Arc::clone(&init_count),
            destroy_count: Arc::clone(&destroy_count),
            execute_count: Arc::clone(&execute_count),
        })
    });
}

#[tokio::test]
async fn set_filters_preserves_clear_disable_order_and_exactly_once_lifecycle() {
    let manager = Arc::new(FilterManager::new());
    let init_count = Arc::new(AtomicUsize::new(0));
    let destroy_count = Arc::new(AtomicUsize::new(0));
    let execute_count = Arc::new(AtomicUsize::new(0));
    register_probe(
        manager.as_ref(),
        STAT_CLASS,
        "stat-probe",
        &init_count,
        &destroy_count,
        &execute_count,
    );
    register_probe(
        manager.as_ref(),
        ENCODING_CLASS,
        "encoding-probe",
        &init_count,
        &destroy_count,
        &execute_count,
    );

    let factory = sqlite_factory().await;
    let mut builder = DruidPool::builder()
        .name("set-filters-sqlite")
        .driver_name("toasty-sqlite")
        .factory(factory as Arc<dyn PhysicalConnectionFactory>)
        .filter_manager(manager);
    builder.set_filters(None).unwrap();
    builder.set_filters(Some("")).unwrap();
    builder.set_filters(Some("encoding")).unwrap();
    builder.set_filters(Some("!stat")).unwrap();
    builder.set_clear_filters_enable(false);
    builder.set_filters(Some("!encoding")).unwrap();
    let pool = builder.build().await.unwrap();

    // SOURCE_PARITY / V2_MIRRORED：
    // 对应 ClearFilterTest#test_filters：`!stat` 先清空；关闭 clear 后，
    // `!encoding` 只剥离 `!` 并追加，最终顺序为 stat、encoding。
    let chain = pool.filter_chain().expect("Filter 配置必须进入生产池");
    assert_eq!(chain.filter_class_names(), [STAT_CLASS, ENCODING_CLASS]);
    assert_eq!(init_count.load(Ordering::SeqCst), 2);

    let mut connection = pool.get().await.unwrap();
    let mut statement = connection.create_statement().await.unwrap();
    statement
        .execute_update(
            &mut connection,
            "CREATE TABLE filter_config_item(id INTEGER PRIMARY KEY, name TEXT)",
        )
        .await
        .unwrap();
    statement
        .execute_update(
            &mut connection,
            "INSERT INTO filter_config_item(id, name) VALUES (1, '真实')",
        )
        .await
        .unwrap();
    let mut result_set = statement
        .execute_query_result_set(
            &mut connection,
            "SELECT name FROM filter_config_item WHERE id = 1",
        )
        .await
        .unwrap();
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(
        result_set.string(&mut connection, 1).unwrap().as_deref(),
        Some("真实")
    );
    result_set.close_with_connection(&mut connection).unwrap();
    statement.close_with_connection(&mut connection).unwrap();
    connection.close().await.unwrap();

    // RUST_OBLIGATION / V5_HOST：
    // 配置所得 Filter 必须进入真实 SQLite Statement/ResultSet 主链。
    assert!(execute_count.load(Ordering::SeqCst) >= 6);

    pool.close().await;
    pool.close().await;
    assert_eq!(destroy_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn add_and_clear_filters_use_java_trim_and_clear_enable_boundaries() {
    let manager = Arc::new(FilterManager::new());
    let init_count = Arc::new(AtomicUsize::new(0));
    let destroy_count = Arc::new(AtomicUsize::new(0));
    let execute_count = Arc::new(AtomicUsize::new(0));
    register_probe(
        manager.as_ref(),
        STAT_CLASS,
        "stat-probe",
        &init_count,
        &destroy_count,
        &execute_count,
    );

    let factory = sqlite_factory().await;
    let mut builder = DruidPool::builder()
        .name("java-trim")
        .driver_name("toasty-sqlite")
        .factory(factory as Arc<dyn PhysicalConnectionFactory>)
        .filter_manager(manager);
    builder
        .add_filters(Some(" \tstat\r\n,\u{00a0}stat\u{00a0},"))
        .unwrap();
    builder.set_clear_filters_enable(false);
    builder.clear_filters();
    builder.add_filters(None).unwrap();
    let pool = builder.build().await.unwrap();

    // VALUE_ADD / V1_RUST_LOCAL：
    // Java String#trim 仅去除 <= U+0020；NBSP 不能被 Rust `str::trim`
    // 意外删除。末尾空 token 与 null add 均保持 no-op。
    assert_eq!(
        pool.filter_chain().unwrap().filter_class_names(),
        [STAT_CLASS]
    );
    assert_eq!(init_count.load(Ordering::SeqCst), 1);

    pool.close().await;
    assert_eq!(destroy_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn builtin_stat_alias_is_the_default_sqlite_datasource_filter() {
    let factory = sqlite_factory().await;
    let mut builder = DruidPool::builder()
        .name("builtin-stat")
        .driver_name("toasty-sqlite")
        .factory(factory as Arc<dyn PhysicalConnectionFactory>);
    builder.set_filters(Some("default")).unwrap();
    builder.add_filters(Some("stat")).unwrap();
    let pool = builder.build().await.unwrap();

    // RUST_OBLIGATION / V1_RUST_LOCAL：
    // 未注入 FilterManager 时，Druid 默认装配必须能构造 bundled
    // default/stat 指向的同一个 StatFilter，并按类名去重。
    assert_eq!(
        pool.filter_chain().unwrap().filter_class_names(),
        [STAT_CLASS]
    );

    let mut connection = pool.get().await.unwrap();
    let mut statement = connection.create_statement().await.unwrap();
    statement
        .execute_update(
            &mut connection,
            "CREATE TABLE builtin_stat_item(id INTEGER PRIMARY KEY)",
        )
        .await
        .unwrap();
    statement.close_with_connection(&mut connection).unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn set_filters_reports_factory_error_immediately_and_keeps_prior_side_effects() {
    let manager = Arc::new(FilterManager::new());
    let init_count = Arc::new(AtomicUsize::new(0));
    let destroy_count = Arc::new(AtomicUsize::new(0));
    let execute_count = Arc::new(AtomicUsize::new(0));
    register_probe(
        manager.as_ref(),
        STAT_CLASS,
        "stat-probe",
        &init_count,
        &destroy_count,
        &execute_count,
    );
    manager.register_filter::<LifecycleProbeFilter, _>(ERROR_CLASS, || {
        Err(DruidError::DriverError("constructor failed".to_string()))
    });
    manager.register_alias("partial", format!("{STAT_CLASS},{ERROR_CLASS}"));

    let factory = sqlite_factory().await;
    let mut builder = DruidPool::builder()
        .name("filter-partial-error")
        .driver_name("toasty-sqlite")
        .factory(factory as Arc<dyn PhysicalConnectionFactory>)
        .filter_manager(manager);

    // SOURCE_PARITY / V2_MIRRORED：
    // Java addFilters 在循环中逐项装配；后项构造失败时 setFilters 立即抛错，
    // 此前已加入的 Filter 不回滚。
    assert_eq!(
        builder.set_filters(Some("partial")).unwrap_err(),
        DruidError::Other(
            "load managed rdbc driver event listener error. partial: \
             driver error: constructor failed"
                .to_string()
        )
    );

    let pool = builder.build().await.unwrap();
    assert_eq!(
        pool.filter_chain().unwrap().filter_class_names(),
        [STAT_CLASS]
    );
    assert_eq!(init_count.load(Ordering::SeqCst), 1);
    pool.close().await;
    assert_eq!(destroy_count.load(Ordering::SeqCst), 1);
}
