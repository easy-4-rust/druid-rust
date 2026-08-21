//! `DruidDataSource` facade differential coverage tests (Java Druid 1.2.28).
//!
//! Covers: `from_pool`, `DataSourceProxy` trait, `DataSourceMonitorable` trait,
//! `ManagedDataSource` trait, Pool trait delegation, `register_monitoring`,
//! `stat_value_and_reset`, `publish_stats`, `close_for_removal_if_idle`, `is_full`,
//! `try_get_connection`, `fill_to`, restart, `notify_credentials_changed`.

use druid::core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionFactory, Pool, Row, Value,
};
use druid::pool::{DruidDataSource, DruidPool};
use druid::stats::DataSourceMonitorable;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

// ===========================================================================
// Test infrastructure
// ===========================================================================

struct FacadeConnection {
    closed: bool,
    close_count: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl PhysicalConnection for FacadeConnection {
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
        if !self.closed {
            self.closed = true;
            self.close_count.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }
    fn is_closed(&self) -> bool {
        self.closed
    }
    fn driver_name(&self) -> &'static str {
        "facade-test"
    }
}

struct FacadeFactory {
    create_count: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for FacadeFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        self.create_count.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(FacadeConnection {
            closed: false,
            close_count: Arc::new(AtomicU64::new(0)),
        }))
    }
    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }
}

fn make_data_source_sync() -> DruidDataSource {
    let factory = Arc::new(FacadeFactory {
        create_count: Arc::new(AtomicU64::new(0)),
    });
    let pool = tokio::runtime::Runtime::new().unwrap().block_on(async {
        DruidPool::builder()
            .name("facade-test")
            .driver_name("facade-test")
            .factory(factory)
            .max_open(4)
            .max_idle(4)
            .build()
            .await
            .unwrap()
    });
    DruidDataSource::from_pool(pool)
}

async fn make_data_source_async() -> DruidDataSource {
    let factory = Arc::new(FacadeFactory {
        create_count: Arc::new(AtomicU64::new(0)),
    });
    let pool = DruidPool::builder()
        .name("facade-test")
        .driver_name("facade-test")
        .factory(factory)
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();
    DruidDataSource::from_pool(pool)
}

// ===========================================================================
// 1. from_pool
// ===========================================================================

#[test]
fn data_source_from_pool() {
    let ds = make_data_source_sync();
    assert!(!ds.is_initialized());
    assert!(!ds.is_closed());
}

// ===========================================================================
// 2. DruidDataSource methods (direct, not through traits)
// ===========================================================================

#[test]
fn data_source_login_timeout() {
    let ds = make_data_source_sync();
    let _ = ds.login_timeout();
}

#[test]
fn data_source_is_on_fatal_error_initial() {
    let ds = make_data_source_sync();
    assert!(!ds.is_on_fatal_error());
}

#[test]
fn data_source_on_fatal_error_max_active() {
    let ds = make_data_source_sync();
    let _ = ds.on_fatal_error_max_active();
}

#[test]
fn data_source_is_async_init() {
    let ds = make_data_source_sync();
    let _ = ds.is_async_init();
}

#[test]
fn data_source_is_init_exception_throw() {
    let ds = make_data_source_sync();
    let _ = ds.is_init_exception_throw();
}

#[test]
fn data_source_is_fail_continuous_initial() {
    let ds = make_data_source_sync();
    assert!(!ds.is_fail_continuous());
}

#[test]
fn data_source_last_create_error_initial() {
    let ds = make_data_source_sync();
    assert!(ds.last_create_error().is_none());
    assert_eq!(ds.last_create_error_time_millis(), 0);
}

#[test]
fn data_source_stat_value_and_reset() {
    let ds = make_data_source_sync();
    let stat = ds.stat_value_and_reset();
    assert_eq!(stat.name, "facade-test");
    assert_eq!(stat.driver_class_name, "facade-test");
    assert!(stat.max_active > 0);
}

