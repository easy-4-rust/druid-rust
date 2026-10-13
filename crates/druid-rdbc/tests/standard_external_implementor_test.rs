use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn external_crate_implements_standard_without_druid_dependency() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after Unix epoch for a unique test directory")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("druid-rdbc-external-{unique}"));
    fs::create_dir_all(root.join("src"))
        .expect("external implementor fixture directory must be created");
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = format!(
        r#"[package]
name = "rdbc-external-implementor"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
async-trait = "0.1"
druid-rdbc = {{ path = {manifest_dir:?} }}
tokio = {{ version = "1", features = ["macros", "rt"] }}
"#,
    );
    fs::write(root.join("Cargo.toml"), manifest)
        .expect("external implementor manifest must be written");
    fs::write(
        root.join("src/main.rs"),
        r#"use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{Statement, StatementOptions};
use druid_rdbc::statement::{CallableStatement, PreparedStatement};
use druid_rdbc::data_source::{
    ConnectionEvent, ConnectionEventListener, PooledConnection, StatementEvent,
    StatementEventListener,
};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper, WrapperExt};
use std::any::TypeId;

struct ExternalConnection;
struct ExternalStatement;
struct ExternalResultSet;
struct ExternalPooledConnection {
    connection_listeners: Vec<std::sync::Arc<dyn ConnectionEventListener>>,
    statement_listeners: Vec<std::sync::Arc<dyn StatementEventListener>>,
}
struct ExternalListener;
impl ConnectionEventListener for ExternalListener {
    fn connection_closed(&self, _event: &ConnectionEvent) {}
    fn connection_error_occurred(&self, _event: &ConnectionEvent) {}
}
impl StatementEventListener for ExternalListener {
    fn statement_closed(&self, _event: &StatementEvent) {}
    fn statement_error_occurred(&self, _event: &StatementEvent) {}
}
struct ExternalCallable;
struct ExternalGeneric<T>(std::marker::PhantomData<T>);
trait ExternalMarker: Send + 'static {}
impl ExternalMarker for u8 {}

druid_rdbc::impl_standard_wrapper!(ExternalConnection => Connection);
druid_rdbc::impl_standard_wrapper!(ExternalStatement => Statement);
druid_rdbc::impl_standard_wrapper!(ExternalResultSet => ResultSet);
druid_rdbc::impl_standard_wrapper!(ExternalCallable => CallableStatement);
druid_rdbc::impl_standard_wrapper!(impl<T: ExternalMarker> ExternalGeneric<T> => Connection);

#[async_trait::async_trait]
impl Connection for ExternalConnection {
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        Ok(Box::new(ExternalStatement))
    }
}

#[async_trait::async_trait]
impl Statement for ExternalStatement {
    async fn execute_query(&mut self, _sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        Ok(Box::new(ExternalResultSet))
    }
}

#[async_trait::async_trait]
impl ResultSet for ExternalResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

#[async_trait::async_trait]
impl Statement for ExternalCallable {}
#[async_trait::async_trait]
impl PreparedStatement for ExternalCallable {}
#[async_trait::async_trait]
impl CallableStatement for ExternalCallable {}

#[async_trait::async_trait]
impl<T: ExternalMarker> Connection for ExternalGeneric<T> {
    async fn create_statement(&mut self, _options: StatementOptions) -> Result<Box<dyn Statement>, RdbcError> {
        Ok(Box::new(ExternalStatement))
    }
}

#[async_trait::async_trait]
impl PooledConnection for ExternalPooledConnection {
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        Err(RdbcError::feature_not_supported("external_pooled_connection"))
    }

    fn add_connection_event_listener(
        &mut self,
        listener: std::sync::Arc<dyn ConnectionEventListener>,
    ) -> Result<(), RdbcError> {
        self.connection_listeners.push(listener);
        Ok(())
    }

    fn remove_connection_event_listener(
        &mut self,
        listener: &std::sync::Arc<dyn ConnectionEventListener>,
    ) -> Result<bool, RdbcError> {
        let Some(index) = self.connection_listeners.iter().position(|candidate| {
            std::sync::Arc::ptr_eq(candidate, listener)
        }) else {
            return Ok(false);
        };
        self.connection_listeners.remove(index);
        Ok(true)
    }

    fn add_statement_event_listener(
        &mut self,
        listener: std::sync::Arc<dyn StatementEventListener>,
    ) -> Result<(), RdbcError> {
        self.statement_listeners.push(listener);
        Ok(())
    }

    fn remove_statement_event_listener(
        &mut self,
        listener: &std::sync::Arc<dyn StatementEventListener>,
    ) -> Result<bool, RdbcError> {
        let Some(index) = self.statement_listeners.iter().position(|candidate| {
            std::sync::Arc::ptr_eq(candidate, listener)
        }) else {
            return Ok(false);
        };
        self.statement_listeners.remove(index);
        Ok(true)
    }
}

