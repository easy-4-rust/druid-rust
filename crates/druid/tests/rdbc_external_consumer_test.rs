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

/// 验证独立包仍可通过既有 druid::sql::* 公共路径编译。
#[test]
fn external_consumer_retains_druid_sql_public_contract() -> Result<(), Box<dyn Error>> {
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
    let source = r"use druid::sql::{Connection, DataSource, Date, ResultSet, SQLException, Statement};

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
        let _ = old_path::<druid::core::wrapper_adapter::WrapperAdapter>;
        let _ = new_path::<druid::core::wrapper_adapter::WrapperAdapter>;
    }};
}

fn accepts_connection(_: Option<Connection>) {}
fn accepts_result_set(_: Option<ResultSet>) {}

fn main() {
    accepts_connection(None);
    accepts_result_set(None);
    let _ = std::any::TypeId::of::<Date>();

    bidirectional_owned_identity!(druid::core::value::Value, druid_rdbc::types::Value);
    bidirectional_owned_identity!(druid::core::Value, druid_rdbc::types::Value);
    bidirectional_owned_identity!(druid::core::row::Row, druid_rdbc::types::Row);
    bidirectional_owned_identity!(druid::core::Row, druid_rdbc::types::Row);
    bidirectional_owned_identity!(druid::core::connection::Row, druid_rdbc::types::Row);
    bidirectional_owned_identity!(druid::core::rdbc_url::RdbcUrl, druid_rdbc::types::RdbcUrl);
    bidirectional_owned_identity!(druid::core::RdbcUrl, druid_rdbc::types::RdbcUrl);
    bidirectional_owned_identity!(druid::sql::types::Types, druid_rdbc::types::Types);
    bidirectional_owned_identity!(druid::sql::Types, druid_rdbc::types::Types);
    bidirectional_owned_identity!(druid::sql::rdbc_type::RdbcType, druid_rdbc::types::RdbcType);
    bidirectional_owned_identity!(druid::sql::RdbcType, druid_rdbc::types::RdbcType);
    bidirectional_owned_identity!(druid::sql::RDBCType, druid_rdbc::types::RdbcType);
    bidirectional_dyn_identity!(druid::sql::rdbc_sql_type::SqlType, druid_rdbc::types::SqlType);
    bidirectional_dyn_identity!(druid::sql::SQLType, druid_rdbc::types::SqlType);

    bidirectional_owned_identity!(
        druid::core::sql_exception::SqlExceptionCause,
        druid_rdbc::error::SqlExceptionCause
    );
    bidirectional_owned_identity!(druid::core::SqlExceptionCause, druid_rdbc::error::SqlExceptionCause);
    bidirectional_owned_identity!(
        druid::core::sql_exception::SqlExceptionKind,
        druid_rdbc::error::SqlExceptionKind
    );
    bidirectional_owned_identity!(druid::core::SqlExceptionKind, druid_rdbc::error::SqlExceptionKind);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlExceptionKind,
        druid_rdbc::error::SqlExceptionKind
    );
    bidirectional_owned_identity!(druid::sql::SqlExceptionKind, druid_rdbc::error::SqlExceptionKind);

    bidirectional_owned_identity!(
        druid::core::sql_exception::SqlException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::core::SqlException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(druid::sql::SQLException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlTransientException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlTransientException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlTransientConnectionException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlTransientConnectionException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlNonTransientException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlNonTransientException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlNonTransientConnectionException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlNonTransientConnectionException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlRecoverableException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlRecoverableException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlDataException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlDataException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlIntegrityConstraintViolationException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlIntegrityConstraintViolationException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlInvalidAuthorizationSpecException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlInvalidAuthorizationSpecException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlSyntaxErrorException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlSyntaxErrorException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlTransactionRollbackException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlTransactionRollbackException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlFeatureNotSupportedException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlFeatureNotSupportedException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlTimeoutException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlTimeoutException, druid_rdbc::error::SqlException);

    bidirectional_owned_identity!(
        druid::core::sql_warning::SqlWarning,
        druid_rdbc::error::SqlWarning
    );
    bidirectional_owned_identity!(druid::core::SqlWarning, druid_rdbc::error::SqlWarning);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlWarning,
        druid_rdbc::error::SqlWarning
    );
    bidirectional_owned_identity!(druid::sql::SqlWarning, druid_rdbc::error::SqlWarning);
    bidirectional_owned_identity!(druid::sql::SQLWarning, druid_rdbc::error::SqlWarning);

    bidirectional_owned_identity!(
        druid::core::error::DruidError,
        druid_rdbc::compat::DruidError
    );
    bidirectional_owned_identity!(druid::core::DruidError, druid_rdbc::compat::DruidError);
    bidirectional_owned_identity!(
        druid::core::rdbc_string::RdbcString,
        druid_rdbc::types::RdbcString
    );
    bidirectional_owned_identity!(druid::core::RdbcString, druid_rdbc::types::RdbcString);
    bidirectional_owned_identity!(druid::sql::RdbcString, druid_rdbc::types::RdbcString);
    bidirectional_owned_identity!(
        druid::core::savepoint::Savepoint,
        druid_rdbc::types::Savepoint
    );
    bidirectional_owned_identity!(druid::core::Savepoint, druid_rdbc::types::Savepoint);
    bidirectional_owned_identity!(druid::sql::savepoint::Savepoint, druid_rdbc::types::Savepoint);
    bidirectional_owned_identity!(druid::sql::Savepoint, druid_rdbc::types::Savepoint);
    bidirectional_owned_identity!(druid::sql::date::Date, druid_rdbc::types::Date);
    bidirectional_owned_identity!(druid::sql::Date, druid_rdbc::types::Date);
    bidirectional_owned_identity!(druid::sql::time::Time, druid_rdbc::types::Time);
    bidirectional_owned_identity!(druid::sql::Time, druid_rdbc::types::Time);
    bidirectional_owned_identity!(
        druid::sql::timestamp::Timestamp,
        druid_rdbc::types::Timestamp
    );
    bidirectional_owned_identity!(druid::sql::Timestamp, druid_rdbc::types::Timestamp);

    bidirectional_owned_identity!(
        druid::sql::exceptions::BatchUpdateException,
        druid_rdbc::compat::BatchUpdateException
    );
    bidirectional_owned_identity!(
        druid::sql::BatchUpdateException,
        druid_rdbc::compat::BatchUpdateException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::DataTruncation,
        druid_rdbc::compat::DataTruncation
    );
    bidirectional_owned_identity!(druid::sql::DataTruncation, druid_rdbc::compat::DataTruncation);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlClientInfoException,
        druid_rdbc::compat::SqlClientInfoException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlClientInfoException,
        druid_rdbc::compat::SqlClientInfoException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlTransientException,
        druid_rdbc::compat::SqlTransientException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlTransientConnectionException,
        druid_rdbc::compat::SqlTransientConnectionException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlNonTransientException,
        druid_rdbc::compat::SqlNonTransientException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlNonTransientConnectionException,
        druid_rdbc::compat::SqlNonTransientConnectionException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlRecoverableException,
        druid_rdbc::compat::SqlRecoverableException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlDataException,
        druid_rdbc::compat::SqlDataException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlIntegrityConstraintViolationException,
        druid_rdbc::compat::SqlIntegrityConstraintViolationException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlInvalidAuthorizationSpecException,
        druid_rdbc::compat::SqlInvalidAuthorizationSpecException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlSyntaxErrorException,
        druid_rdbc::compat::SqlSyntaxErrorException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlTransactionRollbackException,
        druid_rdbc::compat::SqlTransactionRollbackException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlFeatureNotSupportedException,
        druid_rdbc::compat::SqlFeatureNotSupportedException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlTimeoutException,
        druid_rdbc::compat::SqlTimeoutException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlExceptionKind,
        druid_rdbc::compat::SqlExceptionKind
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlException,
        druid_rdbc::compat::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlWarning,
        druid_rdbc::compat::SqlWarning
    );

    bidirectional_owned_identity!(druid::core::exec_result::ExecResult, druid_rdbc::spi::ExecResult);
    bidirectional_owned_identity!(druid::core::ExecResult, druid_rdbc::spi::ExecResult);
    bidirectional_owned_identity!(druid::core::wrapper::Unwrapped, druid_rdbc::spi::Unwrapped);
    bidirectional_owned_identity!(druid::sql::wrapper::Unwrapped, druid_rdbc::spi::Unwrapped);
    bidirectional_dyn_identity!(druid::core::wrapper::Wrapper, druid_rdbc::spi::Wrapper);
    bidirectional_dyn_identity!(druid::sql::wrapper::Wrapper, druid_rdbc::spi::Wrapper);
    bidirectional_path_identity!(druid::core::wrapper::WrapperExt, druid_rdbc::spi::WrapperExt);
    bidirectional_path_identity!(druid::sql::wrapper::WrapperExt, druid_rdbc::spi::WrapperExt);

    bidirectional_owned_identity!(
        druid::core::database_meta_data_row_id_lifetime::DatabaseMetaDataRowIdLifetime,
        druid_rdbc::spi::DatabaseMetaDataRowIdLifetime
    );
    bidirectional_owned_identity!(
        druid::core::result_set_nullability::ResultSetNullability,
        druid_rdbc::spi::ResultSetNullability
    );
    bidirectional_owned_identity!(
        druid::core::callable_input_parameter::CallableInputParameter,
        druid_rdbc::spi::CallableInputParameter
    );
    bidirectional_owned_identity!(
        druid::core::callable_out_parameter::CallableOutParameter,
        druid_rdbc::spi::CallableOutParameter
    );
    bidirectional_owned_identity!(
        druid::core::callable_parameter::CallableParameter,
        druid_rdbc::spi::CallableParameter
    );
    bidirectional_owned_identity!(
        druid::core::result_set_update::ResultSetUpdate,
        druid_rdbc::spi::ResultSetUpdate
    );

    bidirectional_owned_identity!(
        druid::core::rdbc_calendar::RdbcCalendar,
        druid_rdbc::types::RdbcCalendar
    );
    bidirectional_owned_identity!(
        druid::core::rdbc_calendar::RdbcCalendarArgument,
        druid_rdbc::types::RdbcCalendarArgument
    );
    bidirectional_owned_identity!(druid::core::rdbc_object::RdbcObject, druid_rdbc::types::RdbcObject);
    bidirectional_dyn_identity!(
        druid::core::rdbc_opaque_object::PhysicalRdbcOpaqueObject,
        druid_rdbc::types::PhysicalRdbcOpaqueObject
    );
    bidirectional_owned_identity!(
        druid::core::rdbc_opaque_object::RdbcOpaqueObject,
        druid_rdbc::types::RdbcOpaqueObject
    );
    bidirectional_owned_identity!(druid::core::rdbc_row_id::RdbcRowId, druid_rdbc::types::RdbcRowId);
    bidirectional_owned_identity!(
        druid::core::rdbc_target_type::RdbcTargetType,
        druid_rdbc::types::RdbcTargetType
    );
    bidirectional_owned_identity!(
        druid::core::rdbc_type_map::RdbcTypeMap,
        druid_rdbc::types::RdbcTypeMap
    );
    bidirectional_dyn_identity!(
        druid::core::rdbc_parameter::RdbcParameter,
        druid_rdbc::types::RdbcParameter
    );
    bidirectional_owned_identity!(
        druid::core::rdbc_parameter::RdbcParameterType,
        druid_rdbc::types::RdbcParameterType
    );
    bidirectional_owned_identity!(
        druid::core::rdbc_parameter::RdbcParameterValue,
        druid_rdbc::types::RdbcParameterValue
    );
    bidirectional_owned_identity!(druid::core::rdbc_parameter_date::RdbcParameterDate, druid_rdbc::types::RdbcParameterDate);
    bidirectional_owned_identity!(druid::core::rdbc_parameter_decimal::RdbcParameterDecimal, druid_rdbc::types::RdbcParameterDecimal);
    bidirectional_owned_identity!(druid::core::rdbc_parameter_impl::RdbcParameterImpl, druid_rdbc::types::RdbcParameterImpl);
    bidirectional_owned_identity!(druid::core::rdbc_parameter_int::RdbcParameterInt, druid_rdbc::types::RdbcParameterInt);
    bidirectional_owned_identity!(druid::core::rdbc_parameter_long::RdbcParameterLong, druid_rdbc::types::RdbcParameterLong);
    bidirectional_owned_identity!(druid::core::rdbc_parameter_null::RdbcParameterNull, druid_rdbc::types::RdbcParameterNull);
    bidirectional_owned_identity!(druid::core::rdbc_parameter_string::RdbcParameterString, druid_rdbc::types::RdbcParameterString);
    bidirectional_owned_identity!(druid::core::rdbc_parameter_timestamp::RdbcParameterTimestamp, druid_rdbc::types::RdbcParameterTimestamp);

    bidirectional_owned_identity!(druid::core::rdbc_xml_representation_type::RdbcXmlRepresentationType, druid_rdbc::resource::RdbcXmlRepresentationType);
    bidirectional_dyn_identity!(druid::core::rdbc_xml_result::PhysicalXmlResult, druid_rdbc::resource::PhysicalXmlResult);
    bidirectional_owned_identity!(druid::core::rdbc_xml_result::RdbcXmlResult, druid_rdbc::resource::RdbcXmlResult);
    bidirectional_dyn_identity!(druid::core::rdbc_xml_source::PhysicalXmlSource, druid_rdbc::resource::PhysicalXmlSource);
    bidirectional_owned_identity!(druid::core::rdbc_xml_source::RdbcXmlSource, druid_rdbc::resource::RdbcXmlSource);

    bidirectional_arc_dyn_identity!(druid::core::physical_connection_factory::PhysicalConnectionFactory, druid_rdbc::spi::PhysicalConnectionFactory);
    bidirectional_box_dyn_identity!(druid::core::physical_connection::PhysicalConnection, druid_rdbc::spi::PhysicalConnection);
    bidirectional_dyn_identity!(druid::core::physical_statement::PhysicalStatement, druid_rdbc::spi::PhysicalStatement);
    bidirectional_dyn_identity!(druid::core::physical_prepared_statement::PhysicalPreparedStatement, druid_rdbc::spi::PhysicalPreparedStatement);
    bidirectional_dyn_identity!(druid::core::physical_callable_statement::PhysicalCallableStatement, druid_rdbc::spi::PhysicalCallableStatement);
    bidirectional_dyn_identity!(druid::core::physical_database_meta_data::PhysicalDatabaseMetaData, druid_rdbc::spi::PhysicalDatabaseMetaData);
    bidirectional_dyn_identity!(druid::core::physical_result_set_meta_data::PhysicalResultSetMetaData, druid_rdbc::spi::PhysicalResultSetMetaData);
    bidirectional_dyn_identity!(druid::core::rdbc_result_set::PhysicalResultSet, druid_rdbc::spi::PhysicalResultSet);
    bidirectional_owned_identity!(druid::core::physical_connection_capabilities::PhysicalConnectionCapabilities, druid_rdbc::spi::PhysicalConnectionCapabilities);
    bidirectional_owned_identity!(druid::core::physical_connection_info::PhysicalConnectionInfo, druid_rdbc::spi::PhysicalConnectionInfo);
    bidirectional_owned_identity!(druid::core::physical_statement::PhysicalStatementOptions, druid_rdbc::spi::PhysicalStatementOptions);
    bidirectional_owned_identity!(druid::core::physical_statement::StatementGeneratedKeys, druid_rdbc::spi::StatementGeneratedKeys);
    bidirectional_owned_identity!(druid::core::physical_statement::StatementExecuteResult, druid_rdbc::spi::StatementExecuteResult);
    bidirectional_owned_identity!(druid::core::physical_statement::SqlTextStatement, druid_rdbc::spi::SqlTextStatement);
    bidirectional_owned_identity!(druid::core::physical_prepared_statement::SqlTextPreparedStatement, druid_rdbc::spi::SqlTextPreparedStatement);
    bidirectional_owned_identity!(druid::core::rdbc_result_set::RdbcResultSet, druid_rdbc::spi::RdbcResultSet);
    bidirectional_owned_identity!(druid::core::rdbc_result_set::RowSetResultSet, druid_rdbc::spi::RowSetResultSet);
    bidirectional_owned_identity!(druid::core::prepared_input_parameter::PreparedInputParameter, druid_rdbc::spi::PreparedInputParameter);
    bidirectional_owned_identity!(druid::core::prepared_input_parameter::PreparedTypeNameArgument, druid_rdbc::spi::PreparedTypeNameArgument);
    bidirectional_owned_identity!(druid::core::prepared_statement_key::PreparedStatementKey, druid_rdbc::spi::PreparedStatementKey);
    bidirectional_owned_identity!(druid::core::prepared_statement_key::PreparedStatementMethodType, druid_rdbc::spi::PreparedStatementMethodType);
    bidirectional_owned_identity!(druid::core::result_set_meta_data::ResultSetMetaData, druid_rdbc::spi::ResultSetMetaData);
    bidirectional_owned_identity!(druid::core::result_set_column_meta::ResultSetColumnMeta, druid_rdbc::spi::ResultSetColumnMeta);
    bidirectional_owned_identity!(druid::core::result_set_column_type::ResultSetColumnType, druid_rdbc::spi::ResultSetColumnType);

    bidirectional_dyn_identity!(druid::spi::RdbcArrayAccess, druid_rdbc::resource::RdbcArrayAccess);
    bidirectional_dyn_identity!(druid::spi::RdbcBlobAccess, druid_rdbc::resource::RdbcBlobAccess);
    bidirectional_dyn_identity!(druid::spi::RdbcClobAccess, druid_rdbc::resource::RdbcClobAccess);
    bidirectional_dyn_identity!(druid::spi::RdbcNClobAccess, druid_rdbc::resource::RdbcNClobAccess);
    bidirectional_dyn_identity!(druid::spi::RdbcRefAccess, druid_rdbc::resource::RdbcRefAccess);
    bidirectional_dyn_identity!(druid::spi::RdbcResourceAccess, druid_rdbc::resource::RdbcResourceAccess);
    bidirectional_dyn_identity!(druid::spi::RdbcResourceOwner, druid_rdbc::resource::RdbcResourceOwner);
    bidirectional_dyn_identity!(druid::spi::RdbcSqlXmlAccess, druid_rdbc::resource::RdbcSqlXmlAccess);
    bidirectional_owned_identity!(druid::spi::RdbcResourceCapabilities, druid_rdbc::resource::RdbcResourceCapabilities);
    bidirectional_owned_identity!(druid::spi::RdbcResourceContext, druid_rdbc::resource::RdbcResourceContext);
    bidirectional_owned_identity!(druid::spi::RdbcResourceFactory, druid_rdbc::resource::RdbcResourceFactory);
    bidirectional_owned_identity!(druid::spi::RdbcResourceId, druid_rdbc::resource::RdbcResourceId);
    bidirectional_owned_identity!(druid::spi::RdbcResourceKind, druid_rdbc::resource::RdbcResourceKind);
    bidirectional_owned_identity!(druid::spi::RdbcResourceState, druid_rdbc::resource::RdbcResourceState);

    bidirectional_owned_identity!(druid::sql::array::RdbcArray, druid_rdbc::resource::RdbcArray);
    bidirectional_owned_identity!(druid::sql::Array, druid_rdbc::resource::Array);
    bidirectional_owned_identity!(druid::sql::blob::RdbcBlob, druid_rdbc::resource::RdbcBlob);
    bidirectional_owned_identity!(druid::sql::Blob, druid_rdbc::resource::Blob);
    bidirectional_owned_identity!(druid::sql::clob::RdbcClob, druid_rdbc::resource::RdbcClob);
    bidirectional_owned_identity!(druid::sql::Clob, druid_rdbc::resource::Clob);
    bidirectional_owned_identity!(druid::sql::n_clob::RdbcNClob, druid_rdbc::resource::RdbcNClob);
    bidirectional_owned_identity!(druid::sql::NClob, druid_rdbc::resource::NClob);
    bidirectional_owned_identity!(druid::sql::ref_value::RdbcRef, druid_rdbc::resource::RdbcRef);
    bidirectional_owned_identity!(druid::sql::Ref, druid_rdbc::resource::Ref);
    bidirectional_owned_identity!(druid::sql::sql_xml::RdbcSqlXml, druid_rdbc::resource::RdbcSqlXml);
    bidirectional_owned_identity!(druid::sql::SQLXML, druid_rdbc::resource::SqlXml);
    bidirectional_owned_identity!(druid::core::rdbc_input_stream::RdbcInputStream, druid_rdbc::resource::RdbcInputStream);
    bidirectional_owned_identity!(druid::core::rdbc_input_stream::RdbcStreamLength, druid_rdbc::resource::RdbcStreamLength);
    bidirectional_owned_identity!(druid::core::rdbc_output_stream::RdbcOutputStream, druid_rdbc::resource::RdbcOutputStream);
    bidirectional_dyn_identity!(druid::core::rdbc_reader::PhysicalCharacterReader, druid_rdbc::resource::PhysicalCharacterReader);
    bidirectional_owned_identity!(druid::core::rdbc_reader::RdbcReader, druid_rdbc::resource::RdbcReader);
    bidirectional_owned_identity!(druid::core::rdbc_reader::RdbcCharacterLength, druid_rdbc::resource::RdbcCharacterLength);
    bidirectional_dyn_identity!(druid::core::rdbc_writer::PhysicalCharacterWriter, druid_rdbc::resource::PhysicalCharacterWriter);
    bidirectional_owned_identity!(druid::core::rdbc_writer::RdbcWriter, druid_rdbc::resource::RdbcWriter);
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
";
    fs::write(package.path.join("Cargo.toml"), manifest)?;
    fs::create_dir(package.path.join("src"))?;
    fs::write(package.path.join("src/main.rs"), source)?;

    let output = Command::new(env!("CARGO"))
        .arg("check")
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

    Ok(())
}

/// 验证结构化 SQL 异常与警告在旧、新路径之间保持同一类型和完整字段语义。
#[test]
fn error_identity() {
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

    let old_leaf: druid::core::sql_exception::SqlException = exception;
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
    let old_warning: druid::core::sql_warning::SqlWarning = warning;
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
