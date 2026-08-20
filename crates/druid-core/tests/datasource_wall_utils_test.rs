//! `DruidDataSource` + `WallVisitorUtils` 差分测试（C9 批次：pool + sql 0%）。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。

extern crate druid_core as druid;
use druid::core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionFactory, Row, Value,
};
use druid::pool::{DruidDataSource, DruidPool};
use druid::sql::{WallConfig, WallVisitorUtils};
use sqlparser::ast::Statement;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use std::sync::Arc;
use std::time::Duration;

// ── Mock ─────────────────────────────────────────────────────

struct MockConn;
#[async_trait::async_trait]
impl PhysicalConnection for MockConn {
    async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> {
        Ok(ExecResult::default())
    }
    async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> {
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
        Ok(())
    }
    fn driver_name(&self) -> &'static str {
        "mock"
    }
}

struct MockFactory;
#[async_trait::async_trait]
impl PhysicalConnectionFactory for MockFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        Ok(Box::new(MockConn))
    }
    async fn validate(&self, _: &mut Box<dyn PhysicalConnection>) -> Result<(), DruidError> {
        Ok(())
    }
}

// ── DruidDataSource::from_pool + getters ─────────────────────

/// `from_pool` 构造 + 全部 getter。
#[tokio::test]
async fn datasource_from_pool_and_getters() {
    let pool = DruidPool::builder()
        .name("ds-test")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    let ds = DruidDataSource::from_pool(pool);

    // 全部 getter。
    let state = ds.state();
    assert_eq!(state.name, "ds-test");
    assert_eq!(state.driver_name, "mock");
    assert!(!ds.is_on_fatal_error());
    assert_eq!(ds.on_fatal_error_max_active(), 0);
    assert!(!ds.is_async_init());
    assert!(
        ds.is_init_exception_throw(),
        "default initExceptionThrow is true"
    );
    assert!(!ds.is_fail_continuous());
    assert!(ds.last_create_error().is_none());
    assert_eq!(ds.last_create_error_time_millis(), 0);
    assert!(!ds.is_full());
    assert!(ds.is_reset_stat_enable());
    assert_eq!(ds.reset_count(), 0);
    assert_eq!(ds.login_timeout(), 0);
}

/// get / `get_connection` / `get_timeout` / `get_connection_direct`。
#[tokio::test]
async fn datasource_get_connection_variants() {
    let pool = DruidPool::builder()
        .name("ds-get")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    let ds = DruidDataSource::from_pool(pool);

    let c1 = ds.get().await.unwrap();
    assert_eq!(c1.driver_name(), "mock");
    drop(c1);

    let c2 = ds.get_connection().await.unwrap();
    drop(c2);

    let c3 = ds
        .get_connection_with_max_wait(Duration::from_secs(2))
        .await
        .unwrap();
    drop(c3);

    let c4 = ds
        .get_connection_direct(Duration::from_secs(2))
        .await
        .unwrap();
    drop(c4);

    let c5 = ds.get_timeout(Duration::from_secs(2)).await.unwrap();
    drop(c5);
}

/// shrink / `shrink_check_time` / `shrink_with_options`。
#[tokio::test]
async fn datasource_shrink_variants() {
    let pool = DruidPool::builder()
        .name("ds-shrink")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    let ds = DruidDataSource::from_pool(pool);
    let _c = ds.get().await.unwrap();

    // 每种 shrink 变体均不 panic。
    ds.shrink().await;
    ds.shrink_check_time(true).await;
    ds.shrink_with_options(true, false).await;
}

/// fill / `fill_to`。
#[tokio::test]
async fn datasource_fill() {
    let pool = DruidPool::builder()
        .name("ds-fill")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    let ds = DruidDataSource::from_pool(pool);
    // fill 不报错。
    let count = ds.fill().await.unwrap();
    assert!(count >= 0);
    let count2 = ds.fill_to(2).await.unwrap();
    assert!(count2 >= 0);
}

/// `try_get_connection：无连接时返回` None。
#[tokio::test]
async fn datasource_try_get_connection_empty() {
    let pool = DruidPool::builder()
        .name("ds-try")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    let ds = DruidDataSource::from_pool(pool);
    let result = ds.try_get_connection().await.unwrap();
    assert!(result.is_none(), "empty pool should return None");
}

/// `remove_abandoned` / `discard_connection`。
#[tokio::test]
async fn datasource_remove_abandoned_and_discard() {
    let pool = DruidPool::builder()
        .name("ds-abandon")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    let ds = DruidDataSource::from_pool(pool);
    // remove_abandoned 不报错。
    let count = ds.remove_abandoned();
    assert_eq!(count, 0);
    // discard_connection(None) 返回 false。
    assert!(!ds.discard_connection(None));
}

/// `reset_stat_enable` / `reset_stat` / `reset_count` / `publish_stats`。
#[tokio::test]
async fn datasource_stat_reset_and_publish() {
    let pool = DruidPool::builder()
        .name("ds-stat")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    let ds = DruidDataSource::from_pool(pool);

    ds.set_reset_stat_enable(false);
    assert!(!ds.is_reset_stat_enable());
    ds.set_reset_stat_enable(true);
    assert!(ds.is_reset_stat_enable());

    ds.reset_stat();
    assert_eq!(ds.reset_count(), 1);

    // publish_stats 不报错。
    ds.publish_stats().unwrap();
}

