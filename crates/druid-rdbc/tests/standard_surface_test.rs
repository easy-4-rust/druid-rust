use druid_rdbc::connection::Connection;
use druid_rdbc::data_source::{
    CommonDataSource, ConnectionPoolDataSource, DataSource, PooledConnection, XaConnection,
    XaDataSource,
};
use druid_rdbc::driver::{ConnectionFactory, Driver, DriverRegistry};
use druid_rdbc::extensions::{ConnectionCapabilities, ExecResult};
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, DatabaseMetaData, ParameterDescriptor, ParameterMetaData,
    ParameterMode, ParameterNullability, ResultSetColumnMeta, ResultSetColumnType,
    ResultSetMetaData, ResultSetMetaDataSnapshot, ResultSetNullability, RowIdLifetime,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use druid_rdbc::statement::{
    CallableInputParameter, CallableParameter, CallableStatement, GeneratedKeys, ParameterIndex,
    PreparedInputParameter, PreparedStatement, PreparedTypeNameArgument, Statement,
    StatementExecuteResult, StatementOptions,
};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper, WrapperExt};
use std::any::TypeId;

fn assert_send_static<T: Send + 'static + ?Sized>() {}

fn assert_send_sync_static<T: Send + Sync + 'static + ?Sized>() {}

fn assert_wrapper<T: Wrapper + ?Sized>() {}

struct CanonicalConnection;
struct CredentialAwareDataSource;
struct CredentialAwareConnectionPoolDataSource;
struct CredentialAwareXaDataSource;
struct SurfaceResultSet;
struct GeneratedKeysStatement;
struct SurfaceParameterMetaData;

druid_rdbc::impl_standard_wrapper!(CanonicalConnection => Connection);

#[async_trait::async_trait]
impl Connection for CanonicalConnection {
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, druid_rdbc::error::RdbcError> {
        Err(druid_rdbc::error::RdbcError::feature_not_supported(
            "canonical_connection_create_statement",
        ))
    }
}

druid_rdbc::impl_standard_wrapper!(SurfaceResultSet => ResultSet);
druid_rdbc::impl_standard_wrapper!(GeneratedKeysStatement => Statement);
druid_rdbc::impl_standard_wrapper!(SurfaceParameterMetaData => ParameterMetaData);

#[async_trait::async_trait]
impl ResultSet for SurfaceResultSet {
    async fn next(&mut self) -> Result<bool, druid_rdbc::error::RdbcError> {
        Ok(false)
    }
}

#[async_trait::async_trait]
impl Statement for GeneratedKeysStatement {
    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, druid_rdbc::error::RdbcError> {
        Ok(Box::new(SurfaceResultSet))
    }
}

impl ParameterMetaData for SurfaceParameterMetaData {
    fn parameter_count(&self) -> usize {
        1
    }

    fn parameter(
        &self,
        _parameter_index: ParameterIndex,
    ) -> Result<ParameterDescriptor, druid_rdbc::error::RdbcError> {
        Ok(ParameterDescriptor::new(
            "INTEGER",
            ParameterMode::In,
            ParameterNullability::Unknown,
        ))
    }
}

druid_rdbc::impl_standard_wrapper!(CredentialAwareDataSource => DataSource);

impl CommonDataSource for CredentialAwareDataSource {}
impl CommonDataSource for CredentialAwareConnectionPoolDataSource {}
impl CommonDataSource for CredentialAwareXaDataSource {}

#[async_trait::async_trait]
impl DataSource for CredentialAwareDataSource {
    async fn connection(&self) -> Result<Box<dyn Connection>, druid_rdbc::error::RdbcError> {
        Err(druid_rdbc::error::RdbcError::Other(
            "credentialless data-source path invoked".to_string(),
        ))
    }
}

#[async_trait::async_trait]
impl ConnectionPoolDataSource for CredentialAwareConnectionPoolDataSource {
    async fn pooled_connection(
        &self,
    ) -> Result<Box<dyn PooledConnection>, druid_rdbc::error::RdbcError> {
        Err(druid_rdbc::error::RdbcError::Other(
            "credentialless connection-pool path invoked".to_string(),
        ))
    }
}

#[async_trait::async_trait]
impl XaDataSource for CredentialAwareXaDataSource {
    async fn xa_connection(&self) -> Result<Box<dyn XaConnection>, druid_rdbc::error::RdbcError> {
        Err(druid_rdbc::error::RdbcError::Other(
            "credentialless XA path invoked".to_string(),
        ))
    }
}

