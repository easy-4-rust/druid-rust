//! Task 6 owner-disposition integration contract.

use druid::core::{
    Connection, ConnectionFactory, ExceptionSorter, ExceptionSorterProperties, RdbcError,
    SqlException,
};
use druid::pool::DruidPool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

struct FatalWarningConnection {
    closed: bool,
    close_count: Arc<AtomicU64>,
}

druid_rdbc::impl_standard_wrapper!(FatalWarningConnection => Connection);

#[async_trait::async_trait]
impl Connection for FatalWarningConnection {
    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::SqlException(Box::new(SqlException::driver(
            1042,
            "fatal warning failure",
        ))))
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if !self.closed {
            self.closed = true;
            self.close_count.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("owner-disposition-test".to_owned())
    }
}

struct TrackingFactory {
    create_count: Arc<AtomicU64>,
    close_count: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl ConnectionFactory for TrackingFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        self.create_count.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(FatalWarningConnection {
            closed: false,
            close_count: Arc::clone(&self.close_count),
        }))
    }

    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        connection.ping().await
    }
}

struct AlwaysFatalSorter;

impl ExceptionSorter for AlwaysFatalSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        true
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

async fn wait_for_count(counter: &AtomicU64, expected: u64) {
    for _ in 0..100 {
        if counter.load(Ordering::Acquire) == expected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert_eq!(counter.load(Ordering::Acquire), expected);
}

#[tokio::test]
async fn native_pool_fatal_signal_is_synchronous_and_holder_transfer_is_one_shot() {
    let create_count = Arc::new(AtomicU64::new(0));
    let driver_close_count = Arc::new(AtomicU64::new(0));
    let pool = DruidPool::builder()
        .name("owner-disposition-fatal")
        .driver_name("owner-disposition-test")
        .factory(Arc::new(TrackingFactory {
            create_count: Arc::clone(&create_count),
            close_count: Arc::clone(&driver_close_count),
        }))
        .max_open(1)
        .max_idle(1)
        .exception_sorter(Arc::new(AlwaysFatalSorter))
        .build()
        .await
        .unwrap();

    let mut connection = pool.get().await.unwrap();
    assert_eq!(pool.state().active_count, 1);
    assert!(matches!(
        connection.clear_warnings().await,
        Err(RdbcError::SqlException(_))
    ));

    // Observer accounting and lease invalidation happen before the error returns.
    let signaled = pool.state();
    assert_eq!(signaled.active_count, 0);
    assert_eq!(signaled.close_count, 1);
    assert_eq!(signaled.discard_count, 1);
    assert_eq!(signaled.destroy_count, 1);
    assert_eq!(driver_close_count.load(Ordering::Acquire), 0);
    assert!(!connection.is_recycled());
    assert!(connection.is_closed().unwrap());
    assert!(connection.ping().await.is_err());

    // The real holder is transferred only by close, without a second accounting event.
    connection.close().await.unwrap();
    connection.close().await.unwrap();
    wait_for_count(driver_close_count.as_ref(), 1).await;
    let closed = pool.state();
    assert_eq!(closed.close_count, 1);
    assert_eq!(closed.discard_count, 1);
    assert_eq!(closed.destroy_count, 1);

    // A replacement is clean, and its ordinary close remains reusable.
    let mut replacement = pool.get().await.unwrap();
    assert_eq!(create_count.load(Ordering::Acquire), 2);
    replacement.close().await.unwrap();
    let mut reused = pool.get().await.unwrap();
    assert_eq!(create_count.load(Ordering::Acquire), 2);
    reused.close().await.unwrap();
    assert_eq!(pool.state().discard_count, 1);

    pool.close().await;
    wait_for_count(driver_close_count.as_ref(), 2).await;
}

/// 致命错误后先关闭池，延后归还的租约仍必须恰好关闭一次驱动连接。
#[tokio::test]
async fn native_pool_shutdown_before_fatal_holder_return_closes_driver_once() {
    for drop_lease in [false, true] {
        let driver_close_count = Arc::new(AtomicU64::new(0));
        let pool = DruidPool::builder()
            .name("owner-disposition-shutdown-before-return")
            .driver_name("owner-disposition-test")
            .factory(Arc::new(TrackingFactory {
                create_count: Arc::new(AtomicU64::new(0)),
                close_count: Arc::clone(&driver_close_count),
            }))
            .max_open(1)
            .max_idle(1)
            .exception_sorter(Arc::new(AlwaysFatalSorter))
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        assert!(matches!(
            connection.clear_warnings().await,
            Err(RdbcError::SqlException(_))
        ));
        assert_eq!(pool.state().active_count, 0);
        pool.close().await;

        if drop_lease {
            drop(connection);
        } else {
            connection.close().await.unwrap();
            connection.close().await.unwrap();
            drop(connection);
        }
        wait_for_count(driver_close_count.as_ref(), 1).await;
        assert_eq!(pool.state().discard_count, 1);
        assert_eq!(pool.state().destroy_count, 1);
    }
}
