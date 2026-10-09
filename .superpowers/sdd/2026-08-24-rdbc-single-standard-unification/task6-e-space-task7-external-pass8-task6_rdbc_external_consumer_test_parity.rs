//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMPORARY_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TemporaryPackage {
    path: PathBuf,
}

impl TemporaryPackage {
    fn create() -> Result<Self, Box<dyn Error>> {
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let sequence = TEMPORARY_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "druid-rdbc-external-consumer-{}-{elapsed}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TemporaryPackage {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.path);
    }
}

fn toml_path(path: &Path) -> Result<String, Box<dyn Error>> {
    let path = path
        .to_str()
        .ok_or("druid package path is not valid UTF-8")?;
    Ok(path.replace('\\', "\\\\").replace('"', "\\\""))
}

// 验证独立包仍可通过既有 druid::sql::* 公共路径编译。

// 验证结构化 SQL 异常与警告在旧、新路径之间保持同一类型和完整字段语义。

// 新 Druid cache 对象只能是 crate-private；外部仅保留 Task 6 旧 facade。

#[tokio::test]
async fn task6_rdbc_external_consumer_test_external_consumer_retains_druid_sql_public_contract(
) -> Result<(), Box<dyn Error>> {
    let package = TemporaryPackage::create()?;
    let druid_path = Path::new(env!("CARGO_MANIFEST_DIR")).canonicalize()?;
    let druid_rdbc_path = druid_path
        .parent()
        .ok_or("druid package has no workspace crates directory")?
        .join("druid-rdbc")
        .canonicalize()?;
    let manifest = format!(
        "[package]\nname = \"rdbc-external-consumer-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[workspace]\n\n[dependencies]\ndruid = {{ path = \"{}\" }}\ndruid-rdbc = {{ path = \"{}\" }}\n",
        toml_path(&druid_path)?,
        toml_path(&druid_rdbc_path)?,
    );
    let source = r#"use druid::sql::{Connection, DataSource, Date, ResultSet, SQLException, Statement};

macro_rules! bidirectional_owned_identity {
    ($old:path, $new:path) => {{
        let old_value: Option<$old> = None;
        let new_value: Option<$new> = old_value;
        let _old_value: Option<$old> = new_value;
    }};
}

macro_rules! bidirectional_dyn_identity {
    ($old:path, $new:path) => {{
        fn old_to_new(value: &dyn $old) -> &dyn $new {
            value
        }
        fn new_to_old(value: &dyn $new) -> &dyn $old {
            value
        }
        let _ = old_to_new;
        let _ = new_to_old;
    }};
}

macro_rules! bidirectional_arc_dyn_identity {
    ($old:path, $new:path) => {{
        let old_value: Option<std::sync::Arc<dyn $old>> = None;
        let new_value: Option<std::sync::Arc<dyn $new>> = old_value;
        let _old_value: Option<std::sync::Arc<dyn $old>> = new_value;
    }};
}

macro_rules! bidirectional_box_dyn_identity {
    ($old:path, $new:path) => {{
        let old_value: Option<Box<dyn $old>> = None;
        let new_value: Option<Box<dyn $new>> = old_value;
        let _old_value: Option<Box<dyn $old>> = new_value;
    }};
}

macro_rules! bidirectional_path_identity {
    ($old:path, $new:path) => {{
        fn old_path<T: $old + ?Sized>() {}
        fn new_path<T: $new + ?Sized>() {}
        let _ = old_path::<dyn druid_rdbc::wrapper::Wrapper>;
        let _ = new_path::<dyn druid_rdbc::wrapper::Wrapper>;
    }};
}

fn accepts_connection(_: Option<Connection>) {}
fn accepts_result_set(_: Option<ResultSet>) {}

fn main() {
    accepts_connection(None);
    accepts_result_set(None);
    let _ = std::any::TypeId::of::<Date>();

    bidirectional_owned_identity!(druid::core::Value, druid_rdbc::types::Value);
    bidirectional_owned_identity!(druid::core::Value, druid_rdbc::types::Value);
    bidirectional_owned_identity!(druid::core::Row, druid_rdbc::types::Row);
    bidirectional_owned_identity!(druid::core::Row, druid_rdbc::types::Row);
    bidirectional_owned_identity!(druid::core::Row, druid_rdbc::types::Row);
    bidirectional_owned_identity!(druid::core::RdbcUrl, druid_rdbc::types::RdbcUrl);
    bidirectional_owned_identity!(druid::core::RdbcUrl, druid_rdbc::types::RdbcUrl);
    bidirectional_owned_identity!(druid::sql::Types, druid_rdbc::types::Types);
    bidirectional_owned_identity!(druid::sql::Types, druid_rdbc::types::Types);
    bidirectional_owned_identity!(druid::sql::RdbcType, druid_rdbc::types::RdbcType);
    bidirectional_owned_identity!(druid::sql::RdbcType, druid_rdbc::types::RdbcType);
    bidirectional_owned_identity!(druid::sql::RDBCType, druid_rdbc::types::RdbcType);
    bidirectional_dyn_identity!(druid::sql::SQLType, druid_rdbc::types::SqlType);
    bidirectional_dyn_identity!(druid::sql::SQLType, druid_rdbc::types::SqlType);

    bidirectional_owned_identity!(
        druid::core::SqlExceptionCause,
        druid_rdbc::error::SqlExceptionCause
    );
    bidirectional_owned_identity!(druid::core::SqlExceptionCause, druid_rdbc::error::SqlExceptionCause);
    bidirectional_owned_identity!(
        druid::core::SqlExceptionKind,
        druid_rdbc::error::SqlExceptionKind
    );
    bidirectional_owned_identity!(druid::core::SqlExceptionKind, druid_rdbc::error::SqlExceptionKind);
    bidirectional_owned_identity!(
        druid::sql::SqlExceptionKind,
        druid_rdbc::error::SqlExceptionKind
    );
    bidirectional_owned_identity!(druid::sql::SqlExceptionKind, druid_rdbc::error::SqlExceptionKind);

    bidirectional_owned_identity!(
        druid::core::SqlException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::core::SqlException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::SqlException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(druid::sql::SQLException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::SqlTransientException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlTransientException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::SqlTransientConnectionException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlTransientConnectionException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlNonTransientException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlNonTransientException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlNonTransientConnectionException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlNonTransientConnectionException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlRecoverableException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlRecoverableException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::SqlDataException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlDataException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::SqlIntegrityConstraintViolationException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlIntegrityConstraintViolationException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlInvalidAuthorizationSpecException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlInvalidAuthorizationSpecException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlSyntaxErrorException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlSyntaxErrorException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::SqlTransactionRollbackException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlTransactionRollbackException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlFeatureNotSupportedException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlFeatureNotSupportedException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlTimeoutException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlTimeoutException, druid_rdbc::error::SqlException);

    bidirectional_owned_identity!(
        druid::core::SqlWarning,
        druid_rdbc::error::SqlWarning
    );
    bidirectional_owned_identity!(druid::core::SqlWarning, druid_rdbc::error::SqlWarning);
    bidirectional_owned_identity!(
        druid::sql::SqlWarning,
        druid_rdbc::error::SqlWarning
    );
    bidirectional_owned_identity!(druid::sql::SqlWarning, druid_rdbc::error::SqlWarning);
    bidirectional_owned_identity!(druid::sql::SQLWarning, druid_rdbc::error::SqlWarning);

    assert_ne!(
        std::any::TypeId::of::<druid::DruidError>(),
        std::any::TypeId::of::<druid_rdbc::RdbcError>()
    );
    let canonical_error = druid_rdbc::RdbcError::Other("driver boundary".to_string());
    let product_error: druid::DruidError = canonical_error.clone().into();
    match &product_error {
        druid::DruidError::Rdbc(wrapped) => assert_eq!(wrapped, &canonical_error),
        other => panic!("expected one explicit DruidError::Rdbc wrapper, got {other:?}"),
    }
    let recovered: druid_rdbc::RdbcError = product_error.into();
    assert_eq!(recovered, canonical_error);
    bidirectional_owned_identity!(
        druid::core::RdbcString,
        druid_rdbc::types::RdbcString
    );
    bidirectional_owned_identity!(druid::core::RdbcString, druid_rdbc::types::RdbcString);
    bidirectional_owned_identity!(druid::sql::RdbcString, druid_rdbc::types::RdbcString);
    bidirectional_owned_identity!(
        druid::core::Savepoint,
        druid_rdbc::types::Savepoint
    );
    bidirectional_owned_identity!(druid::core::Savepoint, druid_rdbc::types::Savepoint);
    bidirectional_owned_identity!(druid::sql::Savepoint, druid_rdbc::types::Savepoint);
    bidirectional_owned_identity!(druid::sql::Savepoint, druid_rdbc::types::Savepoint);
    bidirectional_owned_identity!(druid::sql::Date, druid_rdbc::types::Date);
    bidirectional_owned_identity!(druid::sql::Date, druid_rdbc::types::Date);
    bidirectional_owned_identity!(druid::sql::Time, druid_rdbc::types::Time);
    bidirectional_owned_identity!(druid::sql::Time, druid_rdbc::types::Time);
    bidirectional_owned_identity!(
        druid::sql::Timestamp,
        druid_rdbc::types::Timestamp
    );
    bidirectional_owned_identity!(druid::sql::Timestamp, druid_rdbc::types::Timestamp);

    bidirectional_owned_identity!(
        druid::sql::BatchUpdateException,
        druid_rdbc::compat::BatchUpdateException
    );
    bidirectional_owned_identity!(
        druid::sql::BatchUpdateException,
        druid_rdbc::compat::BatchUpdateException
    );
    bidirectional_owned_identity!(
        druid::sql::DataTruncation,
        druid_rdbc::compat::DataTruncation
    );
    bidirectional_owned_identity!(druid::sql::DataTruncation, druid_rdbc::compat::DataTruncation);
    bidirectional_owned_identity!(
        druid::sql::SqlClientInfoException,
        druid_rdbc::compat::SqlClientInfoException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlClientInfoException,
        druid_rdbc::compat::SqlClientInfoException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlTransientException,
        druid_rdbc::compat::SqlTransientException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlTransientConnectionException,
        druid_rdbc::compat::SqlTransientConnectionException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlNonTransientException,
        druid_rdbc::compat::SqlNonTransientException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlNonTransientConnectionException,
        druid_rdbc::compat::SqlNonTransientConnectionException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlRecoverableException,
        druid_rdbc::compat::SqlRecoverableException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlDataException,
        druid_rdbc::compat::SqlDataException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlIntegrityConstraintViolationException,
        druid_rdbc::compat::SqlIntegrityConstraintViolationException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlInvalidAuthorizationSpecException,
        druid_rdbc::compat::SqlInvalidAuthorizationSpecException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlSyntaxErrorException,
        druid_rdbc::compat::SqlSyntaxErrorException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlTransactionRollbackException,
        druid_rdbc::compat::SqlTransactionRollbackException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlFeatureNotSupportedException,
        druid_rdbc::compat::SqlFeatureNotSupportedException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlTimeoutException,
        druid_rdbc::compat::SqlTimeoutException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlExceptionKind,
        druid_rdbc::compat::SqlExceptionKind
    );
    bidirectional_owned_identity!(
        druid::sql::SqlException,
        druid_rdbc::compat::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlWarning,
        druid_rdbc::compat::SqlWarning
    );

    bidirectional_owned_identity!(druid::core::ExecResult, druid::core::ExecResult);
    bidirectional_owned_identity!(druid::core::ExecResult, druid::core::ExecResult);
    bidirectional_owned_identity!(
        druid::core::ExecResult,
        druid::core::ExecResult
    );
    bidirectional_owned_identity!(druid::core::StandardUnwrapped, druid::core::StandardUnwrapped);
    bidirectional_owned_identity!(druid::sql::StandardUnwrapped, druid::core::StandardUnwrapped);
    bidirectional_dyn_identity!(druid::core::Wrapper, druid::core::Wrapper);
    bidirectional_dyn_identity!(druid::sql::Wrapper, druid::core::Wrapper);
    bidirectional_path_identity!(druid::core::WrapperExt, druid::core::WrapperExt);
    bidirectional_path_identity!(druid::sql::WrapperExt, druid::core::WrapperExt);

    bidirectional_owned_identity!(
        druid::core::RowIdLifetime,
        druid::core::RowIdLifetime
    );
    bidirectional_owned_identity!(
        druid::core::ResultSetNullability,
        druid::core::ResultSetNullability
    );
    bidirectional_owned_identity!(
        druid::core::CallableInputParameter,
        druid::core::CallableInputParameter
    );
    bidirectional_owned_identity!(
        druid::core::CallableOutParameter,
        druid::core::CallableOutParameter
    );
    bidirectional_owned_identity!(
        druid::core::CallableParameter,
        druid::core::CallableParameter
    );
    bidirectional_owned_identity!(
        druid::core::ResultSetUpdate,
        druid::core::ResultSetUpdate
    );

    bidirectional_owned_identity!(
        druid::core::RdbcCalendar,
        druid_rdbc::types::RdbcCalendar
    );
    bidirectional_owned_identity!(
        druid::core::RdbcCalendarArgument,
        druid_rdbc::types::RdbcCalendarArgument
    );
    bidirectional_owned_identity!(druid::core::RdbcObject, druid_rdbc::types::RdbcObject);
    bidirectional_dyn_identity!(
        druid::core::OpaqueObjectValue,
        druid_rdbc::types::OpaqueObjectValue
    );
    bidirectional_owned_identity!(
        druid::core::RdbcOpaqueObject,
        druid_rdbc::types::RdbcOpaqueObject
    );
    bidirectional_owned_identity!(druid::core::RdbcRowId, druid_rdbc::types::RdbcRowId);
    bidirectional_owned_identity!(
        druid::core::RdbcTargetType,
        druid_rdbc::types::RdbcTargetType
    );
    bidirectional_owned_identity!(
        druid::core::RdbcTypeMap,
        druid_rdbc::types::RdbcTypeMap
    );
    bidirectional_dyn_identity!(
        druid::core::RdbcParameter,
        druid_rdbc::types::RdbcParameter
    );
    bidirectional_owned_identity!(
        druid::core::RdbcParameterType,
        druid_rdbc::types::RdbcParameterType
    );
    bidirectional_owned_identity!(
        druid::core::RdbcParameterValue,
        druid_rdbc::types::RdbcParameterValue
    );
    bidirectional_owned_identity!(druid::core::RdbcParameterDate, druid_rdbc::types::RdbcParameterDate);
    bidirectional_owned_identity!(druid::core::RdbcParameterDecimal, druid_rdbc::types::RdbcParameterDecimal);
    bidirectional_owned_identity!(druid::core::RdbcParameterImpl, druid_rdbc::types::RdbcParameterImpl);
    bidirectional_owned_identity!(druid::core::RdbcParameterInt, druid_rdbc::types::RdbcParameterInt);
    bidirectional_owned_identity!(druid::core::RdbcParameterLong, druid_rdbc::types::RdbcParameterLong);
    bidirectional_owned_identity!(druid::core::RdbcParameterNull, druid_rdbc::types::RdbcParameterNull);
    bidirectional_owned_identity!(druid::core::RdbcParameterString, druid_rdbc::types::RdbcParameterString);
    bidirectional_owned_identity!(druid::core::RdbcParameterTimestamp, druid_rdbc::types::RdbcParameterTimestamp);

    bidirectional_owned_identity!(druid::core::RdbcXmlRepresentationType, druid_rdbc::resource::RdbcXmlRepresentationType);
    bidirectional_dyn_identity!(druid::core::XmlResult, druid_rdbc::extensions::XmlResult);
    bidirectional_owned_identity!(druid::core::RdbcXmlResult, druid_rdbc::resource::RdbcXmlResult);
    bidirectional_dyn_identity!(druid::core::XmlSource, druid_rdbc::extensions::XmlSource);
    bidirectional_owned_identity!(druid::core::RdbcXmlSource, druid_rdbc::resource::RdbcXmlSource);

    bidirectional_arc_dyn_identity!(druid::core::ConnectionFactory, druid::core::ConnectionFactory);
    bidirectional_arc_dyn_identity!(
        druid::core::ConnectionFactory,
        druid::core::ConnectionFactory
    );
    bidirectional_arc_dyn_identity!(
        druid::core::ConnectionFactory,
        druid::core::ConnectionFactory
    );
    bidirectional_box_dyn_identity!(druid::core::Connection, druid::core::Connection);
    bidirectional_box_dyn_identity!(
        druid::core::Connection,
        druid::core::Connection
    );
    bidirectional_box_dyn_identity!(
        druid::core::Connection,
        druid::core::Connection
    );
    bidirectional_dyn_identity!(druid::core::Statement, druid::core::Statement);
    bidirectional_dyn_identity!(druid::core::PreparedStatement, druid::core::PreparedStatement);
    bidirectional_dyn_identity!(druid::core::CallableStatement, druid::core::CallableStatement);
    bidirectional_dyn_identity!(druid::core::DatabaseMetaData, druid::core::DatabaseMetaData);
    bidirectional_dyn_identity!(druid::core::ResultSetMetaData, druid::core::ResultSetMetaData);
    bidirectional_dyn_identity!(druid::core::ResultSet, druid::core::ResultSet);
    bidirectional_owned_identity!(druid_rdbc::extensions::ConnectionCapabilities, druid_rdbc::extensions::ConnectionCapabilities);
    bidirectional_owned_identity!(druid::core::StatementOptions, druid::core::StatementOptions);
    bidirectional_owned_identity!(druid::core::StatementGeneratedKeys, druid::core::StatementGeneratedKeys);
    bidirectional_owned_identity!(druid::core::StatementExecuteResult, druid::core::StatementExecuteResult);
    bidirectional_dyn_identity!(druid::core::ResultSet, druid::core::ResultSet);
    bidirectional_owned_identity!(druid::core::PreparedInputParameter, druid::core::PreparedInputParameter);
    bidirectional_owned_identity!(druid::core::PreparedTypeNameArgument, druid::core::PreparedTypeNameArgument);
    bidirectional_dyn_identity!(druid::core::ResultSetMetaData, druid::core::ResultSetMetaData);
    bidirectional_dyn_identity!(druid::sql::ResultSetMetaData, druid::core::ResultSetMetaData);
    bidirectional_dyn_identity!(druid::sql::ResultSetMetaData, druid::core::ResultSetMetaData);
    bidirectional_owned_identity!(druid::core::ResultSetColumnMeta, druid::core::ResultSetColumnMeta);
    bidirectional_owned_identity!(druid::core::ResultSetColumnType, druid::core::ResultSetColumnType);

    bidirectional_owned_identity!(druid::core::RdbcArray, druid_rdbc::resource::RdbcArray);
    bidirectional_owned_identity!(druid::core::RdbcBlob, druid_rdbc::resource::RdbcBlob);
    bidirectional_owned_identity!(druid::core::RdbcClob, druid_rdbc::resource::RdbcClob);
    bidirectional_owned_identity!(druid::core::RdbcNClob, druid_rdbc::resource::RdbcNClob);
    bidirectional_owned_identity!(druid::core::RdbcRef, druid_rdbc::resource::RdbcRef);
        let _: fn(Box<dyn druid_rdbc::resource::Array>) -> druid_rdbc::resource::RdbcArray = druid::core::ResourceHandleFactory::array;
    let _: fn(Box<dyn druid_rdbc::resource::Blob>) -> druid_rdbc::resource::RdbcBlob = druid::core::ResourceHandleFactory::blob;
    let _: fn(Box<dyn druid_rdbc::resource::Clob>) -> druid_rdbc::resource::RdbcClob = druid::core::ResourceHandleFactory::clob;
    let _: fn(Box<dyn druid_rdbc::resource::NClob>) -> druid_rdbc::resource::RdbcNClob = druid::core::ResourceHandleFactory::n_clob;
    let _: fn(Box<dyn druid_rdbc::resource::Ref>) -> druid_rdbc::resource::RdbcRef = druid::core::ResourceHandleFactory::reference;
    let _: fn(Box<dyn druid_rdbc::resource::SqlXml>) -> druid_rdbc::resource::RdbcSqlXml = druid::core::ResourceHandleFactory::sql_xml;
    bidirectional_owned_identity!(druid::core::RdbcSqlXml, druid_rdbc::resource::RdbcSqlXml);
    bidirectional_owned_identity!(druid::core::ResourceOperations, druid_rdbc::resource::ResourceOperations);
    bidirectional_owned_identity!(druid::core::ResourceHandleFactory, druid_rdbc::resource::ResourceHandleFactory);

    bidirectional_owned_identity!(druid::sql::RdbcArray, druid_rdbc::resource::RdbcArray);
    bidirectional_dyn_identity!(druid::sql::Array, druid_rdbc::resource::Array);
    bidirectional_owned_identity!(druid::sql::RdbcBlob, druid_rdbc::resource::RdbcBlob);
    bidirectional_dyn_identity!(druid::sql::Blob, druid_rdbc::resource::Blob);
    bidirectional_owned_identity!(druid::sql::RdbcClob, druid_rdbc::resource::RdbcClob);
    bidirectional_dyn_identity!(druid::sql::Clob, druid_rdbc::resource::Clob);
    bidirectional_owned_identity!(druid::sql::RdbcNClob, druid_rdbc::resource::RdbcNClob);
    bidirectional_dyn_identity!(druid::sql::NClob, druid_rdbc::resource::NClob);
    bidirectional_owned_identity!(druid::sql::RdbcRef, druid_rdbc::resource::RdbcRef);
    bidirectional_dyn_identity!(druid::sql::Ref, druid_rdbc::resource::Ref);
    bidirectional_owned_identity!(druid::sql::RdbcSqlXml, druid_rdbc::resource::RdbcSqlXml);
    bidirectional_dyn_identity!(druid::sql::SQLXML, druid_rdbc::resource::SqlXml);
    bidirectional_owned_identity!(druid::core::RdbcInputStream, druid_rdbc::resource::RdbcInputStream);
    bidirectional_owned_identity!(druid::core::RdbcStreamLength, druid_rdbc::resource::RdbcStreamLength);
    bidirectional_owned_identity!(druid::core::RdbcOutputStream, druid_rdbc::resource::RdbcOutputStream);
    bidirectional_dyn_identity!(druid::core::CharacterReader, druid_rdbc::extensions::CharacterReader);
    bidirectional_owned_identity!(druid::core::RdbcReader, druid_rdbc::resource::RdbcReader);
    bidirectional_owned_identity!(druid::core::RdbcCharacterLength, druid_rdbc::resource::RdbcCharacterLength);
    bidirectional_dyn_identity!(druid::core::CharacterWriter, druid_rdbc::extensions::CharacterWriter);
    bidirectional_owned_identity!(druid::core::RdbcWriter, druid_rdbc::resource::RdbcWriter);
    bidirectional_owned_identity!(druid::sql::RowId, druid_rdbc::types::RdbcRowId);
    bidirectional_owned_identity!(druid::sql::RowId, druid_rdbc::types::RdbcRowId);
    bidirectional_owned_identity!(
        druid::sql::RowIdLifetime,
        druid::core::RowIdLifetime
    );
    bidirectional_owned_identity!(
        druid::sql::RowIdLifetime,
        druid::core::RowIdLifetime
    );
    let _: Option<druid_rdbc::error::RdbcError> = None;

    mod sql_glob_contract {
        use druid::sql::*;

        pub fn proves_glob_alias_identity(
            exception: Option<SQLException>,
            warning: Option<SQLWarning>,
            rdbc_type: Option<RDBCType>,
            types: Option<Types>,
        ) -> (
            Option<druid_rdbc::error::SqlException>,
            Option<druid_rdbc::error::SqlWarning>,
            Option<druid_rdbc::types::RdbcType>,
            Option<druid_rdbc::types::Types>,
        ) {
            (exception, warning, rdbc_type, types)
        }
    }
    let _ = sql_glob_contract::proves_glob_alias_identity(None, None, None, None);
}
"#;
    fs::write(package.path.join("Cargo.toml"), manifest)?;
    fs::create_dir(package.path.join("src"))?;
    fs::write(package.path.join("src/main.rs"), source)?;

    let output = Command::new(env!("CARGO"))
        .arg("run")
        .arg("--quiet")
        .current_dir(&package.path)
        .env("CARGO_TARGET_DIR", package.path.join("target"))
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "external consumer cargo check failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )
        .into());
    }

    let factory = druid_wrapper::sqlx::SqlxConnectionFactory::new("sqlite::memory:");
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory).await?;
    let mut statement = druid_rdbc::connection::Connection::create_statement(
        &mut *connection,
        druid_rdbc::statement::StatementOptions::default(),
    )
    .await?;
    assert_eq!(
        druid_rdbc::statement::Statement::execute_update(
            &mut *statement,
            "CREATE TABLE canonical_move(id INTEGER PRIMARY KEY, value TEXT)",
            druid_rdbc::statement::GeneratedKeys::None,
        )
        .await?,
        0
    );
    let mut prepared = druid_rdbc::connection::Connection::prepare_statement(
        &mut *connection,
        "INSERT INTO canonical_move(id, value) VALUES (?, ?)",
        druid_rdbc::statement::StatementOptions::default(),
    )
    .await?;
    assert_eq!(
        druid_rdbc::statement::PreparedStatement::sql(&*prepared)?,
        "INSERT INTO canonical_move(id, value) VALUES (?, ?)"
    );
    druid_rdbc::statement::PreparedStatement::set_parameter(
        &mut *prepared,
        druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1)?,
            value: druid_rdbc::types::Value::Int(1),
        },
    )?;
    druid_rdbc::statement::PreparedStatement::set_parameter(
        &mut *prepared,
        druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(2)?,
            value: druid_rdbc::types::Value::String("factory-return".to_owned()),
        },
    )?;
    assert_eq!(
        druid_rdbc::statement::PreparedStatement::execute_prepared_update(&mut *prepared).await?,
        1
    );
    druid_rdbc::statement::Statement::close(&mut *prepared).await?;
    druid_rdbc::statement::Statement::close(&mut *statement).await?;
    druid_rdbc::connection::Connection::close(&mut *connection).await?;
    Ok(())
}