fn assert_send_static<T: Send + 'static + ?Sized>() {}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), RdbcError> {
    let _: Option<Box<dyn Connection>> = None;
    let _: Option<Box<dyn Statement>> = None;
    let _: Option<Box<dyn ResultSet>> = None;
    assert_send_static::<dyn Connection>();
    assert_send_static::<dyn Statement>();
    assert_send_static::<dyn ResultSet>();
    let connection = ExternalConnection;
    let connection_target = Some(TypeId::of::<dyn Connection>());
    assert!(connection.is_wrapper_for(connection_target)?);
    assert!(matches!(
        connection.unwrap(connection_target)?,
        StandardUnwrapped::Connection(_)
    ));
    struct UnknownTarget;
    let unknown = Some(TypeId::of::<UnknownTarget>());
    assert!(!connection.is_wrapper_for(unknown)?);
    assert!(matches!(connection.unwrap(unknown), Err(RdbcError::SqlException(_))));
    assert!(matches!(connection.is_wrapper_for(None), Err(RdbcError::InvalidArgument(_))));
    assert!(matches!(connection.unwrap(None), Err(RdbcError::InvalidArgument(_))));
    let concrete = connection.unwrap_ref::<ExternalConnection>()?;
    assert!(std::ptr::eq(
        std::ptr::from_ref(concrete),
        std::ptr::from_ref(&connection)
    ));

    let mut statement = ExternalStatement;
    assert!(matches!(statement.close().await, Err(RdbcError::SqlException(_))));
    assert!(matches!(statement.is_closed(), Err(RdbcError::SqlException(_))));
    let mut result_set = ExternalResultSet;
    assert!(matches!(result_set.close().await, Err(RdbcError::SqlException(_))));
    assert!(matches!(result_set.is_closed(), Err(RdbcError::SqlException(_))));
    let mut pooled = ExternalPooledConnection {
        connection_listeners: Vec::new(),
        statement_listeners: Vec::new(),
    };
    let connection_listener: std::sync::Arc<dyn ConnectionEventListener> =
        std::sync::Arc::new(ExternalListener);
    pooled.add_connection_event_listener(std::sync::Arc::clone(&connection_listener))?;
    pooled.add_connection_event_listener(std::sync::Arc::clone(&connection_listener))?;
    assert!(pooled.remove_connection_event_listener(&connection_listener)?);
    assert_eq!(pooled.connection_listeners.len(), 1);
    assert!(pooled.remove_connection_event_listener(&connection_listener)?);
    assert!(!pooled.remove_connection_event_listener(&connection_listener)?);

    let statement_listener: std::sync::Arc<dyn StatementEventListener> =
        std::sync::Arc::new(ExternalListener);
    pooled.add_statement_event_listener(std::sync::Arc::clone(&statement_listener))?;
    pooled.add_statement_event_listener(std::sync::Arc::clone(&statement_listener))?;
    assert!(pooled.remove_statement_event_listener(&statement_listener)?);
    assert_eq!(pooled.statement_listeners.len(), 1);
    assert!(pooled.remove_statement_event_listener(&statement_listener)?);
    assert!(!pooled.remove_statement_event_listener(&statement_listener)?);
    assert!(matches!(pooled.close().await, Err(RdbcError::SqlException(_))));
    let callable = ExternalCallable;
    assert!(callable.is_wrapper_for(Some(TypeId::of::<dyn CallableStatement>()))?);
    assert!(callable.is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))?);
    assert!(callable.is_wrapper_for(Some(TypeId::of::<dyn Statement>()))?);
    let generic = ExternalGeneric::<u8>(std::marker::PhantomData);
    assert!(generic.is_wrapper_for(Some(TypeId::of::<dyn Connection>()))?);
    Ok(())
}
"#,
    )
    .expect("external implementor source must be written");

    let output = Command::new(env!("CARGO"))
        .args(["run", "--offline", "--quiet"])
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .expect("external cargo check must execute");
    let _ = fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "external implementor failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
