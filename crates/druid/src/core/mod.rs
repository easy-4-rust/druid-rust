//! druid-rust 核心 SPI 与强类型 RDBC 语义层。
//!
//! 对应 Druid Java 的核心接口层：Connection、DataSource、Filter、
//! FilterChain、ExceptionSorter、ValidConnectionChecker 等。
//!
//! 本模块不依赖具体数据库 driver；除 trait 契约外，还暴露 Decimal、日期时间和
//! Callable 重载所需的强类型平台值，由 `toasty` 或 `druid-wrapper` Adapter
//! 实现具体驱动语义。

pub mod abstract_oracle_exception_sorter;
pub mod auto_load;
pub mod callable_input_parameter;
pub mod callable_out_parameter;
pub mod callable_parameter;
pub mod clob_proxy;
pub mod clob_proxy_impl;
pub mod config;
pub mod conn_state;
pub mod connection;
pub mod connection_defaults;
pub mod connection_event_listener;
pub mod connection_ext;
pub mod connection_factory;
pub mod connection_recycle_disposition;
pub mod database_meta_data_proxy_impl;
pub mod database_meta_data_row_id_lifetime;
pub mod db2_exception_sorter;
pub mod driver;
pub mod druid_connection_holder;
pub mod druid_pooled_callable_statement;
pub mod druid_pooled_connection;
pub mod druid_pooled_prepared_statement;
pub mod druid_pooled_result_set;
pub mod druid_pooled_statement;
pub mod error;
pub mod exception_sorter;
pub mod exec_result;
pub(crate) mod fatal_error_handler;
pub mod filter;
pub mod filter_adapter;
pub mod filter_chain;
pub mod filter_chain_impl;
pub mod filter_event_adapter;
pub mod filter_manager;
pub mod informix_exception_sorter;
pub mod log_filter;
pub mod meta_data;
pub mod mock_exception_sorter;
pub mod ms_sql_valid_connection_checker;
pub mod my_sql_exception_sorter;
pub mod my_sql_valid_connection_checker;
pub mod n_clob_proxy;
pub mod n_clob_proxy_impl;
pub mod null_exception_sorter;
pub mod ocean_base_oracle_exception_sorter;
pub mod ocean_base_valid_connection_checker;
pub mod oracle_exception_sorter;
pub mod oracle_valid_connection_checker;
pub mod pg_exception_sorter;
pub mod pg_valid_connection_checker;
pub mod phoenix_exception_sorter;
pub mod physical_callable_statement;
pub mod physical_connection;
pub mod physical_connection_capabilities;
pub mod physical_connection_factory;
pub mod physical_connection_info;
pub mod physical_connection_lease;
pub mod physical_database_meta_data;
pub mod physical_prepared_statement;
pub mod physical_result_set_meta_data;
pub mod physical_statement;
pub mod pool;
pub mod pool_state;
pub mod poolable_wrapper;
pub mod pooled_connection;
pub mod prepared_input_parameter;
pub mod prepared_statement_cache_stats;
pub mod prepared_statement_holder;
pub mod prepared_statement_key;
mod prepared_statement_physical_statement;
pub mod prepared_statement_pool;
pub mod proxy_attributes;
pub mod rdbc4_valid_connection_checker;
pub mod rdbc_calendar;
pub mod rdbc_input_stream;
pub mod rdbc_object;
pub mod rdbc_opaque_object;
pub mod rdbc_output_stream;
pub mod rdbc_parameter;
pub mod rdbc_parameter_date;
pub mod rdbc_parameter_decimal;
pub mod rdbc_parameter_impl;
pub mod rdbc_parameter_int;
pub mod rdbc_parameter_long;
pub mod rdbc_parameter_null;
pub mod rdbc_parameter_string;
pub mod rdbc_parameter_timestamp;
pub mod rdbc_reader;
pub mod rdbc_result_set;
pub mod rdbc_row_id;
pub mod rdbc_string;
pub mod rdbc_target_type;
pub mod rdbc_type_map;
pub mod rdbc_url;
pub mod rdbc_writer;
pub mod rdbc_xml_representation_type;
pub mod rdbc_xml_result;
pub mod rdbc_xml_source;
pub mod result_set_column_meta;
pub mod result_set_column_type;
pub mod result_set_filter;
pub mod result_set_filter_chain;
pub mod result_set_filter_context;
pub mod result_set_meta_data;
pub mod result_set_meta_data_proxy;
pub mod result_set_meta_data_proxy_impl;
pub mod result_set_nullability;
pub mod result_set_open_context;
pub mod result_set_statement;
pub mod result_set_update;
pub mod row;
pub mod savepoint;
pub mod sql_exception;
pub mod sql_warning;
pub mod statement_event_listener;
pub mod statement_execute_type;
pub mod statement_type;
pub mod sybase_exception_sorter;
pub mod transaction_info;
pub mod valid_connection_checker;
pub mod valid_connection_checker_adapter;
pub mod value;
pub mod wrapper;
pub mod wrapper_adapter;
pub mod xa;

