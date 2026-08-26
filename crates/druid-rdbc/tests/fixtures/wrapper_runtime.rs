#![allow(dead_code)]

use druid_rdbc::connection::Connection;
use druid_rdbc::data_source::{
    CommonDataSource, ConnectionPoolDataSource, DataSource, PooledConnection, XaConnection,
    XaDataSource,
};
use druid_rdbc::driver::{ConnectionFactory, ConnectionProperties, Driver};
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{DatabaseMetaData, ParameterMetaData, ResultSetMetaData};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{CallableStatement, PreparedStatement, Statement, StatementOptions};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper, WrapperExt};
use std::any::TypeId;
use std::marker::PhantomData;

struct ConnectionObject;
struct StatementObject;
struct PreparedStatementObject;
struct CallableStatementObject;
struct DatabaseMetaDataObject;
struct ResultSetMetaDataObject;
struct ParameterMetaDataObject;
struct CommonDataSourceObject;
struct DataSourceObject;
struct PooledConnectionObject;
struct ConnectionPoolDataSourceObject;
struct XaConnectionObject;
struct XaDataSourceObject;
struct DriverObject;
struct ConnectionFactoryObject;
struct WrapperObject;
struct GenericResultSet<T, U, const WIDTH: usize>(PhantomData<(T, U, [u8; WIDTH])>);

trait ResultSetMarker: Send + Sync + 'static {}
impl ResultSetMarker for u8 {}

druid_rdbc::impl_standard_wrapper!(ConnectionObject => Connection);
druid_rdbc::impl_standard_wrapper!(StatementObject => Statement);
druid_rdbc::impl_standard_wrapper!(PreparedStatementObject => PreparedStatement);
druid_rdbc::impl_standard_wrapper!(CallableStatementObject => CallableStatement);
druid_rdbc::impl_standard_wrapper!(DatabaseMetaDataObject => DatabaseMetaData);
druid_rdbc::impl_standard_wrapper!(ResultSetMetaDataObject => ResultSetMetaData);
druid_rdbc::impl_standard_wrapper!(ParameterMetaDataObject => ParameterMetaData);
druid_rdbc::impl_standard_wrapper!(CommonDataSourceObject => CommonDataSource);
druid_rdbc::impl_standard_wrapper!(DataSourceObject => DataSource);
druid_rdbc::impl_standard_wrapper!(PooledConnectionObject => PooledConnection);
druid_rdbc::impl_standard_wrapper!(ConnectionPoolDataSourceObject => ConnectionPoolDataSource);
druid_rdbc::impl_standard_wrapper!(XaConnectionObject => XaConnection);
druid_rdbc::impl_standard_wrapper!(XaDataSourceObject => XaDataSource);
druid_rdbc::impl_standard_wrapper!(DriverObject => Driver);
druid_rdbc::impl_standard_wrapper!(ConnectionFactoryObject => ConnectionFactory);
druid_rdbc::impl_standard_wrapper!(WrapperObject => Wrapper);
druid_rdbc::impl_standard_wrapper!(
    generics [T, U, const WIDTH: usize] for GenericResultSet<T, U, WIDTH>
    where [T: ResultSetMarker, U: Send + Sync + 'static] => ResultSet
);

#[async_trait::async_trait]
impl Connection for ConnectionObject {
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        Ok(Box::new(StatementObject))
    }
}

#[async_trait::async_trait]
impl Statement for StatementObject {}

#[async_trait::async_trait]
impl Statement for PreparedStatementObject {}

#[async_trait::async_trait]
impl PreparedStatement for PreparedStatementObject {}

#[async_trait::async_trait]
impl Statement for CallableStatementObject {}

#[async_trait::async_trait]
impl PreparedStatement for CallableStatementObject {}

#[async_trait::async_trait]
impl CallableStatement for CallableStatementObject {}

#[async_trait::async_trait]
impl<T, U, const WIDTH: usize> ResultSet for GenericResultSet<T, U, WIDTH>
where
    T: ResultSetMarker,
    U: Send + Sync + 'static,
{
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

#[async_trait::async_trait]
impl DatabaseMetaData for DatabaseMetaDataObject {}

impl ResultSetMetaData for ResultSetMetaDataObject {
    fn column_count(&self) -> usize {
        0
    }
}

impl ParameterMetaData for ParameterMetaDataObject {
    fn parameter_count(&self) -> usize {
        0
    }
}

impl CommonDataSource for CommonDataSourceObject {}

impl CommonDataSource for DataSourceObject {}

#[async_trait::async_trait]
impl DataSource for DataSourceObject {
    async fn connection(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(ConnectionObject))
    }
}

#[async_trait::async_trait]
impl PooledConnection for PooledConnectionObject {
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(ConnectionObject))
    }
}

impl CommonDataSource for ConnectionPoolDataSourceObject {}

#[async_trait::async_trait]
impl ConnectionPoolDataSource for ConnectionPoolDataSourceObject {
    async fn pooled_connection(&self) -> Result<Box<dyn PooledConnection>, RdbcError> {
        Ok(Box::new(PooledConnectionObject))
    }
}

#[async_trait::async_trait]
impl PooledConnection for XaConnectionObject {
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(ConnectionObject))
    }
}

impl XaConnection for XaConnectionObject {
    fn xa_resource_name(&self) -> &str {
        "external-xa-resource"
    }
}

impl CommonDataSource for XaDataSourceObject {}

#[async_trait::async_trait]
impl XaDataSource for XaDataSourceObject {
    async fn xa_connection(&self) -> Result<Box<dyn XaConnection>, RdbcError> {
        Ok(Box::new(XaConnectionObject))
    }
}

