use druid_rdbc::connection::Connection;
use druid_rdbc::data_source::{
    CommonDataSource, ConnectionPoolDataSource, DataSource, PooledConnection, XaConnection,
    XaDataSource,
};
use druid_rdbc::driver::{ConnectionFactory, Driver, DriverRegistry};
use druid_rdbc::extensions::{ConnectionCapabilities, ExecResult};
use druid_rdbc::metadata::{
    DatabaseMetaData, ParameterMetaData, ResultSetColumnMeta, ResultSetColumnType,
    ResultSetMetaData, ResultSetMetaDataSnapshot, ResultSetNullability,
};
use druid_rdbc::result_set::{ResultSet, ResultSetUpdate};
use druid_rdbc::statement::{
    CallableInputParameter, CallableParameter, CallableStatement, PreparedInputParameter,
    PreparedStatement, PreparedTypeNameArgument, Statement, StatementExecuteResult,
    StatementOptions,
};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper, WrapperExt};

fn assert_send_static<T: Send + 'static + ?Sized>() {}

fn assert_send_sync_static<T: Send + Sync + 'static + ?Sized>() {}

fn assert_wrapper<T: Wrapper + ?Sized>() {}

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
    assert!(capabilities.transactions);
    assert!(!capabilities.savepoints);

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
