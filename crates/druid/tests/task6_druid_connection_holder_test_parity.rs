//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! `DruidConnectionHolder` Java 语义对照测试。
//!
//! Java 来源：
//! - `DruidConnectionHolder.java`
//! - `DruidConnectionHolderTest4.java`
//! - `LastActiveTest_0.java`

use druid::core::{
    Connection, DruidConnectionHolder, DruidPooledConnection, ExecResult, RdbcError, Row, Value,
};
use druid_rdbc::connection::TransactionIsolation;
use druid_rdbc::statement::ResultSetHoldability;
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
    holdability: ResultSetHoldability,
    isolation: TransactionIsolation,
    schema: Option<String>,
}

impl HolderConnection {
    fn new(probe: Arc<Mutex<HolderProbe>>) -> Self {
        Self {
            probe,
            auto_commit: true,
            read_only: false,
            holdability: ResultSetHoldability::CloseAtCommit,
            isolation: TransactionIsolation::READ_COMMITTED,
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

druid_rdbc::impl_standard_wrapper!(HolderConnection => Connection);

#[async_trait::async_trait]
impl Connection for HolderConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        Ok(ExecResult::default())
    }

    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        Ok(Vec::new())
    }

    async fn begin(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.probe
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self
            .probe
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed)
    }

    fn auto_commit(&self) -> Result<bool, RdbcError> {
        Ok(self.auto_commit)
    }

    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), RdbcError> {
        self.record(format!("auto_commit:{auto_commit}"));
        self.auto_commit = auto_commit;
        Ok(())
    }

    fn read_only(&self) -> Result<bool, RdbcError> {
        Ok(self.read_only)
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), RdbcError> {
        self.record(format!("read_only:{read_only}"));
        self.read_only = read_only;
        Ok(())
    }

    fn transaction_isolation(&self) -> Result<TransactionIsolation, RdbcError> {
        Ok(self.isolation)
    }

    async fn set_transaction_isolation(
        &mut self,
        isolation: TransactionIsolation,
    ) -> Result<(), RdbcError> {
        self.record(format!("isolation:{}", isolation.jdbc_code()));
        self.isolation = isolation;
        Ok(())
    }

    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        Ok(self.holdability)
    }

    async fn set_holdability(
        &mut self,
        holdability: ResultSetHoldability,
    ) -> Result<(), RdbcError> {
        self.record(format!("holdability:{holdability:?}"));
        self.holdability = holdability;
        Ok(())
    }

    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.record("clear_warnings");
        Ok(())
    }

    fn schema(&self) -> Result<Option<String>, RdbcError> {
        Ok(self.schema.clone())
    }

    async fn set_schema(&mut self, schema: Option<&str>) -> Result<(), RdbcError> {
        self.record(format!("schema:{}", schema.unwrap_or("<none>")));
        self.schema = schema.map(str::to_owned);
        Ok(())
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("holder-probe".to_owned())
    }

    async fn warnings(&mut self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        Ok(None)
    }
}

async fn new_holder(
    id: u64,
    user_password_version: u64,
) -> (DruidConnectionHolder, Arc<Mutex<HolderProbe>>) {
    let probe = Arc::new(Mutex::new(HolderProbe::default()));
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(HolderConnection::new(probe.clone())),
        id,
        Duration::from_micros(25),
        user_password_version,
    )
    .await
    .expect("canonical holder");
    (holder, probe)
}

#[tokio::test]
async fn task6_druid_connection_holder_test_holder_owns_connection_and_resets_java_default_state_in_order(
) {
    let (holder, probe) = new_holder(41, 7).await;
    assert_eq!(holder.connection_id(), 41);
    assert_eq!(holder.user_password_version(), 7);
    assert_eq!(holder.create_duration(), Duration::from_micros(25));
    assert!(holder.has_standard_connection());
    assert!(holder.defaults().auto_commit());
    assert!(!holder.defaults().read_only());
    assert_eq!(holder.defaults().holdability(), 0);
    assert_eq!(holder.defaults().transaction_isolation(), 2);
    let returned = Arc::new(Mutex::new(None));
    let returned_holder = Arc::clone(&returned);
    let mut connection = DruidPooledConnection::with_standard_holder(
        holder,
        "holder-reset".to_owned(),
        None,
        false,
        false,
        Box::new(move |holder, disposition| {
            assert!(disposition.is_reusable());
            *returned_holder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(holder);
            true
        }),
    );
    connection.set_read_only(true).await.unwrap();
    connection
        .set_holdability(ResultSetHoldability::HoldOverCommit)
        .await
        .unwrap();
    connection
        .set_transaction_isolation(TransactionIsolation::SERIALIZABLE)
        .await
        .unwrap();
    connection.set_auto_commit(false).await.unwrap();
    probe.lock().unwrap().events.clear();
    connection.close().await.unwrap();
    assert!(returned.lock().unwrap().is_some());
    assert_eq!(
        probe.lock().unwrap().events,
        vec!["read_only:false", "auto_commit:true", "clear_warnings"]
    );
}