pub use abstract_oracle_exception_sorter::{
    AbstractOracleExceptionSorter, ORACLE_FATAL_ERROR_CODES_PROPERTY,
};
pub use auto_load::AutoLoad;
pub use clob_proxy::ClobProxy;
pub use clob_proxy_impl::ClobProxyImpl;
pub use config::{PoolConfig, PoolConfigBuilder};
pub use conn_state::ConnState;
pub use connection_defaults::ConnectionDefaults;
pub use connection_event_listener::ConnectionEventListener;
pub use connection_ext::ConnectionExt;
pub use connection_recycle_disposition::ConnectionRecycleDisposition;
pub use database_meta_data_proxy_impl::DatabaseMetaDataProxyImpl;
pub use db2_exception_sorter::Db2ExceptionSorter;
pub use driver::{Driver, DriverProperty};
pub use druid_connection_holder::{
    ConnectionState, DruidConnectionHolder, DruidConnectionHolder as ConnectionHolder,
};
pub use druid_pooled_callable_statement::{
    DruidPooledCallableStatement, DruidPooledCallableStatementHandle,
};
pub use druid_pooled_connection::DruidPooledConnection;
pub use druid_pooled_connection::DruidPooledConnection as PooledConnection;
pub use druid_pooled_prepared_statement::{
    DruidPooledPreparedStatement, DruidPooledPreparedStatementHandle,
};
pub use druid_pooled_result_set::DruidPooledResultSet;
pub use druid_pooled_statement::DruidPooledStatement;
pub use druid_rdbc::error::{
    RdbcError, SqlException, SqlExceptionCause, SqlExceptionKind, SqlWarning,
};
pub use druid_rdbc::resource::RdbcOutputStream;
pub use druid_rdbc::resource::RdbcXmlRepresentationType;
pub use druid_rdbc::resource::{PhysicalCharacterReader, RdbcCharacterLength, RdbcReader};
pub use druid_rdbc::resource::{PhysicalCharacterWriter, RdbcWriter};
pub use druid_rdbc::resource::{PhysicalXmlResult, RdbcXmlResult};
pub use druid_rdbc::resource::{PhysicalXmlSource, RdbcXmlSource};
pub use druid_rdbc::resource::{RdbcArray, RdbcBlob, RdbcClob, RdbcNClob, RdbcRef, RdbcSqlXml};
pub use druid_rdbc::resource::{RdbcInputStream, RdbcStreamLength};
pub use druid_rdbc::spi::CallableInputParameter;
pub use druid_rdbc::spi::CallableOutParameter;
pub use druid_rdbc::spi::CallableParameter;
pub use druid_rdbc::spi::DatabaseMetaDataRowIdLifetime;
pub use druid_rdbc::spi::ExecResult;
pub use druid_rdbc::spi::PhysicalCallableStatement;
pub use druid_rdbc::spi::PhysicalConnection;
pub use druid_rdbc::spi::PhysicalConnection as Connection;
pub use druid_rdbc::spi::PhysicalConnectionCapabilities;
pub use druid_rdbc::spi::PhysicalConnectionFactory;
pub use druid_rdbc::spi::PhysicalConnectionFactory as ConnectionFactory;
pub use druid_rdbc::spi::PhysicalConnectionInfo;
pub use druid_rdbc::spi::PhysicalDatabaseMetaData;
pub use druid_rdbc::spi::PhysicalResultSetMetaData;
pub use druid_rdbc::spi::ResultSetColumnMeta;
pub use druid_rdbc::spi::ResultSetColumnType;
pub use druid_rdbc::spi::ResultSetMetaData;
pub use druid_rdbc::spi::ResultSetNullability;
pub use druid_rdbc::spi::ResultSetUpdate;
pub use druid_rdbc::spi::{PhysicalPreparedStatement, SqlTextPreparedStatement};
pub use druid_rdbc::spi::{PhysicalResultSet, RdbcResultSet, RowSetResultSet};
pub use druid_rdbc::spi::{
    PhysicalStatement, PhysicalStatementOptions, SqlTextStatement, StatementExecuteResult,
    StatementGeneratedKeys,
};
pub use druid_rdbc::spi::{PreparedInputParameter, PreparedTypeNameArgument};
pub use druid_rdbc::spi::{PreparedStatementKey, PreparedStatementMethodType};
pub use druid_rdbc::spi::{Unwrapped, Wrapper, WrapperExt};
pub use druid_rdbc::types::RdbcParameterDate;
pub use druid_rdbc::types::RdbcParameterDecimal;
pub use druid_rdbc::types::RdbcParameterImpl;
pub use druid_rdbc::types::RdbcParameterInt;
pub use druid_rdbc::types::RdbcParameterLong;
pub use druid_rdbc::types::RdbcParameterNull;
pub use druid_rdbc::types::RdbcParameterString;
pub use druid_rdbc::types::RdbcParameterTimestamp;
pub use druid_rdbc::types::RdbcRowId;
pub use druid_rdbc::types::RdbcTargetType;
pub use druid_rdbc::types::RdbcTargetType as CallableTargetType;
pub use druid_rdbc::types::{PhysicalRdbcOpaqueObject, RdbcOpaqueObject};
pub use druid_rdbc::types::{
    RdbcCalendar, RdbcCalendar as CallableCalendar, RdbcCalendarArgument,
    RdbcCalendarArgument as CallableCalendarArgument,
};
pub use druid_rdbc::types::{RdbcObject, RdbcObject as CallableOutputValue};
pub use druid_rdbc::types::{RdbcParameter, RdbcParameterType, RdbcParameterValue};
pub use druid_rdbc::types::{RdbcString, RdbcUrl, Row, Savepoint, Value};
pub use druid_rdbc::types::{RdbcTypeMap, RdbcTypeMap as CallableTypeMap};
pub use error::DruidError;
pub use exception_sorter::{ExceptionSorter, ExceptionSorterProperties};
#[allow(deprecated)]
pub use filter::config::{ConfigFilter, ConfigTools};
#[allow(deprecated)]
pub use filter::encoding::{CharsetConvert, CharsetParameter, EncodingConvertFilter};
pub use filter::mysql8datetime::{MySQL8DateTimeResultSetMetaData, MySQL8DateTimeSqlTypeFilter};
pub use filter::{
    AfterFilter, BatchExecContext, BatchExecKind, BeforeFilter, ConnectionEventContext,
    ExecContext, ExecOperation, StatementEventContext,
};
pub use filter::{ClobEvent, DataSourceEvent, ExtendedFilter, StatementPropertyEvent};
pub use filter::{ConnectionEvent, ResultSetEvent, StatementEvent};
pub use filter_adapter::FilterAdapter;
pub use filter_chain::FilterChain;
pub use filter_chain_impl::{
    ClobFilterChain, ConnectionDatabaseMetaDataFilterChain, ConnectionLobFilterChain,
    ConnectionWarningFilterChain, DataSourceConnectionProvider, DataSourceGetConnectionFilterChain,
    DataSourceReleaseConnectionFilterChain, FilterChainImpl, PhysicalConnectionCloseContext,
    PhysicalConnectionCloseFilterChain, PhysicalConnectionConnectFilterChain,
    PhysicalConnectionConnectResult, StatementWarningFilterChain,
};
pub use filter_event_adapter::{FilterEventAdapter, FilterEventListener};
pub use filter_manager::FilterManager;
pub use informix_exception_sorter::InformixExceptionSorter;
pub use log_filter::LogFilter;
pub use meta_data::MetaData;
pub use mock_exception_sorter::MockExceptionSorter;
pub use ms_sql_valid_connection_checker::MsSqlValidConnectionChecker;
pub use my_sql_exception_sorter::MySqlExceptionSorter;
pub use my_sql_valid_connection_checker::MySqlValidConnectionChecker;
pub use n_clob_proxy::NClobProxy;
pub use n_clob_proxy_impl::NClobProxyImpl;
pub use null_exception_sorter::NullExceptionSorter;
pub use ocean_base_oracle_exception_sorter::OceanBaseOracleExceptionSorter;
pub use ocean_base_valid_connection_checker::OceanBaseValidConnectionChecker;
pub use oracle_exception_sorter::OracleExceptionSorter;
pub use oracle_valid_connection_checker::OracleValidConnectionChecker;
pub use pg_exception_sorter::PgExceptionSorter;
pub use pg_valid_connection_checker::PgValidConnectionChecker;
pub use phoenix_exception_sorter::PhoenixExceptionSorter;
pub use physical_connection_lease::PhysicalConnectionLease;
pub use pool::Pool;
pub use pool_state::PoolState;
pub use poolable_wrapper::PoolableWrapper;
pub use prepared_statement_cache_stats::PreparedStatementCacheStats;
pub use prepared_statement_holder::PreparedStatementHolder;
pub use prepared_statement_pool::PreparedStatementPool;
pub use proxy_attributes::{ProxyAttributeValue, ProxyAttributes};
pub use rdbc4_valid_connection_checker::{PingConnectionChecker, Rdbc4ValidConnectionChecker};
pub use result_set_filter::ResultSetFilter;
pub use result_set_filter_chain::ResultSetFilterChain;
pub use result_set_filter_context::ResultSetFilterContext;
pub use result_set_meta_data_proxy::ResultSetMetaDataProxy;
pub use result_set_meta_data_proxy_impl::ResultSetMetaDataProxyImpl;
pub use result_set_open_context::ResultSetOpenContext;
pub use result_set_statement::ResultSetStatement;
pub use statement_event_listener::StatementEventListener;
pub use statement_execute_type::StatementExecuteType;
pub use statement_type::StatementType;
pub use sybase_exception_sorter::SybaseExceptionSorter;
pub use transaction_info::TransactionInfo;
pub use valid_connection_checker::ValidConnectionChecker;
pub use valid_connection_checker_adapter::ValidConnectionCheckerAdapter;
pub use wrapper_adapter::WrapperAdapter;
pub use xa::{
    flags as xa_flags, XaOperation, XaPrepareResult, XaResource, XaState, XaStateTransitionError,
    XaStateTransitionRecord, XaTransactionState, Xid,
};

/// Look up a driver extension by database type.
///
/// Delegates to [`crate::spi::driver_extension_registry::lookup_driver_extension`].
pub fn lookup_driver_extension(
    db_type: &str,
) -> Result<&'static crate::spi::driver_extension_descriptor::DriverExtensionDescriptor, RdbcError>
{
    crate::spi::driver_extension_registry::lookup_driver_extension(db_type)
}
