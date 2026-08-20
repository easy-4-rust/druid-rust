//! `DruidConnectionHolder` Java 语义对照测试。
//!
//! Java 来源：
//! - `DruidConnectionHolder.java`
//! - `DruidConnectionHolderTest4.java`
//! - `LastActiveTest_0.java`

extern crate druid_core as druid;
use druid_core::core::{
    DruidConnectionHolder, DruidError, DruidPooledConnection, ExecResult, PhysicalConnection,
    PhysicalConnectionCapabilities, Row, Value,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Default)]
struct HolderProbe {
    events: Vec<String>,
    discarded: bool,
    closed: bool,
}

struct HolderConnection {
    probe: Arc<Mutex<HolderProbe>>,
    auto_commit: bool,
    read_only: bool,
    holdability: i32,
    isolation: u8,
    schema: Option<String>,
}

impl HolderConnection {
    fn new(probe: Arc<Mutex<HolderProbe>>) -> Self {
        Self {
            probe,
            auto_commit: true,
            read_only: false,
            holdability: 1,
            isolation: 2,
            schema: Some("base".to_string()),
        }
    }

    fn record(&self, event: impl Into<String>) {
        self.probe
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .events
            .push(event.into());
    }
}

#[async_trait::async_trait]
impl PhysicalConnection for HolderConnection {
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
        self.probe
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.probe
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed
    }

    fn capabilities(&self) -> PhysicalConnectionCapabilities {
        PhysicalConnectionCapabilities {
            transactions: true,
            auto_commit: true,
            read_only: true,
            transaction_isolation: true,
            holdability: true,
            clear_warnings: true,
            catalog: false,
            schema: true,
            savepoints: false,
        }
    }

    fn auto_commit(&self) -> bool {
        self.auto_commit
    }

    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), DruidError> {
        self.record(format!("auto_commit:{auto_commit}"));
        self.auto_commit = auto_commit;
        Ok(())
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), DruidError> {
        self.record(format!("read_only:{read_only}"));
        self.read_only = read_only;
        Ok(())
    }

    fn transaction_isolation(&self) -> u8 {
        self.isolation
    }

    async fn set_transaction_isolation(&mut self, isolation: u8) -> Result<(), DruidError> {
        self.record(format!("isolation:{isolation}"));
        self.isolation = isolation;
        Ok(())
    }

    fn holdability(&self) -> i32 {
        self.holdability
    }

    async fn set_holdability(&mut self, holdability: i32) -> Result<(), DruidError> {
        self.record(format!("holdability:{holdability}"));
        self.holdability = holdability;
        Ok(())
    }

    async fn clear_warnings(&mut self) -> Result<(), DruidError> {
        self.record("clear_warnings");
        Ok(())
    }

    fn mark_discarded(&mut self) {
        self.probe
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .discarded = true;
    }

    fn is_discarded(&self) -> bool {
        self.probe
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .discarded
    }

    fn schema(&self) -> Option<&str> {
        self.schema.as_deref()
    }

    async fn set_schema(&mut self, schema: &str) -> Result<(), DruidError> {
        self.record(format!("schema:{schema}"));
        self.schema = Some(schema.to_string());
        Ok(())
    }

    fn driver_name(&self) -> &str {
        "holder-probe"
    }
}

fn new_holder(
    id: u64,
    user_password_version: u64,
) -> (DruidConnectionHolder, Arc<Mutex<HolderProbe>>) {
    let probe = Arc::new(Mutex::new(HolderProbe::default()));
    let holder = DruidConnectionHolder::with_connection(
        Box::new(HolderConnection::new(probe.clone())),
        id,
        Duration::from_micros(25),
        user_password_version,
    );
    (holder, probe)
}

