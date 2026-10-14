use druid::core::{Connection, ConnectionFactory};
use druid::pool::DruidPool;
use druid_rdbc::data_source::{
    ConnectionEvent, ConnectionEventListener, PooledConnection, PooledConnectionSource,
};
use druid_rdbc::error::RdbcError;
use druid_rdbc::impl_standard_wrapper;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct OwnerTestConnection {
    physical_closes: Arc<AtomicUsize>,
}

impl_standard_wrapper!(OwnerTestConnection => Connection);

#[async_trait::async_trait]
impl Connection for OwnerTestConnection {
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.physical_closes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

struct OwnerTestFactory {
    creates: Arc<AtomicUsize>,
    physical_closes: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl ConnectionFactory for OwnerTestFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        self.creates.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(OwnerTestConnection {
            physical_closes: Arc::clone(&self.physical_closes),
        }))
    }
}

#[derive(Default)]
struct ClosedEvents(Mutex<Vec<PooledConnectionSource>>);

impl ConnectionEventListener for ClosedEvents {
    fn connection_closed(&self, event: &ConnectionEvent) {
        self.0
            .lock()
            .expect("event sources")
            .push(Arc::clone(event.source()));
    }

    fn connection_error_occurred(&self, _event: &ConnectionEvent) {}
}

fn pool_factory(
    creates: Arc<AtomicUsize>,
    physical_closes: Arc<AtomicUsize>,
) -> Arc<OwnerTestFactory> {
    Arc::new(OwnerTestFactory {
        creates,
        physical_closes,
    })
}

#[tokio::test]
async fn native_owner_keeps_one_holder_across_logical_connection_returns() {
    let creates = Arc::new(AtomicUsize::new(0));
    let physical_closes = Arc::new(AtomicUsize::new(0));
    let pool = DruidPool::builder()
        .name("standard-owner-reborrow")
        .driver_name("owner-test")
        .factory(pool_factory(
            Arc::clone(&creates),
            Arc::clone(&physical_closes),
        ))
        .max_open(1)
        .build()
        .await
        .expect("pool builds");
    let mut owner = pool
        .get_pooled_connection_owner()
        .await
        .expect("standard owner is acquired");
    let events = Arc::new(ClosedEvents::default());
    owner
        .add_connection_event_listener(events.clone())
        .expect("listener registers");

    let mut first = owner.connection().await.expect("first logical lease");
    first.close().await.expect("first logical lease returns");
    drop(first);
    assert_eq!(
        pool.state().active_count,
        1,
        "owner retains the active holder"
    );
    assert_eq!(
        pool.state().idle_count,
        0,
        "holder is not in the native idle queue"
    );
    assert_eq!(physical_closes.load(Ordering::SeqCst), 0);

    let mut second = owner
        .connection()
        .await
        .expect("owner can issue another lease");
    second.close().await.expect("second logical lease returns");
    drop(second);
    assert_eq!(
        creates.load(Ordering::SeqCst),
        1,
        "both leases use one physical connection"
    );

    {
        let sources = events.0.lock().expect("event sources");
        assert_eq!(sources.len(), 2, "each logical close emits one event");
        assert!(
            Arc::ptr_eq(&sources[0], &sources[1]),
            "events retain stable owner identity"
        );
    }

    owner
        .close()
        .await
        .expect("owner returns holder to native pool");
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 1);
    pool.close().await;
    assert_eq!(physical_closes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn closing_native_owner_invalidates_an_outstanding_logical_handle() {
    let creates = Arc::new(AtomicUsize::new(0));
    let physical_closes = Arc::new(AtomicUsize::new(0));
    let pool = DruidPool::builder()
        .name("standard-owner-close-active")
        .driver_name("owner-test")
        .factory(pool_factory(
            Arc::clone(&creates),
            Arc::clone(&physical_closes),
        ))
        .max_open(1)
        .build()
        .await
        .expect("pool builds");
    let mut owner = pool
        .get_pooled_connection_owner()
        .await
        .expect("standard owner is acquired");
    let events = Arc::new(ClosedEvents::default());
    owner
        .add_connection_event_listener(events.clone())
        .expect("listener registers");
    let mut logical = owner.connection().await.expect("logical lease");

    owner
        .close()
        .await
        .expect("owner close is not held hostage by a lease handle");
    assert_eq!(events.0.lock().expect("event sources").len(), 1);
    assert!(logical.is_closed().expect("closed-state query succeeds"));
    assert!(logical.exec("select 1", Vec::new()).await.is_err());
    drop(logical);
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(creates.load(Ordering::SeqCst), 1);
}