#[test]
fn canonical_traits_are_dyn_compatible_with_frozen_auto_traits() {
    let _: Option<Box<dyn Connection>> = None;
    let _: Option<Box<dyn Statement>> = None;
    let _: Option<Box<dyn PreparedStatement>> = None;
    let _: Option<Box<dyn CallableStatement>> = None;
    let _: Option<Box<dyn ResultSet>> = None;
    let _: Option<Box<dyn DatabaseMetaData>> = None;
    let _: Option<Box<dyn ResultSetMetaData>> = None;
    let _: Option<Box<dyn ParameterMetaData>> = None;
    let _: Option<Box<dyn Driver>> = None;
    let _: Option<Box<dyn ConnectionFactory>> = None;
    let _: Option<Box<dyn CommonDataSource>> = None;
    let _: Option<Box<dyn DataSource>> = None;
    let _: Option<Box<dyn PooledConnection>> = None;
    let _: Option<Box<dyn ConnectionPoolDataSource>> = None;
    let _: Option<Box<dyn XaConnection>> = None;
    let _: Option<Box<dyn XaDataSource>> = None;
    let _: Option<Box<dyn Wrapper>> = None;

    assert_send_static::<dyn Connection>();
    assert_send_static::<dyn Statement>();
    assert_send_static::<dyn PreparedStatement>();
    assert_send_static::<dyn CallableStatement>();
    assert_send_static::<dyn ResultSet>();
    assert_send_static::<dyn DatabaseMetaData>();
    assert_send_sync_static::<dyn ResultSetMetaData>();
    assert_send_sync_static::<dyn ParameterMetaData>();
    assert_send_sync_static::<dyn Driver>();
    assert_send_sync_static::<dyn ConnectionFactory>();
    assert_send_sync_static::<DriverRegistry>();
    assert_wrapper::<dyn Connection>();
    assert_wrapper::<dyn Statement>();
    assert_wrapper::<dyn PreparedStatement>();
    assert_wrapper::<dyn CallableStatement>();
    assert_wrapper::<dyn ResultSet>();
    assert_wrapper::<dyn DatabaseMetaData>();
    assert_wrapper::<dyn ResultSetMetaData>();
    assert_wrapper::<dyn ParameterMetaData>();
}

#[test]
fn typed_standard_descriptors_have_neutral_defaults() {
    let options = StatementOptions::default();
    assert_eq!(options, StatementOptions::default());

    let capabilities = ConnectionCapabilities::default();
    assert!(!capabilities.transactions);
    assert!(!capabilities.savepoints);
    assert!(!capabilities.catalogs);
    assert!(!capabilities.schemas);
    assert!(!capabilities.callable_statements);

    let _ = ExecResult::default();
    let _ = ResultSetMetaDataSnapshot::default();
    let _: Option<ResultSetColumnMeta> = None;
    let _: Option<ResultSetColumnType> = None;
    let _: Option<ResultSetNullability> = None;
    let _: Option<ResultSetUpdate> = None;
    let _: Option<CallableInputParameter> = None;
    let _: Option<CallableParameter> = None;
    let _: Option<PreparedInputParameter> = None;
    let _: Option<PreparedTypeNameArgument> = None;
    let _: Option<StatementExecuteResult> = None;
    let _: Option<StandardUnwrapped<'static>> = None;
    fn assert_wrapper_ext<T: WrapperExt + ?Sized>() {}
    assert_wrapper_ext::<dyn Wrapper>();
}

#[tokio::test]
async fn credentials_and_close_defaults_fail_observably() {
    let data_source = CredentialAwareDataSource;
    assert!(matches!(
        data_source
            .connection_with_credentials("alice", "secret")
            .await,
        Err(druid_rdbc::error::RdbcError::SqlException(_))
    ));

    let connection_pool_data_source = CredentialAwareConnectionPoolDataSource;
    assert!(matches!(
        connection_pool_data_source
            .pooled_connection_with_credentials("alice", "secret")
            .await,
        Err(druid_rdbc::error::RdbcError::SqlException(_))
    ));

    let xa_data_source = CredentialAwareXaDataSource;
    assert!(matches!(
        xa_data_source
            .xa_connection_with_credentials("alice", "secret")
            .await,
        Err(druid_rdbc::error::RdbcError::SqlException(_))
    ));

    let mut connection = CanonicalConnection;
    assert!(matches!(
        connection.close().await,
        Err(druid_rdbc::error::RdbcError::SqlException(_))
    ));
    assert!(matches!(
        connection.is_closed(),
        Err(druid_rdbc::error::RdbcError::SqlException(_))
    ));
}

