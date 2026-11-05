//! 零散未评估 JDBC 表面的 Rust 入口合同。
//!
//! 本文件只断言 `druid-rdbc` 中已经存在的 Rust 入口的真实行为：
//! 有条目的断言精确的值、错误或对象身份；标准 trait 默认入口返回精确
//! unsupported operation 的，断言精确的 operation 名称。
use std::any::TypeId;
use std::sync::{Arc, Mutex};

use druid_rdbc::connection::Connection;
use druid_rdbc::data_source::{
    ConnectionEvent, ConnectionEventListener, PooledConnection, PooledConnectionSource,
    StatementEvent, StatementEventListener, StatementSource, XaConnection,
};
use druid_rdbc::driver::{ConnectionProperties, Driver, DriverPropertyInfo};
use druid_rdbc::error::{BatchUpdateException, RdbcError, SqlException, SqlExceptionCause};
use druid_rdbc::extensions::{
    BufferedSqlInput, BufferedSqlOutput, RowSetEvent, RowSetListener, SharedRowSet,
};
use druid_rdbc::resource::{SqlData, SqlInput, SqlOutput};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{PreparedStatement, Statement};
use druid_rdbc::types::{JdbcType, RowId, Savepoint, SqlType, Value};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};

// ── java.sql.Driver ──────────────────────────────────────────────────────────

/// 只声明 JDBC `Driver` 必须实现的三个入口，其余入口使用标准 trait 默认行为。
struct ContractDriver;

#[async_trait::async_trait]
impl Driver for ContractDriver {
    fn name(&self) -> &str {
        "contract-driver"
    }

    fn accepts_url(&self, _url: &str) -> bool {
        false
    }

    async fn connect(
        &self,
        _url: &str,
        _properties: &ConnectionProperties,
    ) -> Result<Option<Box<dyn Connection>>, RdbcError> {
        Ok(None)
    }
}

/// 覆盖版本号、JDBC 一致性声明、日志命名空间与 property 描述符的驱动。
struct VersionedDriver;

#[async_trait::async_trait]
impl Driver for VersionedDriver {
    fn name(&self) -> &str {
        "versioned-driver"
    }

    fn accepts_url(&self, url: &str) -> bool {
        url.starts_with("jdbc:contract:")
    }

    async fn connect(
        &self,
        _url: &str,
        _properties: &ConnectionProperties,
    ) -> Result<Option<Box<dyn Connection>>, RdbcError> {
        Ok(None)
    }

    fn property_info(
        &self,
        _url: &str,
        _properties: &ConnectionProperties,
    ) -> Result<Vec<DriverPropertyInfo>, RdbcError> {
        Ok(vec![DriverPropertyInfo::new(
            "ssl",
            Some("true".to_owned()),
        )])
    }

    fn major_version(&self) -> u32 {
        4
    }

    fn minor_version(&self) -> u32 {
        2
    }

    fn rdbc_compliant(&self) -> bool {
        true
    }

    fn parent_logger(&self) -> Result<&str, RdbcError> {
        Ok("app.driver")
    }
}

#[test]
fn driver_type_entry_is_a_public_object_safe_standard_trait() {
    fn accepts_object_safe_trait(_: &dyn Driver) {}
    fn is_send_and_sync<T: Send + Sync + 'static>() {}
    is_send_and_sync::<ContractDriver>();
    accepts_object_safe_trait(&ContractDriver);
    accepts_object_safe_trait(&VersionedDriver);
}

#[test]
fn driver_major_and_minor_version_entries_report_the_exact_version_numbers() {
    assert_eq!(ContractDriver.major_version(), 0);
    assert_eq!(ContractDriver.minor_version(), 0);
    assert_eq!(VersionedDriver.major_version(), 4);
    assert_eq!(VersionedDriver.minor_version(), 2);
}