#[test]
fn task6_rdbc_external_consumer_test_error_identity() {
    let mut exception = druid_rdbc::error::SqlException::new(
        1205,
        Some("40001".to_string()),
        Some("deadlock victim".to_string()),
    )
    .with_class_name("vendor.DeadlockException")
    .with_assignable_type("vendor.DatabaseException")
    .with_cause(druid_rdbc::error::SqlExceptionCause::ClassName(
        "vendor.RootCause".to_string(),
    ));
    exception.set_next_exception(druid_rdbc::error::SqlException::driver(
        1213,
        "retry transaction",
    ));

    let old_leaf: druid::core::SqlException = exception;
    let old_parent: druid::core::SqlException = old_leaf;
    let old_sql_alias: druid::sql::SQLException = old_parent;
    let canonical: druid_rdbc::error::SqlException = old_sql_alias;

    assert_eq!(canonical.error_code(), 1205);
    assert_eq!(canonical.sql_state(), Some("40001"));
    assert_eq!(canonical.message(), Some("deadlock victim"));
    assert_eq!(canonical.class_name(), "vendor.DeadlockException");
    assert!(canonical.is_instance_of("vendor.DatabaseException"));
    assert_eq!(
        canonical.causes(),
        &[druid_rdbc::error::SqlExceptionCause::ClassName(
            "vendor.RootCause".to_string(),
        )]
    );
    assert_eq!(
        canonical
            .next_exception()
            .and_then(druid_rdbc::error::SqlException::message),
        Some("retry transaction")
    );

    let mut warning =
        druid_rdbc::error::SqlWarning::new("plan fallback", Some("01000".to_string()), 7);
    warning.set_next_warning(druid_rdbc::error::SqlWarning::new(
        "second warning",
        None,
        8,
    ));
    let old_warning: druid::core::SqlWarning = warning;
    let old_sql_warning: druid::sql::SQLWarning = old_warning;
    let canonical_warning: druid_rdbc::error::SqlWarning = old_sql_warning;

    assert_eq!(canonical_warning.message(), "plan fallback");
    assert_eq!(canonical_warning.sql_state(), Some("01000"));
    assert_eq!(canonical_warning.error_code(), 7);
    assert_eq!(
        canonical_warning
            .next_warning()
            .map(druid_rdbc::error::SqlWarning::message),
        Some("second warning")
    );
}

