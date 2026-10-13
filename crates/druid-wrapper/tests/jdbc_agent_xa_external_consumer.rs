use druid_rdbc::connection::Connection;
use druid_rdbc::data_source::{
    CommonDataSource, ConnectionEvent, ConnectionEventListener, PooledConnectionSource,
    XaConnection, XaDataSource, XaError, XaFlags, Xid,
};
use druid_rdbc::error::RdbcError;
use druid_wrapper::jdbc_agent::{JdbcAgentOptions, JdbcAgentXaDataSource};
use serde_json::{json, Map};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn assert_standard_xa_data_source<T: CommonDataSource + XaDataSource>() {}

async fn transaction_manager_starts_through_the_standard_resource(
    connection: &mut dyn XaConnection,
    xid: &Xid,
) -> Result<(), XaError> {
    connection
        .xa_resource()?
        .start(xid, XaFlags::TMNOFLAGS)
        .await
}

#[derive(Default)]
struct PoolEventState {
    logical_returns: AtomicUsize,
    owner_errors: AtomicUsize,
    discarded: AtomicBool,
    error_source_lock_acquired: AtomicUsize,
    panic_on_error: AtomicBool,
    event_sources: Mutex<Vec<PooledConnectionSource>>,
    error_sql_states: Mutex<Vec<Option<String>>>,
}

struct ExternalPoolListener(Arc<PoolEventState>);

impl ConnectionEventListener for ExternalPoolListener {
    fn connection_closed(&self, event: &ConnectionEvent) {
        self.0.logical_returns.fetch_add(1, Ordering::AcqRel);
        self.0
            .event_sources
            .lock()
            .expect("pool event sources")
            .push(Arc::clone(event.source()));
    }

    fn connection_error_occurred(&self, event: &ConnectionEvent) {
        self.0.owner_errors.fetch_add(1, Ordering::AcqRel);
        self.0.discarded.store(true, Ordering::Release);
        self.0
            .event_sources
            .lock()
            .expect("pool event sources")
            .push(Arc::clone(event.source()));
        self.0
            .error_sql_states
            .lock()
            .expect("pool error states")
            .push(
                event
                    .sql_exception()
                    .and_then(|error| error.sql_state().map(str::to_owned)),
            );
        if let Ok(mut source) = event.source().try_lock() {
            self.0
                .error_source_lock_acquired
                .fetch_add(1, Ordering::AcqRel);
            source
                .add_connection_event_listener(Arc::new(ExternalPoolListener(Arc::clone(&self.0))))
                .expect("listener re-registration during callback must not deadlock");
        }
        if self.0.panic_on_error.load(Ordering::Acquire) {
            panic!("injected external pool listener failure");
        }
    }
}

/// 最小的外部 XA 池消费器；仅通过 RDBC 标准 traits 管理 owner 与逻辑租约。
struct ExternalXaPool {
    owner: Box<dyn XaConnection>,
    listener: Arc<dyn ConnectionEventListener>,
    events: Arc<PoolEventState>,
}

impl ExternalXaPool {
    async fn new(data_source: &dyn XaDataSource) -> Result<Self, RdbcError> {
        let mut owner = data_source.xa_connection().await?;
        let events = Arc::new(PoolEventState::default());
        let listener: Arc<dyn ConnectionEventListener> =
            Arc::new(ExternalPoolListener(Arc::clone(&events)));
        owner.add_connection_event_listener(Arc::clone(&listener))?;
        Ok(Self {
            owner,
            listener,
            events,
        })
    }

    async fn borrow(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        if self.events.discarded.load(Ordering::Acquire) {
            return Err(RdbcError::ConnectionDiscarded);
        }
        self.owner.connection().await
    }

    fn event_source_is_stable(&self) -> bool {
        let sources = self
            .events
            .event_sources
            .lock()
            .expect("pool event sources");
        sources
            .first()
            .is_some_and(|first| sources.iter().all(|source| Arc::ptr_eq(first, source)))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        let _ = self
            .owner
            .remove_connection_event_listener(&self.listener)?;
        self.owner.close().await
    }
}

fn real_h2_xa_data_source(identity: &str) -> JdbcAgentXaDataSource {
    let agent_jar = std::env::var_os("DRUID_JDBC_AGENT_JAR")
        .expect("set DRUID_JDBC_AGENT_JAR to the built JDBC Agent shaded jar");
    let h2_jar = std::env::var_os("DRUID_JDBC_DRIVER_JAR")
        .expect("set DRUID_JDBC_DRIVER_JAR to the H2 JDBC jar");
    let java = std::env::var_os("DRUID_JAVA_EXECUTABLE").unwrap_or_else(|| "java".into());
    let options = JdbcAgentOptions::java(java, [PathBuf::from(agent_jar), PathBuf::from(h2_jar)])
        .expect("Java classpath must be representable on this platform");
    let artifact_version = options.artifact_version().to_owned();
    let jvm_options_hash = options.jvm_options_hash().to_owned();
    let options = options
        .runtime_identity(identity, artifact_version, jvm_options_hash)
        .request_timeout(Duration::from_secs(30));
    JdbcAgentXaDataSource::new("org.h2.jdbcx.JdbcDataSource", Map::new(), options)
        .expect("explicit H2 XADataSource class should be accepted")
        .property(
            "url",
            json!(format!("jdbc:h2:mem:{identity};DB_CLOSE_DELAY=-1")),
        )
        .property("user", json!("sa"))
        .property("password", json!(""))
}