#[test]
fn driver_parent_logger_entry_returns_the_exact_namespace() {
    assert_eq!(ContractDriver.parent_logger().unwrap(), "druid::rdbc");
    assert_eq!(ContractDriver.get_parent_logger().unwrap(), "druid::rdbc");
    assert_eq!(VersionedDriver.get_parent_logger().unwrap(), "app.driver");
}

#[test]
fn driver_property_info_entry_returns_the_exact_descriptors_for_the_url() {
    let properties = ConnectionProperties::new();
    assert!(ContractDriver
        .property_info("jdbc:contract:memory", &properties)
        .unwrap()
        .is_empty());
    let descriptors = VersionedDriver
        .property_info("jdbc:contract:memory", &properties)
        .unwrap();
    assert_eq!(descriptors.len(), 1);
    assert_eq!(descriptors[0].name, "ssl");
    assert_eq!(descriptors[0].value.as_deref(), Some("true"));
}

#[test]
fn driver_jdbc_compliance_entry_reports_the_declared_conformance() {
    assert!(!ContractDriver.rdbc_compliant());
    assert!(VersionedDriver.rdbc_compliant());
}

// ── java.sql.DriverPropertyInfo ─────────────────────────────────────────────

#[test]
fn driver_property_info_type_and_constructor_preserve_the_jdbc_defaults() {
    let property = DriverPropertyInfo::new("ssl", None);
    assert_eq!(property.name, "ssl");
    assert_eq!(property.value, None);
    assert_eq!(property.description, None);
    assert!(!property.required);
    assert!(property.choices.is_empty());

    let configured = DriverPropertyInfo::new("user", Some("druid".to_owned()));
    assert_eq!(configured.name, "user");
    assert_eq!(configured.value.as_deref(), Some("druid"));
    assert_eq!(
        configured,
        DriverPropertyInfo {
            name: "user".to_owned(),
            value: Some("druid".to_owned()),
            description: None,
            required: false,
            choices: Vec::new(),
        }
    );
}

// ── javax.sql.XAConnection / javax.sql.PooledConnection ──────────────────────

/// 不覆盖任何默认入口的 pooled owner：直接暴露标准 trait 的默认行为。
struct DefaultPoolOwner;

#[async_trait::async_trait]
impl PooledConnection for DefaultPoolOwner {
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        Err(RdbcError::PoolClosed)
    }
}

/// 覆盖监听器注册入口的 pooled owner：证明这些入口是真实可实现的扩展点。
struct SupportingPoolOwner {
    connection_listeners: Vec<Arc<dyn ConnectionEventListener>>,
    statement_listeners: Vec<Arc<dyn StatementEventListener>>,
}

#[async_trait::async_trait]
impl PooledConnection for SupportingPoolOwner {
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        Err(RdbcError::PoolClosed)
    }

    fn add_connection_event_listener(
        &mut self,
        listener: Arc<dyn ConnectionEventListener>,
    ) -> Result<(), RdbcError> {
        self.connection_listeners.push(listener);
        Ok(())
    }

    fn remove_connection_event_listener(
        &mut self,
        listener: &Arc<dyn ConnectionEventListener>,
    ) -> Result<bool, RdbcError> {
        let Some(index) = self
            .connection_listeners
            .iter()
            .position(|candidate| Arc::ptr_eq(candidate, listener))
        else {
            return Ok(false);
        };
        self.connection_listeners.remove(index);
        Ok(true)
    }

    fn add_statement_event_listener(
        &mut self,
        listener: Arc<dyn StatementEventListener>,
    ) -> Result<(), RdbcError> {
        self.statement_listeners.push(listener);
        Ok(())
    }

    fn remove_statement_event_listener(
        &mut self,
        listener: &Arc<dyn StatementEventListener>,
    ) -> Result<bool, RdbcError> {
        let Some(index) = self
            .statement_listeners
            .iter()
            .position(|candidate| Arc::ptr_eq(candidate, listener))
        else {
            return Ok(false);
        };
        self.statement_listeners.remove(index);
        Ok(true)
    }
}