#[tokio::test]
async fn holder_owns_connection_and_resets_java_default_state_in_order() {
    let (mut holder, probe) = new_holder(41, 7);

    assert_eq!(holder.connection_id(), 41);
    assert_eq!(holder.user_password_version(), 7);
    assert_eq!(holder.create_duration(), Duration::from_micros(25));
    assert!(holder.has_physical_connection());
    assert!(holder.defaults().auto_commit());
    assert!(!holder.defaults().read_only());
    assert_eq!(holder.defaults().holdability(), 1);
    assert_eq!(holder.defaults().transaction_isolation(), 2);

    {
        let connection = holder.physical_connection_mut().unwrap();
        connection.set_read_only(true).await.unwrap();
        connection.set_holdability(9).await.unwrap();
        connection.set_transaction_isolation(8).await.unwrap();
        connection.set_auto_commit(false).await.unwrap();
    }
    probe
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .events
        .clear();

    holder.reset(false).await.unwrap();

    assert_eq!(
        probe
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .events,
        vec![
            "read_only:false",
            "holdability:1",
            "isolation:2",
            "auto_commit:true",
            "clear_warnings",
        ]
    );
}

#[tokio::test]
async fn holder_preserves_schema_until_successful_recycle_restore() {
    let (mut holder, probe) = new_holder(42, 0);
    holder.set_restore_schema_on_recycle(true);
    assert!(holder.should_restore_schema_on_recycle());
    holder.remember_initial_schema(Some("base".to_string()));
    holder.remember_initial_schema(Some("ignored".to_string()));
    holder
        .physical_connection_mut()
        .unwrap()
        .set_schema("tenant")
        .await
        .unwrap();
    probe
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .events
        .clear();

    holder.restore_initial_schema().await.unwrap();
    holder.restore_initial_schema().await.unwrap();

    assert_eq!(
        probe
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .events,
        vec!["schema:base"]
    );
}

#[tokio::test]
async fn pooled_mysql_schema_is_restored_after_validation_position() {
    let (holder, probe) = new_holder(46, 0);
    holder.set_restore_schema_on_recycle(true);
    let returned = Arc::new(AtomicU64::new(0));
    let returned_for_callback = returned.clone();
    let mut connection = DruidPooledConnection::with_holder(
        holder,
        "mysql-datasource".to_string(),
        None,
        false,
        None,
        Box::new(move |_holder, disposition| {
            assert!(disposition.is_reusable());
            returned_for_callback.fetch_add(1, Ordering::Relaxed);
            false
        }),
    );

    connection.set_schema("tenant").await.unwrap();
    connection.close().await.unwrap();

    assert_eq!(returned.load(Ordering::Relaxed), 1);
    assert_eq!(
        probe
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .events,
        vec!["schema:tenant", "clear_warnings", "schema:base"]
    );
}

#[test]
fn holder_tracks_java_lifecycle_times_counts_and_version() {
    let (holder, _probe) = new_holder(43, 11);

    assert!(holder.mark_active());
    assert_eq!(holder.use_count(), 1);
    assert!(!holder.mark_active());
    holder.record_execute();
    holder.record_valid();
    holder.record_keep_alive();
    holder.increment_keep_alive_check_count();
    holder.set_last_not_empty_wait(Duration::from_millis(3));

    assert_eq!(holder.keep_alive_check_count(), 1);
    assert_eq!(holder.last_not_empty_wait(), Duration::from_millis(3));
    assert!(holder.last_exec_idle_duration() < Duration::from_secs(1));
    assert!(holder.last_valid_elapsed().unwrap() < Duration::from_secs(1));
    assert!(holder.last_keep_elapsed().unwrap() < Duration::from_secs(1));
    assert!(holder.mark_idle());
    assert!(holder.idle_duration() < Duration::from_secs(1));
    assert!(holder.physical_age() < Duration::from_secs(1));
    assert!(format!("{holder:?}").contains("user_password_version"));
}

#[test]
fn holder_discard_marker_reaches_adapter_and_connection_is_taken_once() {
    let (mut holder, probe) = new_holder(44, 0);

    holder.mark_discarded();
    assert!(holder.is_discard());
    assert!(
        probe
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .discarded
    );

    let connection = holder.take_physical_connection();
    assert!(connection.is_some());
    assert!(holder.take_physical_connection().is_none());
    assert!(!holder.has_physical_connection());
    assert!(holder.is_discard());
}

#[test]
fn empty_compatibility_holder_exposes_honest_absence() {
    let mut holder = DruidConnectionHolder::new(45);

    assert!(!holder.has_physical_connection());
    assert!(holder.physical_connection().is_none());
    assert!(holder.physical_connection_mut().is_none());
    assert!(holder.physical_connection_box_mut().is_none());
    assert!(holder.is_discard());
    assert!(format!("{holder:?}").contains("has_physical_connection"));
}