#[tokio::test]
async fn task6_druid_connection_holder_test_holder_preserves_schema_until_successful_recycle_restore(
) {
    let (holder, probe) = new_holder(42, 0).await;
    holder.set_restore_schema_on_recycle(true);
    assert!(holder.should_restore_schema_on_recycle());
    holder.remember_initial_schema(Some("base".to_owned()));
    holder.remember_initial_schema(Some("ignored".to_owned()));
    let mut connection = DruidPooledConnection::with_standard_holder(
        holder,
        "holder-schema".to_owned(),
        None,
        false,
        false,
        Box::new(|_, disposition| disposition.is_reusable()),
    );
    connection.set_schema(Some("tenant")).await.unwrap();
    probe.lock().unwrap().events.clear();
    connection.close().await.unwrap();
    let events = probe.lock().unwrap().events.clone();
    assert_eq!(
        events
            .iter()
            .filter(|event| *event == "schema:base")
            .count(),
        1
    );
    assert!(events.contains(&"clear_warnings".to_owned()));
}

#[tokio::test]
async fn task6_druid_connection_holder_test_pooled_mysql_schema_is_restored_after_validation_position(
) {
    let (holder, probe) = new_holder(46, 0).await;
    holder.set_restore_schema_on_recycle(true);
    let returned = Arc::new(AtomicU64::new(0));
    let returned_for_callback = Arc::clone(&returned);
    let mut connection = DruidPooledConnection::with_standard_holder(
        holder,
        "mysql-datasource".to_owned(),
        None,
        false,
        false,
        Box::new(move |_, disposition| {
            assert!(disposition.is_reusable());
            returned_for_callback.fetch_add(1, Ordering::Relaxed);
            false
        }),
    );
    connection.set_schema(Some("tenant")).await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(returned.load(Ordering::Relaxed), 1);
    assert_eq!(
        probe.lock().unwrap().events,
        vec!["schema:tenant", "clear_warnings", "schema:base"]
    );
}

#[tokio::test]
async fn task6_druid_connection_holder_test_holder_tracks_java_lifecycle_times_counts_and_version()
{
    let (holder, _probe) = new_holder(43, 11).await;
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
    assert!(holder.connection_age() < Duration::from_secs(1));
    assert!(format!("{holder:?}").contains("user_password_version"));
}

#[tokio::test]
async fn task6_druid_connection_holder_test_holder_discard_marker_reaches_adapter_and_connection_is_taken_once(
) {
    let (mut holder, probe) = new_holder(44, 0).await;
    assert_eq!(holder.connection_id(), 44);
    holder.mark_discarded();
    assert!(holder.is_discard());
    assert!(holder.has_standard_connection());
    holder.close_standard_connection().await.unwrap();
    assert!(!holder.has_standard_connection());
    assert!(holder.is_discard());
    assert!(probe.lock().unwrap().closed);
}

#[test]
fn task6_druid_connection_holder_test_empty_compatibility_holder_exposes_honest_absence() {
    let holder = DruidConnectionHolder::new(45);
    assert_eq!(holder.connection_id(), 45);
    assert_eq!(holder.use_count(), 0);
    assert!(!holder.has_standard_connection());
    assert!(holder.variables().is_none());
    assert!(holder.global_variables().is_none());
    assert!(holder.is_discard());
    let debug = format!("{holder:?}");
    assert!(debug.contains("has_connection"));
    assert!(debug.contains("discard"));
}