/// 使用同一 pooled owner 的 XA 连接实现。
struct ContractXaConnection;

#[async_trait::async_trait]
impl PooledConnection for ContractXaConnection {
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        Err(RdbcError::PoolClosed)
    }
}

impl XaConnection for ContractXaConnection {
    fn xa_resource_name(&self) -> &str {
        "contract-xa-resource"
    }
}

#[test]
fn xa_connection_type_entry_is_the_standard_trait_extending_pooled_connection() {
    fn accepts_object_safe_trait(_: &dyn XaConnection) {}
    fn is_pooled_connection<T: PooledConnection + ?Sized>() {}
    is_pooled_connection::<dyn XaConnection>();
    accepts_object_safe_trait(&ContractXaConnection);
    assert_eq!(
        ContractXaConnection.xa_resource_name(),
        "contract-xa-resource"
    );
}

#[test]
fn xa_connection_inherited_pooled_entry_points_report_precise_unsupported_operations() {
    let mut connection = ContractXaConnection;
    let listener: Arc<dyn ConnectionEventListener> = Arc::new(NoopConnectionListener);
    let statement_listener: Arc<dyn StatementEventListener> = Arc::new(NoopStatementListener);

    assert_feature_not_supported(
        &connection
            .add_connection_event_listener(Arc::clone(&listener))
            .unwrap_err(),
        "pooled_connection_add_connection_event_listener",
    );
    assert_feature_not_supported(
        &connection
            .remove_connection_event_listener(&listener)
            .unwrap_err(),
        "pooled_connection_remove_connection_event_listener",
    );
    assert_feature_not_supported(
        &connection
            .add_statement_event_listener(Arc::clone(&statement_listener))
            .unwrap_err(),
        "pooled_connection_add_statement_event_listener",
    );
    assert_feature_not_supported(
        &connection
            .remove_statement_event_listener(&statement_listener)
            .unwrap_err(),
        "pooled_connection_remove_statement_event_listener",
    );
}

#[tokio::test]
async fn xa_connection_close_entry_reports_the_precise_unsupported_operation() {
    let mut connection = ContractXaConnection;
    assert_feature_not_supported(
        &connection.close().await.unwrap_err(),
        "pooled_connection_close",
    );
}

#[test]
fn pooled_connection_type_entry_is_the_standard_trait() {
    fn accepts_object_safe_trait(_: &dyn PooledConnection) {}
    accepts_object_safe_trait(&DefaultPoolOwner);
    accepts_object_safe_trait(&SupportingPoolOwner {
        connection_listeners: Vec::new(),
        statement_listeners: Vec::new(),
    });
}

#[test]
fn pooled_connection_statement_event_listener_entries_report_precise_unsupported_operations() {
    let mut owner = DefaultPoolOwner;
    let listener: Arc<dyn StatementEventListener> = Arc::new(NoopStatementListener);

    assert_feature_not_supported(
        &owner
            .add_statement_event_listener(Arc::clone(&listener))
            .unwrap_err(),
        "pooled_connection_add_statement_event_listener",
    );
    assert_feature_not_supported(
        &owner
            .remove_statement_event_listener(&listener)
            .unwrap_err(),
        "pooled_connection_remove_statement_event_listener",
    );
}

#[test]
fn pooled_connection_listener_entries_accept_overriding_implementations() {
    let mut owner = SupportingPoolOwner {
        connection_listeners: Vec::new(),
        statement_listeners: Vec::new(),
    };
    let connection_listener: Arc<dyn ConnectionEventListener> = Arc::new(NoopConnectionListener);
    let statement_listener: Arc<dyn StatementEventListener> = Arc::new(NoopStatementListener);

    owner
        .add_connection_event_listener(Arc::clone(&connection_listener))
        .unwrap();
    owner
        .add_statement_event_listener(Arc::clone(&statement_listener))
        .unwrap();
    assert!(owner
        .remove_connection_event_listener(&connection_listener)
        .unwrap());
    assert!(owner
        .remove_statement_event_listener(&statement_listener)
        .unwrap());
    assert!(!owner
        .remove_connection_event_listener(&connection_listener)
        .unwrap());
    assert!(!owner
        .remove_statement_event_listener(&statement_listener)
        .unwrap());
}