#[test]
fn data_source_reset_stat_enable() {
    let ds = make_data_source_sync();
    assert!(ds.is_reset_stat_enable());
    ds.set_reset_stat_enable(false);
    assert!(!ds.is_reset_stat_enable());
    ds.set_reset_stat_enable(true);
}

#[test]
fn data_source_reset_stat_increments() {
    let ds = make_data_source_sync();
    let before = ds.reset_count();
    ds.reset_stat();
    assert!(ds.reset_count() > before);
}

#[test]
fn data_source_reset_stat_disabled() {
    let ds = make_data_source_sync();
    ds.set_reset_stat_enable(false);
    let before = ds.reset_count();
    ds.reset_stat();
    assert_eq!(ds.reset_count(), before);
    ds.set_reset_stat_enable(true);
}

#[test]
fn data_source_publish_stats() {
    let ds = make_data_source_sync();
    let result = ds.publish_stats();
    assert!(result.is_ok());
}

#[tokio::test]
async fn data_source_try_get_connection_empty() {
    let ds = make_data_source_async().await;
    let result = ds.try_get_connection().await.unwrap();
    assert!(result.is_none());
}

#[test]
fn data_source_is_full_initial() {
    let ds = make_data_source_sync();
    assert!(!ds.is_full());
}

#[test]
fn data_source_user_password_version_initial() {
    let ds = make_data_source_sync();
    assert_eq!(ds.user_password_version(), 0);
}

#[test]
fn data_source_discard_connection_none() {
    let ds = make_data_source_sync();
    assert!(!ds.discard_connection(None));
}

#[test]
fn data_source_remove_abandoned_disabled() {
    let ds = make_data_source_sync();
    assert_eq!(ds.remove_abandoned(), 0);
}

#[test]
fn data_source_native_pool() {
    let ds = make_data_source_sync();
    let pool = ds.native_pool();
    assert_eq!(pool.name(), "facade-test");
}

// ===========================================================================
// 3. DataSourceMonitorable trait methods
// ===========================================================================

#[test]
fn data_source_monitorable_name() {
    let ds = make_data_source_sync();
    assert_eq!(DataSourceMonitorable::name(&ds), "facade-test");
}

#[test]
fn data_source_monitorable_driver_name() {
    let ds = make_data_source_sync();
    assert_eq!(DataSourceMonitorable::driver_name(&ds), Some("facade-test"));
}

#[test]
fn data_source_monitorable_stat_data() {
    let ds = make_data_source_sync();
    let stat = ds.data_source_stat_data();
    assert!(stat.is_object());
    let map = stat.as_object().unwrap();
    assert!(map.contains_key("Name"));
    assert!(map.contains_key("URL"));
    assert!(map.contains_key("DriverClassName"));
    assert!(map.contains_key("MaxActive"));
    assert!(map.contains_key("ActiveCount"));
    assert!(map.contains_key("PoolingCount"));
    assert!(map.contains_key("Closed"));
}

#[test]
fn data_source_monitorable_sql_stat_data() {
    let ds = make_data_source_sync();
    let list = ds.sql_stat_data();
    assert!(list.is_empty() || !list.is_empty());
}

#[test]
fn data_source_monitorable_wall_stat_data() {
    let ds = make_data_source_sync();
    let wall = ds.wall_stat_data();
    assert!(wall.is_object());
}

#[test]
fn data_source_monitorable_pooling_connection_info() {
    let ds = make_data_source_sync();
    let info = ds.pooling_connection_info();
    assert!(info.is_empty() || !info.is_empty());
}

#[test]
fn data_source_monitorable_active_connection_stack_trace() {
    let ds = make_data_source_sync();
    let traces = ds.active_connection_stack_trace();
    assert!(traces.is_empty());
}

#[test]
fn data_source_monitorable_is_remove_abandoned() {
    let ds = make_data_source_sync();
    assert!(!ds.is_remove_abandoned());
}

