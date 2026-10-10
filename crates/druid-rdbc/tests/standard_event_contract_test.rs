use druid_rdbc::connection::Connection;
use druid_rdbc::data_source::{
    ConnectionEvent, ConnectionEventListener, PooledConnection, PooledConnectionSource,
    StatementEvent, StatementEventListener, StatementSource,
};
use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::statement::{PreparedStatement, Statement};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

struct PoolOwner(Arc<AtomicUsize>);
#[async_trait::async_trait]
impl PooledConnection for PoolOwner {
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        Err(RdbcError::PoolClosed)
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
struct Prepared(Arc<AtomicUsize>);
druid_rdbc::impl_standard_wrapper!(Prepared => PreparedStatement);
#[async_trait::async_trait]
impl Statement for Prepared {
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
#[async_trait::async_trait]
impl PreparedStatement for Prepared {}
fn pool(count: &Arc<AtomicUsize>) -> PooledConnectionSource {
    Arc::new(tokio::sync::Mutex::new(Box::new(PoolOwner(Arc::clone(
        count,
    )))))
}
fn prepared(count: &Arc<AtomicUsize>) -> StatementSource {
    Arc::new(tokio::sync::Mutex::new(Box::new(Prepared(Arc::clone(
        count,
    )))))
}
fn sql_error() -> Arc<SqlException> {
    Arc::new(
        SqlException::new(1040, Some("08004".into()), Some("capacity".into()))
            .with_class_name("vendor.CapacityException"),
    )
}

#[tokio::test]
async fn connection_event_owns_the_same_source_after_callers_drop_it() {
    let count = Arc::new(AtomicUsize::new(0));
    let source = pool(&count);
    let weak = Arc::downgrade(&source);
    let event = ConnectionEvent::new(Arc::clone(&source));
    assert!(Arc::ptr_eq(event.source(), &source));
    assert!(event.sql_exception().is_none());
    let cloned = event.clone();
    drop(source);
    drop(event);
    assert!(weak.upgrade().is_some());
    cloned.source().lock().await.close().await.unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    drop(cloned);
    assert!(weak.upgrade().is_none());
}

#[test]
fn connection_error_event_preserves_nullable_exception_and_original_identity() {
    let source = pool(&Arc::new(AtomicUsize::new(0)));
    let error = sql_error();
    let event = ConnectionEvent::with_sql_exception(Arc::clone(&source), Some(Arc::clone(&error)));
    assert!(Arc::ptr_eq(event.sql_exception().unwrap(), &error));
    assert_eq!(event.sql_exception().unwrap().as_ref(), error.as_ref());
    assert!(ConnectionEvent::with_sql_exception(source, None)
        .sql_exception()
        .is_none());
}

#[tokio::test]
async fn statement_event_retains_exact_prepared_statement_and_pool_owner() {
    let count = Arc::new(AtomicUsize::new(0));
    let source = pool(&count);
    let statement = prepared(&count);
    let error = sql_error();
    let event = StatementEvent::with_sql_exception(
        Arc::clone(&source),
        Some(Arc::clone(&statement)),
        Some(Arc::clone(&error)),
    );
    assert!(Arc::ptr_eq(event.source(), &source));
    assert!(Arc::ptr_eq(event.statement().unwrap(), &statement));
    assert!(Arc::ptr_eq(event.sql_exception().unwrap(), &error));
    let cloned = event.clone();
    drop(statement);
    drop(source);
    drop(error);
    drop(event);
    cloned
        .statement()
        .unwrap()
        .lock()
        .await
        .close()
        .await
        .unwrap();
    cloned.source().lock().await.close().await.unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[test]
fn statement_event_preserves_java_nullable_statement_and_exception() {
    let event = StatementEvent::new(pool(&Arc::new(AtomicUsize::new(0))), None);
    assert!(event.statement().is_none());
    assert!(event.sql_exception().is_none());
    let event = StatementEvent::with_sql_exception(Arc::clone(event.source()), None, None);
    assert!(event.statement().is_none());
    assert!(event.sql_exception().is_none());
}

type ObservedEvent = (&'static str, usize, Option<usize>, Option<usize>);

#[derive(Default)]
struct Listener(Mutex<Vec<ObservedEvent>>);
impl ConnectionEventListener for Listener {
    fn connection_closed(&self, event: &ConnectionEvent) {
        self.record(
            "connection-closed",
            event.source(),
            None,
            event.sql_exception(),
        );
    }
    fn connection_error_occurred(&self, event: &ConnectionEvent) {
        self.record(
            "connection-error",
            event.source(),
            None,
            event.sql_exception(),
        );
    }
}
impl StatementEventListener for Listener {
    fn statement_closed(&self, event: &StatementEvent) {
        self.record(
            "statement-closed",
            event.source(),
            event.statement(),
            event.sql_exception(),
        );
    }
    fn statement_error_occurred(&self, event: &StatementEvent) {
        self.record(
            "statement-error",
            event.source(),
            event.statement(),
            event.sql_exception(),
        );
    }
}
impl Listener {
    fn record(
        &self,
        name: &'static str,
        source: &PooledConnectionSource,
        statement: Option<&StatementSource>,
        error: Option<&Arc<SqlException>>,
    ) {
        self.0.lock().unwrap().push((
            name,
            Arc::as_ptr(source) as usize,
            statement.map(|s| Arc::as_ptr(s) as usize),
            error.map(|e| Arc::as_ptr(e) as usize),
        ));
    }
}

#[test]
fn external_listener_callbacks_receive_original_event_objects() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<ConnectionEvent>();
    send_sync::<StatementEvent>();
    let source = pool(&Arc::new(AtomicUsize::new(0)));
    let statement = prepared(&Arc::new(AtomicUsize::new(0)));
    let error = sql_error();
    let connection =
        ConnectionEvent::with_sql_exception(Arc::clone(&source), Some(Arc::clone(&error)));
    let prepared = StatementEvent::with_sql_exception(
        Arc::clone(&source),
        Some(Arc::clone(&statement)),
        Some(Arc::clone(&error)),
    );
    let listener = Listener::default();
    let connection_listener: &dyn ConnectionEventListener = &listener;
    let statement_listener: &dyn StatementEventListener = &listener;
    connection_listener.connection_closed(&connection);
    connection_listener.connection_error_occurred(&connection);
    statement_listener.statement_closed(&prepared);
    statement_listener.statement_error_occurred(&prepared);
    let events = listener.0.lock().unwrap();
    assert_eq!(events.len(), 4);
    for (index, item) in events.iter().enumerate() {
        assert_eq!(item.1, Arc::as_ptr(&source) as usize);
        assert_eq!(
            item.2,
            (index >= 2).then_some(Arc::as_ptr(&statement) as usize)
        );
        assert_eq!(item.3, Some(Arc::as_ptr(&error) as usize));
    }
    assert_eq!(
        events.iter().map(|e| e.0).collect::<Vec<_>>(),
        [
            "connection-closed",
            "connection-error",
            "statement-closed",
            "statement-error"
        ]
    );
}