// ── javax.sql.ConnectionEventListener / javax.sql.StatementEventListener ─────

struct NoopConnectionListener;

impl ConnectionEventListener for NoopConnectionListener {
    fn connection_closed(&self, _event: &ConnectionEvent) {}

    fn connection_error_occurred(&self, _event: &ConnectionEvent) {}
}

struct NoopStatementListener;

impl StatementEventListener for NoopStatementListener {
    fn statement_closed(&self, _event: &StatementEvent) {}

    fn statement_error_occurred(&self, _event: &StatementEvent) {}
}

/// 记录回调收到的来源身份，用于验证监听器入口保留原事件对象。
struct RecordingListener {
    closed: Mutex<Vec<usize>>,
    errored: Mutex<Vec<usize>>,
    statement_closed: Mutex<Vec<usize>>,
    statement_errored: Mutex<Vec<usize>>,
}

impl ConnectionEventListener for RecordingListener {
    fn connection_closed(&self, event: &ConnectionEvent) {
        self.closed
            .lock()
            .unwrap()
            .push(Arc::as_ptr(event.source()) as usize);
    }

    fn connection_error_occurred(&self, event: &ConnectionEvent) {
        self.errored
            .lock()
            .unwrap()
            .push(Arc::as_ptr(event.source()) as usize);
    }
}

impl StatementEventListener for RecordingListener {
    fn statement_closed(&self, event: &StatementEvent) {
        self.statement_closed
            .lock()
            .unwrap()
            .push(Arc::as_ptr(event.source()) as usize);
    }

    fn statement_error_occurred(&self, event: &StatementEvent) {
        self.statement_errored
            .lock()
            .unwrap()
            .push(Arc::as_ptr(event.source()) as usize);
    }
}

/// 构造一个不记录任何事件的监听器实例，仅用于类型级断言。
fn empty_recording_listener() -> RecordingListener {
    RecordingListener {
        closed: Mutex::new(Vec::new()),
        errored: Mutex::new(Vec::new()),
        statement_closed: Mutex::new(Vec::new()),
        statement_errored: Mutex::new(Vec::new()),
    }
}

struct ContractPrepared;

druid_rdbc::impl_standard_wrapper!(ContractPrepared => PreparedStatement);