#[test]
fn task6_rdbc_external_consumer_test_external_consumer_cannot_name_druid_private_cache_types(
) -> Result<(), Box<dyn Error>> {
    let package = TemporaryPackage::create()?;
    let druid_path = Path::new(env!("CARGO_MANIFEST_DIR")).canonicalize()?;
    let manifest = format!(
        "[package]\nname = \"druid-private-cache-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[workspace]\n\n[dependencies]\ndruid = {{ path = \"{}\" }}\n",
        toml_path(&druid_path)?,
    );
    let source = r"use druid::core::PreparedStatementKey;
use druid::core::PreparedStatementMethodType;

fn main() {
    let _ = std::any::TypeId::of::<PreparedStatementKey>();
    let _ = std::any::TypeId::of::<PreparedStatementMethodType>();
}
";
    fs::create_dir(package.path.join("src"))?;
    fs::write(package.path.join("Cargo.toml"), manifest)?;
    fs::write(package.path.join("src/main.rs"), source)?;
    let output = Command::new(env!("CARGO"))
        .arg("check")
        .arg("--quiet")
        .current_dir(&package.path)
        .env("CARGO_TARGET_DIR", package.path.join("target"))
        .output()?;
    assert!(
        !output.status.success(),
        "private cache type probe unexpectedly compiled"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("PreparedStatementKey"));
    assert!(stderr.contains("PreparedStatementMethodType"));
    assert!(stderr.contains("private") || stderr.contains("unresolved import"));
    Ok(())
}