#[test]
fn wrapper_preserves_canonical_interface_and_negative_error_semantics(
) -> Result<(), druid_rdbc::error::RdbcError> {
    struct UnknownTarget;

    let connection = CanonicalConnection;
    let canonical_target = Some(TypeId::of::<dyn Connection>());
    assert!(connection.is_wrapper_for(canonical_target)?);
    match connection.unwrap(canonical_target)? {
        StandardUnwrapped::Connection(unwrapped) => {
            assert!(unwrapped.is_wrapper_for(canonical_target)?);
        }
        other => panic!("expected canonical Connection, got {other:?}"),
    }

    let concrete = connection.unwrap_ref::<CanonicalConnection>()?;
    assert!(std::ptr::eq(
        std::ptr::from_ref(concrete),
        std::ptr::from_ref(&connection)
    ));

    let unknown_target = Some(TypeId::of::<UnknownTarget>());
    assert!(!connection.is_wrapper_for(unknown_target)?);
    assert!(matches!(
        connection.unwrap(unknown_target),
        Err(druid_rdbc::error::RdbcError::SqlException(_))
    ));
    assert!(matches!(
        connection.is_wrapper_for(None),
        Err(druid_rdbc::error::RdbcError::InvalidArgument(_))
    ));
    assert!(matches!(
        connection.unwrap(None),
        Err(druid_rdbc::error::RdbcError::InvalidArgument(_))
    ));
    Ok(())
}

#[tokio::test]
async fn one_based_indexes_and_generated_keys_reject_invalid_values(
) -> Result<(), druid_rdbc::error::RdbcError> {
    assert!(ColumnIndex::try_from(-1_i64).is_err());
    assert!(ColumnIndex::try_from(0_i64).is_err());
    assert_eq!(ColumnIndex::try_from(1_i64)?.get(), 1);
    assert!(ColumnIndex::try_from(i128::MAX).is_err());

    assert!(ParameterIndex::try_from(-1_i64).is_err());
    assert!(ParameterIndex::try_from(0_i64).is_err());
    assert_eq!(ParameterIndex::try_from(1_i64)?.get(), 1);
    assert!(ParameterIndex::try_from(i128::MAX).is_err());

    let generated_keys = GeneratedKeys::ColumnIndexes(vec![ColumnIndex::try_from(1_i64)?]);
    assert!(matches!(generated_keys, GeneratedKeys::ColumnIndexes(_)));

    let mut statement = GeneratedKeysStatement;
    assert!(matches!(
        statement.query_timeout(),
        Err(druid_rdbc::error::RdbcError::UnsupportedOperation {
            operation: "statement_query_timeout"
        })
    ));
    assert!(matches!(
        statement.set_query_timeout(5),
        Err(druid_rdbc::error::RdbcError::UnsupportedOperation {
            operation: "statement_set_query_timeout"
        })
    ));
    let mut result_set = statement.generated_keys().await?;
    assert!(!result_set.next().await?);
    Ok(())
}

#[test]
fn metadata_indexes_and_value_enums_preserve_jdbc_states(
) -> Result<(), druid_rdbc::error::RdbcError> {
    let result_set_meta_data = ResultSetMetaDataSnapshot::new(vec![ColumnDescriptor::new(
        "id",
        "INTEGER",
        ColumnNullability::NoNulls,
    )]);
    let column = result_set_meta_data.column(ColumnIndex::try_from(1_i64)?)?;
    assert_eq!(column.label(), "id");

    let parameter_meta_data = SurfaceParameterMetaData;
    let parameter = parameter_meta_data.parameter(ParameterIndex::try_from(1_i64)?)?;
    assert_eq!(parameter.nullability(), ParameterNullability::Unknown);

    let row_id_lifetimes = [
        (RowIdLifetime::Unsupported, "ROWID_UNSUPPORTED"),
        (RowIdLifetime::ValidOther, "ROWID_VALID_OTHER"),
        (RowIdLifetime::Transaction, "ROWID_VALID_TRANSACTION"),
        (RowIdLifetime::Session, "ROWID_VALID_SESSION"),
        (RowIdLifetime::Forever, "ROWID_VALID_FOREVER"),
    ];
    for (lifetime, jdbc_name) in row_id_lifetimes {
        assert_eq!(lifetime.jdbc_name(), jdbc_name);
        assert_eq!(RowIdLifetime::try_from(jdbc_name)?, lifetime);
    }
    Ok(())
}