#[async_trait::async_trait]
impl Statement for ContractPrepared {
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl PreparedStatement for ContractPrepared {}

fn pool_source() -> PooledConnectionSource {
    Arc::new(tokio::sync::Mutex::new(Box::new(DefaultPoolOwner)))
}

fn statement_source() -> StatementSource {
    Arc::new(tokio::sync::Mutex::new(Box::new(ContractPrepared)))
}

#[test]
fn connection_event_listener_type_entry_is_the_standard_trait() {
    fn accepts_object_safe_trait(_: &dyn ConnectionEventListener) {}
    fn is_send_and_sync<T: Send + Sync + 'static>() {}
    is_send_and_sync::<RecordingListener>();
    accepts_object_safe_trait(&empty_recording_listener());
}

#[test]
fn connection_event_listener_callbacks_receive_the_original_event_source() {
    let source = pool_source();
    let exception = Arc::new(SqlException::new(
        1040,
        Some("08004".to_owned()),
        Some("capacity".to_owned()),
    ));
    let closed = ConnectionEvent::new(Arc::clone(&source));
    let errored =
        ConnectionEvent::with_sql_exception(Arc::clone(&source), Some(Arc::clone(&exception)));

    let listener = RecordingListener {
        closed: Mutex::new(Vec::new()),
        errored: Mutex::new(Vec::new()),
        statement_closed: Mutex::new(Vec::new()),
        statement_errored: Mutex::new(Vec::new()),
    };
    listener.connection_closed(&closed);
    listener.connection_error_occurred(&errored);

    let expected = Arc::as_ptr(&source) as usize;
    assert_eq!(*listener.closed.lock().unwrap(), vec![expected]);
    assert_eq!(*listener.errored.lock().unwrap(), vec![expected]);
    assert!(Arc::ptr_eq(errored.sql_exception().unwrap(), &exception));
}

#[test]
fn statement_event_listener_type_entry_is_the_standard_trait() {
    fn accepts_object_safe_trait(_: &dyn StatementEventListener) {}
    fn is_send_and_sync<T: Send + Sync + 'static>() {}
    is_send_and_sync::<RecordingListener>();
    accepts_object_safe_trait(&empty_recording_listener());
}

#[test]
fn statement_event_listener_callbacks_receive_the_original_event_source() {
    let source = pool_source();
    let statement = statement_source();
    let exception = Arc::new(SqlException::new(
        1040,
        Some("08004".to_owned()),
        Some("capacity".to_owned()),
    ));
    let closed = StatementEvent::new(Arc::clone(&source), Some(Arc::clone(&statement)));
    let errored = StatementEvent::with_sql_exception(
        Arc::clone(&source),
        Some(Arc::clone(&statement)),
        Some(Arc::clone(&exception)),
    );

    let listener = RecordingListener {
        closed: Mutex::new(Vec::new()),
        errored: Mutex::new(Vec::new()),
        statement_closed: Mutex::new(Vec::new()),
        statement_errored: Mutex::new(Vec::new()),
    };
    listener.statement_closed(&closed);
    listener.statement_error_occurred(&errored);

    let expected = Arc::as_ptr(&source) as usize;
    assert_eq!(*listener.statement_closed.lock().unwrap(), vec![expected]);
    assert_eq!(*listener.statement_errored.lock().unwrap(), vec![expected]);
    assert!(Arc::ptr_eq(closed.statement().unwrap(), &statement));
    assert!(Arc::ptr_eq(errored.sql_exception().unwrap(), &exception));
}

// ── java.sql.BatchUpdateException / java.sql.SQLException ────────────────────

#[test]
fn batch_update_exception_type_entry_is_the_canonical_value_type() {
    let exception = BatchUpdateException::new(
        SqlException::new(1, Some("23000".to_owned()), Some("constraint".to_owned())),
        vec![1, 2, 3],
    );
    assert_eq!(exception.update_counts(), vec![1, 2, 3]);
    assert_eq!(exception.large_update_counts(), [1, 2, 3]);
    assert_eq!(exception.exception.class_name(), "java.sql.SQLException");
}

#[test]
fn batch_update_exception_inherited_getters_report_the_embedded_exception_state() {
    let mut exception = BatchUpdateException::new(
        SqlException::new(23, Some("23505".to_owned()), Some("duplicate".to_owned())),
        vec![1],
    );
    assert_eq!(exception.exception.error_code(), 23);
    assert_eq!(exception.exception.sql_state(), Some("23505"));
    assert_eq!(exception.exception.message(), Some("duplicate"));
    assert!(exception.exception.next_exception().is_none());

    let following = SqlException::new(24, Some("23506".to_owned()), Some("next".to_owned()));
    exception.exception.set_next_exception(following);
    assert_eq!(
        exception
            .exception
            .next_exception()
            .map(SqlException::sql_state),
        Some(Some("23506"))
    );
}

#[test]
fn batch_update_exception_cause_iterator_reports_the_embedded_cause_chain() {
    let exception = BatchUpdateException::new(
        SqlException::new(1, Some("23000".to_owned()), Some("constraint".to_owned()))
            .with_cause(SqlExceptionCause::SocketTimeout)
            .with_cause(SqlExceptionCause::ClassName(
                "vendor.DriverError".to_owned(),
            )),
        vec![-3],
    );
    assert_eq!(
        exception.exception.causes(),
        [
            SqlExceptionCause::SocketTimeout,
            SqlExceptionCause::ClassName("vendor.DriverError".to_owned()),
        ]
    );
    assert_eq!(exception.update_counts(), vec![-3]);
}

#[test]
fn sql_exception_type_entry_is_the_canonical_value_type() {
    let exception = SqlException::new(0, Some("0A000".to_owned()), Some("nope".to_owned()));
    assert_eq!(exception.class_name(), "java.sql.SQLException");
    assert!(exception.is_instance_of("java.sql.SQLException"));
    assert_eq!(exception.error_code(), 0);
}

#[test]
fn sql_exception_cause_iterator_reports_the_cause_chain() {
    let exception = SqlException::new(0, Some("08006".to_owned()), Some("broken".to_owned()))
        .with_cause(SqlExceptionCause::SocketTimeout);
    assert_eq!(exception.causes(), [SqlExceptionCause::SocketTimeout]);
    assert!(!exception.causes().is_empty());
}

// ── java.sql.SQLData ─────────────────────────────────────────────────────────

#[derive(Default)]
struct ContractRecord {
    type_name: String,
    number: i32,
    label: Option<String>,
}

#[async_trait::async_trait]
impl SqlData for ContractRecord {
    async fn sql_type_name(&mut self) -> Result<String, RdbcError> {
        Ok(self.type_name.clone())
    }

