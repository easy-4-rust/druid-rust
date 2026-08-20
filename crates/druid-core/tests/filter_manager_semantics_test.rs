//! Java `FilterManager` 别名、工厂、去重与真实 Toasty SQLite 验证。

extern crate druid_core as druid;
use druid::core::{
    AfterFilter, BeforeFilter, DruidError, DruidPooledConnection, ExecContext, ExecResult,
    FilterChain, FilterManager, PhysicalConnectionFactory, ResultSetFilter,
};
use druid::stats::{StatFilter, StatsCollector};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

const PROBE_CLASS: &str = "example.filter.ProbeFilter";
const STAT_CLASS: &str = "com.alibaba.druid.filter.stat.StatFilter";

struct ProbeFilter;

#[async_trait::async_trait]
impl BeforeFilter for ProbeFilter {
    fn name(&self) -> &str {
        "probe"
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for ProbeFilter {
    fn name(&self) -> &str {
        "probe"
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

impl ResultSetFilter for ProbeFilter {}

#[test]
fn bundled_aliases_and_java_utf16_fallback_match_filter_manager_contract() {
    let manager = FilterManager::new();

    // SOURCE_PARITY / V2_MIRRORED：
    // 对应 FilterManagerTest 静态块与 DruidLoaderUtilsTest 的别名断言。
    assert_eq!(
        manager.get_filter(Some("stat")).as_deref(),
        Some(STAT_CLASS)
    );
    assert_eq!(
        manager.get_filter(Some("default")).as_deref(),
        Some(STAT_CLASS)
    );
    assert_eq!(
        manager.get_filter(Some("log")).as_deref(),
        Some("druid::core::LogFilter")
    );
    // Java 日志框架名不是 Rust Filter alias；未知短名称仍遵循 FilterManager
    // 的直接标识符回退规则。
    assert_eq!(
        manager.get_filter(Some("commonLogging")).as_deref(),
        Some("commonLogging")
    );
    assert_eq!(manager.get_filter(Some("Stat")).as_deref(), Some("Stat"));
    assert_eq!(manager.get_filter(None), None);

    // RUST_OBLIGATION / V1_RUST_LOCAL：
    // Java String#length 统计 UTF-16 code unit，不能错误使用 UTF-8 byte 长度。
    let short_non_bmp = "😀".repeat(63);
    assert_eq!(
        manager.get_filter(Some(&short_non_bmp)).as_deref(),
        Some(short_non_bmp.as_str())
    );
    let boundary_non_bmp = "😀".repeat(64);
    assert_eq!(manager.get_filter(Some(&boundary_non_bmp)), None);
}

#[test]
fn ordered_property_sources_preserve_override_escape_and_continuation_semantics() {
    let manager = FilterManager::from_property_sources([
        "druid.filters.custom=first.Filter\nignored=value\n",
        "druid.filters.custom=second.Filter\\\n  ,third.Filter\n\
         druid.filters.uni\\u0063ode=emoji.\\uD83D\\uDE00\n",
    ])
    .unwrap();

    // VALUE_ADD / V1_RUST_LOCAL：
    // 后加载资源覆盖、续行去除前导空白、key/value Unicode escape 必须与
    // Properties#load + putAll 一致；否则多 classloader 配置会产生错误顺序。
    assert_eq!(
        manager.get_filter(Some("custom")).as_deref(),
        Some("second.Filter,third.Filter")
    );
    assert_eq!(
        manager.get_filter(Some("unicode")).as_deref(),
        Some("emoji.😀")
    );
    assert!(matches!(
        FilterManager::from_property_sources(["druid.filters.bad=\\u12\n"]),
        Err(DruidError::InvalidArgument(message))
            if message == "malformed Unicode escape in filter properties"
    ));
    assert!(matches!(
        FilterManager::from_property_sources(["druid.filters.bad=\\uZZZZ\n"]),
        Err(DruidError::InvalidArgument(message))
            if message == "malformed Unicode escape in filter properties"
    ));
}

#[test]
fn java_properties_separators_comments_and_escape_families_remain_observable() {
    let manager = FilterManager::from_property_sources(["  # comment\n\
         ! another comment\n\
         \n\
         druid.filters.whitespace : line\\twith\\ncontrols\\rreturn\\fform\n\
         druid.filters.escaped\\ key=escaped\\=value\\:tail\\\\slash\n\
         druid.filters.empty\n\
         druid.filters.trailing=tail\\"])
    .unwrap();

    // RUST_OBLIGATION / V1_RUST_LOCAL：
    // Rust 内置解析器替代 java.util.Properties，必须覆盖 Filter 资源可能使用的
    // 分隔符、注释、空值和全部标准字符 escape，而不只支持当前 bundled 文件。
    assert_eq!(
        manager.get_filter(Some("whitespace")).as_deref(),
        Some("line\twith\ncontrols\rreturn\u{000C}form")
    );
    assert_eq!(
        manager.get_filter(Some("escaped key")).as_deref(),
        Some("escaped=value:tail\\slash")
    );
    assert_eq!(manager.get_filter(Some("empty")).as_deref(), Some(""));
    assert_eq!(
        manager.get_filter(Some("trailing")).as_deref(),
        Some("tail")
    );
}

#[test]
fn explicit_factories_preserve_alias_expansion_case_insensitive_dedup_and_failure() {
    let manager = FilterManager::new();
    let created = Arc::new(AtomicUsize::new(0));
    let created_by_factory = Arc::clone(&created);
    manager.register_filter(PROBE_CLASS, move || {
        created_by_factory.fetch_add(1, Ordering::SeqCst);
        Ok(ProbeFilter)
    });
    manager.register_alias("pair", format!("{PROBE_CLASS},EXAMPLE.FILTER.PROBEFILTER"));

    let mut chain = FilterChain::new();
    manager.load_filter(&mut chain, "pair").unwrap();

    // SOURCE_PARITY / V2_MIRRORED：
    // Java existsFilter 使用 equalsIgnoreCase；第二个大小写变体不得再次构造。
    assert_eq!(created.load(Ordering::SeqCst), 1);
    assert_eq!(chain.filter_class_names(), [PROBE_CLASS]);
    assert_eq!(chain.before_count(), 1);
    assert_eq!(chain.after_count(), 1);
    assert_eq!(chain.result_set_count(), 1);

    manager.load_filter(&mut chain, "").unwrap();
    manager.load_filter(&mut chain, PROBE_CLASS).unwrap();
    assert_eq!(created.load(Ordering::SeqCst), 1);

    // Java 对找不到的类只记录错误并继续，不能插入占位 Filter。
    manager.load_filter(&mut chain, "missing.Filter").unwrap();
    assert_eq!(chain.filter_class_names(), [PROBE_CLASS]);

    manager.register_alias("untrimmed", format!(" {PROBE_CLASS}"));
    manager.load_filter(&mut chain, "untrimmed").unwrap();
    assert_eq!(chain.filter_class_names(), [PROBE_CLASS]);

    // Java getFilter 对 UTF-16 长度 >=128 的名称返回 null，随后进入直接类加载分支。
    let long_class_name = "L".repeat(128);
    manager.register_filter(&long_class_name, || Ok(ProbeFilter));
    manager
        .load_filter(&mut chain, long_class_name.as_str())
        .unwrap();
    manager
        .load_filter(&mut chain, long_class_name.as_str())
        .unwrap();
    assert_eq!(
        chain.filter_class_names(),
        [PROBE_CLASS, long_class_name.as_str()]
    );

    manager.register_filter::<ProbeFilter, _>("example.filter.ErrorFilter", || {
        Err(DruidError::DriverError("constructor failed".to_string()))
    });
    assert_eq!(
        manager
            .load_filter(&mut chain, "example.filter.ErrorFilter")
            .unwrap_err(),
        DruidError::Other(
            "load managed rdbc driver event listener error. \
             example.filter.ErrorFilter: driver error: constructor failed"
                .to_string()
        )
    );

    // Default 仍读取同一 bundled alias 表；不是另一个空注册表实现。
    assert_eq!(
        FilterManager::default().get_filter(Some("stat")).as_deref(),
        Some(STAT_CLASS)
    );
}

#[tokio::test]
async fn real_toasty_sqlite_uses_filter_loaded_from_builtin_stat_alias() {
    let collector = Arc::new(StatsCollector::new(
        "filter-manager-sqlite",
        Duration::from_secs(60),
    ));
    let manager = FilterManager::new();
    let factory_collector = Arc::clone(&collector);
    manager.register_filter(STAT_CLASS, move || {
        Ok(StatFilter::new(Arc::clone(&factory_collector)))
    });

    let mut chain = FilterChain::new();
    manager.load_filter(&mut chain, "stat").unwrap();
    manager.load_filter(&mut chain, "default").unwrap();
    assert_eq!(chain.filter_class_names(), [STAT_CLASS]);

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 Toasty SQLite 物理连接");
    let mut connection = DruidPooledConnection::with_context(
        physical,
        102,
        "filter-manager-sqlite".to_string(),
        Some(Arc::new(chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    statement
        .execute_update(
            &mut connection,
            "CREATE TABLE managed_item(id INTEGER PRIMARY KEY, name TEXT)",
        )
        .await
        .unwrap();
    statement
        .execute_update(
            &mut connection,
            "INSERT INTO managed_item(id, name) VALUES (1, '一号')",
        )
        .await
        .unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT name FROM managed_item ORDER BY id")
        .await
        .unwrap();
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(
        result_set.string(&mut connection, 1).unwrap(),
        Some("一号".to_string())
    );
    result_set.close_with_connection(&mut connection).unwrap();

    // VALUE_ADD / V5_HOST：
    // 别名解析得到的工厂实例必须真正进入池化 Statement/ResultSet 生产链。
    assert_eq!(collector.result_set_stat().open_count(), 1);
    assert_eq!(collector.result_set_stat().fetch_row_count(), 1);
    assert_eq!(collector.result_set_stat().close_count(), 1);
}