#[test]
fn public_xa_data_source_implements_only_the_rdbc_standard_contract() {
    assert_standard_xa_data_source::<JdbcAgentXaDataSource>();
    let _standard_resource_consumer = transaction_manager_starts_through_the_standard_resource;

    let source = JdbcAgentXaDataSource::new(
        "org.postgresql.xa.PGXADataSource",
        Map::new(),
        JdbcAgentOptions::new("external-xa-consumer"),
    )
    .expect("explicit XA class should construct");
    let source = source
        .property("serverName", json!("127.0.0.1"))
        .property("password", json!("consumer-secret"));

    let debug = format!("{source:?}");
    assert!(!debug.contains("consumer-secret"));
}

#[tokio::test]
async fn blank_xa_class_fails_before_agent_startup() {
    let error = match JdbcAgentXaDataSource::new(
        "  ",
        Map::new(),
        JdbcAgentOptions::new("external-xa-consumer"),
    ) {
        Ok(_) => panic!("blank XA class must be rejected"),
        Err(error) => error,
    };

    assert!(matches!(error, RdbcError::InvalidArgument(_)));
}

#[tokio::test]
#[ignore = "requires DRUID_JDBC_AGENT_JAR, DRUID_JDBC_DRIVER_JAR, and Java"]
async fn external_pool_returns_logical_lease_and_reuses_live_xa_owner() {
    let source = real_h2_xa_data_source("external_pool_return");
    let mut pool = ExternalXaPool::new(&source)
        .await
        .expect("external pool must acquire standard XA owner");

    let mut first = pool.borrow().await.expect("first logical lease must open");
    first.ping().await.expect("first lease must reach H2");
    first
        .close()
        .await
        .expect("first lease must return to the pool");
    assert_eq!(pool.events.logical_returns.load(Ordering::Acquire), 1);
    assert!(!pool.events.discarded.load(Ordering::Acquire));
    assert!(pool.event_source_is_stable());

    pool.owner
        .xa_resource()
        .expect("owner remains live after logical return")
        .get_transaction_timeout()
        .await
        .expect("XA resource must remain usable after logical return");

    let mut second = pool.borrow().await.expect("owner must issue a new lease");
    second
        .ping()
        .await
        .expect("new generation must remain usable");
    second.close().await.expect("second lease must return");
    assert_eq!(pool.events.logical_returns.load(Ordering::Acquire), 2);
    assert!(pool.event_source_is_stable());
    pool.close()
        .await
        .expect("external pool closes owner on shutdown");
}

#[tokio::test]
#[ignore = "requires DRUID_JDBC_AGENT_JAR, DRUID_JDBC_DRIVER_JAR, and Java"]
async fn external_pool_discards_xa_owner_after_agent_transport_failure() {
    let source = real_h2_xa_data_source("external_pool_transport_failure");
    let mut pool = ExternalXaPool::new(&source)
        .await
        .expect("external pool must acquire standard XA owner");
    let mut connection = pool.borrow().await.expect("logical lease must open");
    connection
        .exec(
            "CREATE ALIAS AGENT_EXIT FOR 'java.lang.System.exit(int)'",
            Vec::new(),
        )
        .await
        .expect("H2 must create the process-failure probe");

    pool.events.panic_on_error.store(true, Ordering::Release);
    let primary_error = connection
        .fetch("CALL AGENT_EXIT(23)", Vec::new())
        .await
        .expect_err("Agent process failure must fail the in-flight SQL request");
    assert!(
        primary_error.sql_exception().is_none(),
        "fault injection must exercise a non-SQL transport error"
    );

    assert_eq!(pool.events.owner_errors.load(Ordering::Acquire), 1);
    assert_eq!(
        pool.events
            .error_source_lock_acquired
            .load(Ordering::Acquire),
        1,
        "error callback can reenter the owner source and update registrations"
    );
    assert_eq!(
        pool.events
            .error_sql_states
            .lock()
            .expect("pool error states")
            .as_slice(),
        &[Some("08006".to_owned())]
    );
    assert!(pool.events.discarded.load(Ordering::Acquire));
    assert!(pool.event_source_is_stable());
    assert!(matches!(
        pool.borrow().await,
        Err(RdbcError::ConnectionDiscarded)
    ));
    assert!(
        pool.owner.xa_resource().is_err(),
        "an unusable owner must not hand out an XA resource"
    );
    let _ = pool.close().await;
}