    async fn read_sql(
        &mut self,
        input: &mut dyn SqlInput,
        type_name: &str,
    ) -> Result<(), RdbcError> {
        self.type_name = type_name.to_owned();
        self.number = input.read_int().await?;
        self.label = input.read_string().await?;
        Ok(())
    }

    async fn write_sql(&mut self, output: &mut dyn SqlOutput) -> Result<(), RdbcError> {
        output.write_int(self.number).await?;
        output.write_string(self.label.clone()).await
    }
}

#[test]
fn sql_data_type_entry_is_the_standard_trait() {
    fn accepts_object_safe_trait(_: &mut dyn SqlData) {}
    accepts_object_safe_trait(&mut ContractRecord::default());
}

#[tokio::test]
async fn sql_data_type_name_entry_returns_the_qualified_udt_name() {
    let mut record = ContractRecord::default();
    let data: &mut dyn SqlData = &mut record;
    assert_eq!(data.sql_type_name().await.unwrap(), "");
}

#[tokio::test]
async fn sql_data_read_and_write_entries_preserve_the_declared_attribute_order() {
    let mut input = BufferedSqlInput::new(vec![Value::Int(41).into(), Value::Null.into()]);
    let mut record = ContractRecord::default();
    let data: &mut dyn SqlData = &mut record;
    data.read_sql(&mut input, "app.record").await.unwrap();
    assert_eq!(data.sql_type_name().await.unwrap(), "app.record");

    let mut output = BufferedSqlOutput::new();
    data.write_sql(&mut output).await.unwrap();
    let mut round_trip = output.into_input();
    assert_eq!(round_trip.read_int().await.unwrap(), 41);
    assert!(!round_trip.was_null().unwrap());
    assert!(round_trip.read_string().await.unwrap().is_none());
    assert!(round_trip.was_null().unwrap());
}

// ── java.sql.SQLType ─────────────────────────────────────────────────────────

#[test]
fn sql_type_type_entry_is_the_standard_trait() {
    fn accepts_object_safe_trait(_: &dyn SqlType) {}
    accepts_object_safe_trait(&JdbcType::Integer);
}

#[test]
fn sql_type_name_vendor_and_type_number_entries_report_the_jdbc_values() {
    let sql_type: &dyn SqlType = &JdbcType::Integer;
    assert_eq!(sql_type.name(), "INTEGER");
    assert_eq!(sql_type.vendor(), "java.sql");
    assert_eq!(sql_type.vendor_type_number(), 4);

    let varchar: &dyn SqlType = &JdbcType::VarChar;
    assert_eq!(varchar.name(), "VARCHAR");
    assert_eq!(varchar.vendor_type_number(), 12);
}

// ── java.sql.Savepoint ───────────────────────────────────────────────────────

#[test]
fn savepoint_type_entry_is_the_canonical_value_type() {
    let savepoint = Savepoint { id: 7, name: None };
    assert_eq!(savepoint.id, 7);
    assert_eq!(savepoint.name, None);
}

#[test]
fn savepoint_id_and_name_entries_reject_the_opposite_savepoint_kind() {
    let anonymous = Savepoint { id: 7, name: None };
    assert_eq!(anonymous.get_savepoint_id().unwrap(), 7);
    let named_error = anonymous.get_savepoint_name().unwrap_err();
    assert_sql_exception(&named_error, "HY024");

    let named = Savepoint {
        id: 9,
        name: Some("sp1".to_owned()),
    };
    assert_eq!(named.get_savepoint_name().unwrap(), "sp1");
    let id_error = named.get_savepoint_id().unwrap_err();
    assert_sql_exception(&id_error, "HY024");
}

// ── java.sql.Wrapper ─────────────────────────────────────────────────────────

struct ContractResultSet;

druid_rdbc::impl_standard_wrapper!(ContractResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for ContractResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
}

#[test]
fn wrapper_type_entry_is_the_standard_trait() {
    fn accepts_object_safe_trait(_: &dyn Wrapper) {}
    accepts_object_safe_trait(&ContractResultSet);
}

#[test]
fn wrapper_is_wrapper_for_and_unwrap_entries_report_the_canonical_identity() {
    let result_set = ContractResultSet;
    let target = Some(TypeId::of::<dyn ResultSet>());
    assert!(result_set.is_wrapper_for(target).unwrap());

    match result_set.unwrap(target).unwrap() {
        StandardUnwrapped::ResultSet(inner) => {
            assert_eq!(
                std::ptr::from_ref::<dyn ResultSet>(inner).cast::<u8>(),
                std::ptr::from_ref(&result_set).cast::<u8>()
            );
        }
        _ => panic!("expected the canonical ResultSet view"),
    }

    struct UnknownTarget;
    let unknown = Some(TypeId::of::<UnknownTarget>());
    assert!(!result_set.is_wrapper_for(unknown).unwrap());
    assert_feature_not_supported(
        &result_set.unwrap(unknown).unwrap_err(),
        "wrapper_unwrap_target",
    );
    assert!(matches!(
        result_set.is_wrapper_for(None),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(matches!(
        result_set.unwrap(None),
        Err(RdbcError::InvalidArgument(_))
    ));
}

// ── java.sql.RowId ───────────────────────────────────────────────────────────

#[test]
fn row_id_type_entry_is_the_canonical_value_type() {
    let row_id = RowId::new(vec![1, 2, 3]);
    assert_eq!(row_id.bytes(), [1, 2, 3]);
    assert_eq!(row_id.get_bytes(), vec![1, 2, 3]);
}

#[test]
fn row_id_string_rendering_is_the_debug_form_of_the_stored_bytes() {
    let row_id = RowId::new(vec![1, 2, 3]);
    assert_eq!(format!("{row_id:?}"), "RdbcRowId { bytes: [1, 2, 3] }");
    assert_eq!(
        RowId::new(Vec::new()),
        RowId::new(Vec::new()),
        "empty RowId keeps byte identity"
    );
}

// ── java.sql.ResultSet ───────────────────────────────────────────────────────

#[test]
fn result_set_type_entry_is_the_standard_trait() {
    fn accepts_object_safe_trait(_: &dyn ResultSet) {}
    accepts_object_safe_trait(&ContractResultSet);
}

#[test]
fn result_set_get_statement_entry_reports_the_precise_unsupported_operation() {
    let result_set = ContractResultSet;
    let error = match result_set.get_statement() {
        Ok(_) => panic!("expected the precise unsupported operation"),
        Err(error) => error,
    };
    let RdbcError::UnsupportedOperation { operation } = error else {
        panic!("expected the precise unsupported operation, got {error:?}");
    };
    assert_eq!(operation, "result_set_get_statement");
}

// ── javax.sql.RowSetEvent / javax.sql.RowSetListener ─────────────────────────

struct RecordingRowSetListener {
    changed: Mutex<Vec<SharedRowSet>>,
}

impl RowSetListener for RecordingRowSetListener {
    fn row_set_changed(&self, _event: &RowSetEvent) {}

    fn row_changed(&self, event: &RowSetEvent) {
        self.changed.lock().unwrap().push(event.source());
    }

    fn cursor_moved(&self, _event: &RowSetEvent) {}
}

#[test]
fn row_set_event_type_entry_is_the_canonical_value_type() {
    let source = SharedRowSet::new();
    let event = RowSetEvent::new(Some(source.clone())).unwrap();
    assert_eq!(event.source(), source);
}

#[test]
fn row_set_event_constructor_keeps_the_source_identity_and_rejects_null() {
    assert!(matches!(
        RowSetEvent::new(None),
        Err(RdbcError::InvalidArgument(_))
    ));
    let source = SharedRowSet::new();
    let event = RowSetEvent::new(Some(source.clone())).unwrap();
    assert_eq!(event.source(), source);
}

#[test]
fn row_set_listener_type_entry_is_the_standard_trait() {
    fn accepts_object_safe_trait(_: &dyn RowSetListener) {}
    fn is_send_and_sync<T: Send + Sync + 'static>() {}
    is_send_and_sync::<RecordingRowSetListener>();
    accepts_object_safe_trait(&RecordingRowSetListener {
        changed: Mutex::new(Vec::new()),
    });
}

#[test]
fn row_set_listener_row_changed_callback_receives_the_original_event() {
    let source = SharedRowSet::new();
    let event = RowSetEvent::new(Some(source.clone())).unwrap();
    let listener = RecordingRowSetListener {
        changed: Mutex::new(Vec::new()),
    };
    listener.row_changed(&event);
    let changed = listener.changed.lock().unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0], source);
}

// ── 共享断言辅助 ─────────────────────────────────────────────────────────────

/// 断言标准 trait 默认入口返回精确的 feature-not-supported 错误。
fn assert_feature_not_supported(error: &RdbcError, operation: &str) {
    let RdbcError::SqlException(exception) = error else {
        panic!("expected a feature-not-supported SQL exception, got {error:?}");
    };
    assert_eq!(
        exception.class_name(),
        "java.sql.SQLFeatureNotSupportedException"
    );
    assert_eq!(exception.sql_state(), Some("0A000"));
    let expected = format!("RDBC operation is not supported: {operation}");
    assert_eq!(exception.message(), Some(expected.as_str()));
    assert!(exception.is_instance_of("java.sql.SQLNonTransientException"));
    assert_eq!(
        error.kind(),
        druid_rdbc::error::RdbcErrorKind::SqlFeatureNotSupported
    );
}

/// 断言错误是携带精确 SQLState 的标准 SQL 异常。
fn assert_sql_exception(error: &RdbcError, sql_state: &str) {
    let RdbcError::SqlException(exception) = error else {
        panic!("expected a SQL exception, got {error:?}");
    };
    assert_eq!(exception.sql_state(), Some(sql_state));
}