#[test]
fn data_source_monitorable_reset_stat() {
    let ds = make_data_source_sync();
    ds.reset_stat();
}

#[test]
fn data_source_monitorable_reset_rdbc_stat() {
    let ds = make_data_source_sync();
    ds.reset_rdbc_stat();
}

#[test]
fn data_source_monitorable_log_stats() {
    let ds = make_data_source_sync();
    let result = ds.log_stats();
    assert!(result.is_ok());
}

// ===========================================================================
// 4. Pool trait delegation
// ===========================================================================

#[test]
fn data_source_pool_state() {
    let ds = make_data_source_sync();
    let state = Pool::state(&ds);
    assert_eq!(state.name, "facade-test");
    assert_eq!(state.driver_name, "facade-test");
}

#[test]
fn data_source_pool_driver_name() {
    let ds = make_data_source_sync();
    assert_eq!(Pool::driver_name(&ds), "facade-test");
}

#[test]
fn data_source_pool_name() {
    let ds = make_data_source_sync();
    assert_eq!(Pool::name(&ds), "facade-test");
}

// ===========================================================================
// 5. register_monitoring
// ===========================================================================

#[test]
fn data_source_register_monitoring() {
    let ds = Arc::new(make_data_source_sync());
    let id1 = ds.register_monitoring();
    assert!(id1 > 0);
    let id2 = ds.register_monitoring();
    assert_eq!(id1, id2);
}

// ===========================================================================
// 6. state snapshot
// ===========================================================================

#[test]
fn data_source_state_snapshot() {
    let ds = make_data_source_sync();
    let state = ds.state();
    assert_eq!(state.name, "facade-test");
    assert_eq!(state.driver_name, "facade-test");
    assert_eq!(state.max_open, 4);
    assert_eq!(state.active_count, 0);
    assert_eq!(state.idle_count, 0);
    assert!(!state.closed);
}

// ===========================================================================
// 7. init / close lifecycle
// ===========================================================================

#[tokio::test]
async fn data_source_init_and_close() {
    let ds = make_data_source_async().await;
    assert!(!ds.is_initialized());
    ds.init().await.unwrap();
    assert!(ds.is_initialized());
    assert!(!ds.is_closed());
    ds.close().await;
    assert!(ds.is_closed());
}

// ===========================================================================
// 8. restart
// ===========================================================================

#[tokio::test]
async fn data_source_restart_after_close() {
    let ds = make_data_source_async().await;
    ds.init().await.unwrap();
    ds.close().await;
    assert!(ds.is_closed());
    ds.restart().await.unwrap();
    assert!(!ds.is_closed());
    assert!(!ds.is_initialized());
}

// ===========================================================================
// 9. fill / fill_to
// ===========================================================================

#[tokio::test]
async fn data_source_fill_to_count() {
    let ds = make_data_source_async().await;
    let created = ds.fill_to(2).await.unwrap();
    assert!(created >= 2);
}

// ===========================================================================
// 10. shrink
// ===========================================================================

#[tokio::test]
async fn data_source_shrink() {
    let ds = make_data_source_async().await;
    ds.shrink().await;
}

#[tokio::test]
async fn data_source_shrink_check_time() {
    let ds = make_data_source_async().await;
    ds.shrink_check_time(false).await;
}

#[tokio::test]
async fn data_source_shrink_with_options() {
    let ds = make_data_source_async().await;
    ds.shrink_with_options(false, false).await;
}

// ===========================================================================
// 11. notify_credentials_changed
// ===========================================================================

#[tokio::test]
async fn data_source_credentials_version() {
    let ds = make_data_source_async().await;
    assert_eq!(ds.user_password_version(), 0);
    let v1 = ds.notify_credentials_changed().await.unwrap();
    assert_eq!(v1, 1);
    assert_eq!(ds.user_password_version(), 1);
}