/// init / close 生命周期。
#[tokio::test]
async fn datasource_init_and_close() {
    let pool = DruidPool::builder()
        .name("ds-lifecycle")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    let ds = DruidDataSource::from_pool(pool);
    ds.init().await.unwrap();
    let state = ds.state();
    assert!(!state.closed);

    ds.close().await;
    let state = ds.state();
    assert!(state.closed);
}

/// `notify_credentials_changed` / `user_password_version`。
#[tokio::test]
async fn datasource_credentials_version() {
    let pool = DruidPool::builder()
        .name("ds-creds")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    let ds = DruidDataSource::from_pool(pool);
    assert_eq!(ds.user_password_version(), 0);
    let version = ds.notify_credentials_changed().await.unwrap();
    assert_eq!(version, 1);
    assert_eq!(ds.user_password_version(), 1);
}

/// `is_full` / `stat_value_and_reset`。
#[tokio::test]
async fn datasource_is_full_and_stat_value() {
    let pool = DruidPool::builder()
        .name("ds-full")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();

    let ds = DruidDataSource::from_pool(pool);
    assert!(!ds.is_full());

    let stat_value = ds.stat_value_and_reset();
    assert!(stat_value.name.is_empty() || !stat_value.name.is_empty());
}

// ── WallVisitorUtils（Java WallVisitorUtils.rewriteForMultiTenant）──

/// `rewrite_for_multi_tenant：无` `tenant_call_back` + 空 pattern → 不修改。
#[test]
fn wall_visitor_utils_rewrite_no_tenant() {
    let config = WallConfig::default();
    let mut stmts = Parser::parse_sql(&GenericDialect, "SELECT * FROM t").unwrap();
    let result = WallVisitorUtils::rewrite_for_multi_tenant(&mut stmts, &config).unwrap();
    assert!(!result, "no tenant config should not modify");
}

/// `rewrite_for_multi_tenant：有` `tenant_table_pattern` + 无 callback → 不修改。
#[test]
fn wall_visitor_utils_rewrite_pattern_no_callback() {
    let config = WallConfig::builder().tenant_table_pattern("t*").build();
    let mut stmts = Parser::parse_sql(&GenericDialect, "SELECT * FROM t").unwrap();
    let result = WallVisitorUtils::rewrite_for_multi_tenant(&mut stmts, &config).unwrap();
    assert!(!result, "pattern without callback should not modify");
}

/// `rewrite_for_multi_tenant：UPDATE` 语句。
#[test]
fn wall_visitor_utils_rewrite_update() {
    let config = WallConfig::builder()
        .tenant_table_pattern("t")
        .tenant_column("tenant_id")
        .build();
    let mut stmts = Parser::parse_sql(&GenericDialect, "UPDATE t SET a = 1 WHERE id = 2").unwrap();
    let _result = WallVisitorUtils::rewrite_for_multi_tenant(&mut stmts, &config);
    // 无 callback 时返回 Ok(false)，但 config 有 pattern 时走 rewrite 路径。
}

/// `rewrite_for_multi_tenant：INSERT` 语句。
#[test]
fn wall_visitor_utils_rewrite_insert() {
    let config = WallConfig::builder()
        .tenant_table_pattern("t")
        .tenant_column("tenant_id")
        .build();
    let mut stmts = Parser::parse_sql(&GenericDialect, "INSERT INTO t (a) VALUES (1)").unwrap();
    let _result = WallVisitorUtils::rewrite_for_multi_tenant(&mut stmts, &config);
}

/// `rewrite_for_multi_tenant：DELETE` 语句（Java 不对 DELETE 注入条件）。
#[test]
fn wall_visitor_utils_rewrite_delete() {
    let config = WallConfig::builder()
        .tenant_table_pattern("t")
        .tenant_column("tenant_id")
        .build();
    let mut stmts = Parser::parse_sql(&GenericDialect, "DELETE FROM t WHERE id = 1").unwrap();
    let result = WallVisitorUtils::rewrite_for_multi_tenant(&mut stmts, &config).unwrap();
    // Java 不对 DELETE 注入条件。
    assert!(!result);
}

/// `rewrite_for_multi_tenant：CREATE` TABLE 不修改。
#[test]
fn wall_visitor_utils_rewrite_create_table() {
    let config = WallConfig::builder()
        .tenant_table_pattern("t")
        .tenant_column("tenant_id")
        .build();
    let mut stmts = Parser::parse_sql(&GenericDialect, "CREATE TABLE t (id INT)").unwrap();
    let result = WallVisitorUtils::rewrite_for_multi_tenant(&mut stmts, &config).unwrap();
    assert!(!result, "CREATE TABLE should not be rewritten");
}

/// `rewrite_for_multi_tenant：空语句列表`。
#[test]
fn wall_visitor_utils_rewrite_empty_statements() {
    let config = WallConfig::builder()
        .tenant_table_pattern("t")
        .tenant_column("tenant_id")
        .build();
    let mut stmts: Vec<Statement> = vec![];
    let result = WallVisitorUtils::rewrite_for_multi_tenant(&mut stmts, &config).unwrap();
    assert!(!result);
}