#[async_trait::async_trait]
impl Driver for DriverObject {
    fn name(&self) -> &str {
        "external-driver"
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

#[async_trait::async_trait]
impl ConnectionFactory for ConnectionFactoryObject {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(ConnectionObject))
    }
}

macro_rules! assert_exact_identity {
    ($value:expr, $trait_object:ty, $variant:ident) => {{
        let value = $value;
        let expected: &$trait_object = &value;
        match value.unwrap(Some(TypeId::of::<$trait_object>()))? {
            StandardUnwrapped::$variant(actual) => {
                assert_eq!(
                    actual as *const _ as *const (),
                    expected as *const _ as *const ()
                );
            }
            other => panic!("unexpected canonical variant: {other:?}"),
        }
        let expected_wrapper: &dyn Wrapper = &value;
        match value.unwrap(Some(TypeId::of::<dyn Wrapper>()))? {
            StandardUnwrapped::Wrapper(actual) => {
                assert_eq!(
                    actual as *const _ as *const (),
                    expected_wrapper as *const _ as *const (),
                );
            }
            other => panic!("unexpected Wrapper identity: {other:?}"),
        }
    }};
}

macro_rules! assert_inherited_identity {
    ($value:expr, $trait_object:ty, $variant:ident) => {{
        let value = &$value;
        let expected: &$trait_object = value;
        let target = TypeId::of::<$trait_object>();
        assert!(value.is_wrapper_for(Some(target))?);
        match value.unwrap(Some(target))? {
            StandardUnwrapped::$variant(actual) => {
                assert_eq!(
                    actual as *const _ as *const (),
                    expected as *const _ as *const ()
                );
            }
            other => panic!("unexpected inherited variant: {other:?}"),
        }
    }};
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), RdbcError> {
    assert_exact_identity!(ConnectionObject, dyn Connection, Connection);
    assert_exact_identity!(StatementObject, dyn Statement, Statement);
    assert_exact_identity!(
        PreparedStatementObject,
        dyn PreparedStatement,
        PreparedStatement
    );
    assert_exact_identity!(
        CallableStatementObject,
        dyn CallableStatement,
        CallableStatement
    );
    assert_exact_identity!(
        GenericResultSet::<u8, String, 4>(PhantomData),
        dyn ResultSet,
        ResultSet
    );
    assert_exact_identity!(
        DatabaseMetaDataObject,
        dyn DatabaseMetaData,
        DatabaseMetaData
    );
    assert_exact_identity!(
        ResultSetMetaDataObject,
        dyn ResultSetMetaData,
        ResultSetMetaData
    );
    assert_exact_identity!(
        ParameterMetaDataObject,
        dyn ParameterMetaData,
        ParameterMetaData
    );
    assert_exact_identity!(
        CommonDataSourceObject,
        dyn CommonDataSource,
        CommonDataSource
    );
    assert_exact_identity!(DataSourceObject, dyn DataSource, DataSource);
    assert_exact_identity!(
        PooledConnectionObject,
        dyn PooledConnection,
        PooledConnection
    );
    assert_exact_identity!(
        ConnectionPoolDataSourceObject,
        dyn ConnectionPoolDataSource,
        ConnectionPoolDataSource
    );
    assert_exact_identity!(XaConnectionObject, dyn XaConnection, XaConnection);
    assert_exact_identity!(XaDataSourceObject, dyn XaDataSource, XaDataSource);
    assert_exact_identity!(DriverObject, dyn Driver, Driver);
    assert_exact_identity!(
        ConnectionFactoryObject,
        dyn ConnectionFactory,
        ConnectionFactory
    );
    assert_exact_identity!(WrapperObject, dyn Wrapper, Wrapper);

    let callable = CallableStatementObject;
    assert_inherited_identity!(callable, dyn PreparedStatement, PreparedStatement);
    assert_inherited_identity!(callable, dyn Statement, Statement);

    let prepared = PreparedStatementObject;
    assert_inherited_identity!(prepared, dyn Statement, Statement);

    let data_source = DataSourceObject;
    assert_inherited_identity!(data_source, dyn CommonDataSource, CommonDataSource);

    let pooled_data_source = ConnectionPoolDataSourceObject;
    assert_inherited_identity!(pooled_data_source, dyn CommonDataSource, CommonDataSource);

    let xa_data_source = XaDataSourceObject;
    assert_inherited_identity!(xa_data_source, dyn CommonDataSource, CommonDataSource);

    let xa_connection = XaConnectionObject;
    assert_inherited_identity!(xa_connection, dyn PooledConnection, PooledConnection);

    struct UnknownTarget;
    let wrapper = WrapperObject;
    let unknown = Some(TypeId::of::<UnknownTarget>());
    assert!(!wrapper.is_wrapper_for(unknown)?);
    match wrapper.unwrap(unknown) {
        Err(RdbcError::SqlException(exception)) => {
            assert_eq!(exception.sql_state(), Some("0A000"));
        }
        other => panic!("unexpected unknown-target result: {other:?}"),
    }
    match wrapper.is_wrapper_for(None) {
        Err(RdbcError::InvalidArgument(message)) => assert!(message.contains("target")),
        other => panic!("unexpected null is_wrapper_for result: {other:?}"),
    }
    match wrapper.unwrap(None) {
        Err(RdbcError::InvalidArgument(message)) => assert!(message.contains("target")),
        other => panic!("unexpected null unwrap result: {other:?}"),
    }
    assert!(wrapper.is_wrapper_for_type::<WrapperObject>());
    let concrete = wrapper.unwrap_ref::<WrapperObject>()?;
    assert!(std::ptr::eq(concrete, &wrapper));
    Ok(())
}
