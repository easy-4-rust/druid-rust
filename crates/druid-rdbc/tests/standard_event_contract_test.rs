use druid_rdbc::connection::Connection;
use druid_rdbc::data_source::{
    ConnectionEvent, ConnectionEventListener, PooledConnection, PooledConnectionEventRegistry,
    PooledConnectionSource, StatementEvent, StatementEventListener, StatementSource,
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

#[tokio::test]
async fn pooled_connection_event_registry_preserves_owner_error_and_duplicate_semantics() {
    let registry = PooledConnectionEventRegistry::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let source = pool(&counter);
    let listener = Arc::new(Listener::default());
    let connection_listener: Arc<dyn ConnectionEventListener> = listener.clone();

    registry.add_connection_event_listener(Arc::clone(&connection_listener));
    registry.add_connection_event_listener(Arc::clone(&connection_listener));
    assert_eq!(registry.notify_connection_closed(Arc::clone(&source)), 0);
    assert!(registry.remove_connection_event_listener(&connection_listener));

    let error = sql_error();
    assert_eq!(
        registry.notify_connection_error(Arc::clone(&source), Arc::clone(&error)),
        0
    );
    assert!(registry.remove_connection_event_listener(&connection_listener));
    assert!(!registry.remove_connection_event_listener(&connection_listener));

    let statement_listener: Arc<dyn StatementEventListener> = listener.clone();
    let statement = prepared(&counter);
    registry.add_statement_event_listener(Arc::clone(&statement_listener));
    registry.add_statement_event_listener(Arc::clone(&statement_listener));
    assert_eq!(
        registry.notify_statement_closed(Arc::clone(&source), Arc::clone(&statement)),
        0
    );
    assert!(registry.remove_statement_event_listener(&statement_listener));
    assert_eq!(
        registry.notify_statement_error(
            Arc::clone(&source),
            Arc::clone(&statement),
            Arc::clone(&error),
        ),
        0
    );
    assert!(registry.remove_statement_event_listener(&statement_listener));
    assert!(!registry.remove_statement_event_listener(&statement_listener));

    let events = listener.0.lock().unwrap();
    assert_eq!(events.len(), 6, "duplicate registration is delivered twice");
    assert_eq!(events[0].0, "connection-closed");
    assert_eq!(events[1].0, "connection-closed");
    assert_eq!(events[2].0, "connection-error");
    assert_eq!(events[3].0, "statement-closed");
    assert_eq!(events[4].0, "statement-closed");
    assert_eq!(events[5].0, "statement-error");
    assert!(events
        .iter()
        .all(|event| event.1 == Arc::as_ptr(&source) as usize));
    assert!(events[0].3.is_none());
    assert!(events[1].3.is_none());
    assert_eq!(events[2].3, Some(Arc::as_ptr(&error) as usize));
    assert!(events[3..5]
        .iter()
        .all(|event| event.2 == Some(Arc::as_ptr(&statement) as usize)));
    assert_eq!(events[5].2, Some(Arc::as_ptr(&statement) as usize));
    assert_eq!(events[5].3, Some(Arc::as_ptr(&error) as usize));
}

struct PanickingConnectionListener;
impl ConnectionEventListener for PanickingConnectionListener {
    fn connection_closed(&self, _event: &ConnectionEvent) {
        panic!("listener panic is isolated");
    }

    fn connection_error_occurred(&self, _event: &ConnectionEvent) {
        panic!("listener panic is isolated");
    }
}

#[tokio::test]
async fn pooled_connection_event_registry_counts_panics_and_continues_snapshot() {
    let registry = PooledConnectionEventRegistry::new();
    let source = pool(&Arc::new(AtomicUsize::new(0)));
    let panicking: Arc<dyn ConnectionEventListener> = Arc::new(PanickingConnectionListener);
    let recording = Arc::new(Listener::default());
    let recording_listener: Arc<dyn ConnectionEventListener> = recording.clone();
    registry.add_connection_event_listener(panicking);
    registry.add_connection_event_listener(Arc::clone(&recording_listener));

    assert_eq!(registry.notify_connection_closed(Arc::clone(&source)), 1);
    assert_eq!(registry.notify_connection_closed(Arc::clone(&source)), 1);
    assert_eq!(recording.0.lock().unwrap().len(), 2);
}

struct ReentrantConnectionListener {
    registry: Arc<PooledConnectionEventRegistry>,
    self_reference: Mutex<Option<std::sync::Weak<dyn ConnectionEventListener>>>,
    calls: AtomicUsize,
}

impl ConnectionEventListener for ReentrantConnectionListener {
    fn connection_closed(&self, _event: &ConnectionEvent) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let listener = self
            .self_reference
            .lock()
            .unwrap()
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .unwrap();
        assert!(self.registry.remove_connection_event_listener(&listener));
    }

    fn connection_error_occurred(&self, _event: &ConnectionEvent) {}
}

#[tokio::test]
async fn pooled_connection_event_registry_allows_reentrant_self_removal() {
    let registry = Arc::new(PooledConnectionEventRegistry::new());
    let concrete = Arc::new(ReentrantConnectionListener {
        registry: Arc::clone(&registry),
        self_reference: Mutex::new(None),
        calls: AtomicUsize::new(0),
    });
    let listener: Arc<dyn ConnectionEventListener> = concrete.clone();
    *concrete.self_reference.lock().unwrap() = Some(Arc::downgrade(&listener));
    registry.add_connection_event_listener(Arc::clone(&listener));
    let source = pool(&Arc::new(AtomicUsize::new(0)));

    assert_eq!(registry.notify_connection_closed(Arc::clone(&source)), 0);
    assert_eq!(registry.notify_connection_closed(source), 0);
    assert_eq!(concrete.calls.load(Ordering::SeqCst), 1);
}
