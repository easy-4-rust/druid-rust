//! druid-rust 核心 SPI 与强类型 JDBC 语义层。
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
pub mod java_string;
pub mod jdbc4_valid_connection_checker;
pub mod jdbc_array;
pub mod jdbc_blob;
pub mod jdbc_calendar;
pub mod jdbc_clob;
pub mod jdbc_input_stream;
pub mod jdbc_n_clob;
pub mod jdbc_object;
pub mod jdbc_opaque_object;
pub mod jdbc_output_stream;
pub mod jdbc_parameter;
pub mod jdbc_parameter_date;
pub mod jdbc_parameter_decimal;
pub mod jdbc_parameter_impl;
pub mod jdbc_parameter_int;
pub mod jdbc_parameter_long;
pub mod jdbc_parameter_null;
pub mod jdbc_parameter_string;
pub mod jdbc_parameter_timestamp;
pub mod jdbc_reader;
pub mod jdbc_ref;
pub mod jdbc_result_set;
pub mod jdbc_row_id;
pub mod jdbc_sql_xml;
pub mod jdbc_target_type;
pub mod jdbc_type_map;
pub mod jdbc_url;
pub mod jdbc_writer;
pub mod jdbc_xml_representation_type;
pub mod jdbc_xml_result;
pub mod jdbc_xml_source;
pub mod log_filter;
pub mod meta_data;
pub mod mock_exception_sorter;
pub mod ms_sql_valid_connection_checker;
pub mod my_sql_exception_sorter;
pub mod my_sql_valid_connection_checker;
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

pub use abstract_oracle_exception_sorter::{
    AbstractOracleExceptionSorter, ORACLE_FATAL_ERROR_CODES_PROPERTY,
};
pub use auto_load::AutoLoad;
pub use callable_input_parameter::CallableInputParameter;
pub use callable_out_parameter::CallableOutParameter;
pub use callable_parameter::CallableParameter;
pub use config::{PoolConfig, PoolConfigBuilder};
pub use conn_state::ConnState;
pub use connection_defaults::ConnectionDefaults;
pub use connection_event_listener::ConnectionEventListener;
pub use connection_ext::ConnectionExt;
pub use connection_recycle_disposition::ConnectionRecycleDisposition;
pub use database_meta_data_proxy_impl::DatabaseMetaDataProxyImpl;
pub use database_meta_data_row_id_lifetime::DatabaseMetaDataRowIdLifetime;
pub use db2_exception_sorter::Db2ExceptionSorter;
pub use driver::Driver;
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
pub use error::DruidError;
pub use exception_sorter::{ExceptionSorter, ExceptionSorterProperties};
pub use exec_result::ExecResult;
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
    ConnectionDatabaseMetaDataFilterChain, ConnectionWarningFilterChain,
    DataSourceConnectionProvider, DataSourceGetConnectionFilterChain,
    DataSourceReleaseConnectionFilterChain, FilterChainImpl, PhysicalConnectionCloseContext,
    PhysicalConnectionCloseFilterChain, StatementWarningFilterChain,
};
pub use filter_event_adapter::{FilterEventAdapter, FilterEventListener};
pub use filter_manager::FilterManager;
pub use informix_exception_sorter::InformixExceptionSorter;
pub use java_string::JavaString;
pub use jdbc4_valid_connection_checker::{Jdbc4ValidConnectionChecker, PingConnectionChecker};
pub use jdbc_array::{JdbcArray, PhysicalArray};
pub use jdbc_blob::{JdbcBlob, PhysicalBlob};
pub use jdbc_calendar::{
    JdbcCalendar, JdbcCalendar as CallableCalendar, JdbcCalendarArgument,
    JdbcCalendarArgument as CallableCalendarArgument,
};
pub use jdbc_clob::{JdbcClob, PhysicalClob};
pub use jdbc_input_stream::{JdbcInputStream, JdbcStreamLength};
pub use jdbc_n_clob::{JdbcNClob, PhysicalNClob};
pub use jdbc_object::{JdbcObject, JdbcObject as CallableOutputValue};
pub use jdbc_opaque_object::{JdbcOpaqueObject, PhysicalJdbcOpaqueObject};
pub use jdbc_output_stream::JdbcOutputStream;
pub use jdbc_parameter::{JdbcParameter, JdbcParameterType, JdbcParameterValue};
pub use jdbc_parameter_date::JdbcParameterDate;
pub use jdbc_parameter_decimal::JdbcParameterDecimal;
pub use jdbc_parameter_impl::JdbcParameterImpl;
pub use jdbc_parameter_int::JdbcParameterInt;
pub use jdbc_parameter_long::JdbcParameterLong;
pub use jdbc_parameter_null::JdbcParameterNull;
pub use jdbc_parameter_string::JdbcParameterString;
pub use jdbc_parameter_timestamp::JdbcParameterTimestamp;
pub use jdbc_reader::{JdbcCharacterLength, JdbcReader, PhysicalCharacterReader};
pub use jdbc_ref::{JdbcRef, PhysicalRef};
pub use jdbc_result_set::{JdbcResultSet, PhysicalResultSet, RowSetResultSet};
pub use jdbc_row_id::JdbcRowId;
pub use jdbc_sql_xml::{JdbcSqlXml, PhysicalSqlXml};
pub use jdbc_target_type::JdbcTargetType;
pub use jdbc_target_type::JdbcTargetType as CallableTargetType;
pub use jdbc_type_map::{JdbcTypeMap, JdbcTypeMap as CallableTypeMap};
pub use jdbc_url::JdbcUrl;
pub use jdbc_writer::{JdbcWriter, PhysicalCharacterWriter};
pub use jdbc_xml_representation_type::JdbcXmlRepresentationType;
pub use jdbc_xml_result::{JdbcXmlResult, PhysicalXmlResult};
pub use jdbc_xml_source::{JdbcXmlSource, PhysicalXmlSource};
pub use log_filter::LogFilter;
pub use meta_data::MetaData;
pub use mock_exception_sorter::MockExceptionSorter;
pub use ms_sql_valid_connection_checker::MsSqlValidConnectionChecker;
pub use my_sql_exception_sorter::MySqlExceptionSorter;
pub use my_sql_valid_connection_checker::MySqlValidConnectionChecker;
pub use null_exception_sorter::NullExceptionSorter;
pub use ocean_base_oracle_exception_sorter::OceanBaseOracleExceptionSorter;
pub use ocean_base_valid_connection_checker::OceanBaseValidConnectionChecker;
pub use oracle_exception_sorter::OracleExceptionSorter;
pub use oracle_valid_connection_checker::OracleValidConnectionChecker;
pub use pg_exception_sorter::PgExceptionSorter;
pub use pg_valid_connection_checker::PgValidConnectionChecker;
pub use phoenix_exception_sorter::PhoenixExceptionSorter;
pub use physical_callable_statement::PhysicalCallableStatement;
pub use physical_connection::PhysicalConnection;
pub use physical_connection::PhysicalConnection as Connection;
pub use physical_connection_capabilities::PhysicalConnectionCapabilities;
pub use physical_connection_factory::PhysicalConnectionFactory;
pub use physical_connection_factory::PhysicalConnectionFactory as ConnectionFactory;
pub use physical_connection_info::PhysicalConnectionInfo;
pub use physical_connection_lease::PhysicalConnectionLease;
pub use physical_database_meta_data::PhysicalDatabaseMetaData;
pub use physical_prepared_statement::{PhysicalPreparedStatement, SqlTextPreparedStatement};
pub use physical_result_set_meta_data::PhysicalResultSetMetaData;
pub use physical_statement::{
    PhysicalStatement, PhysicalStatementOptions, SqlTextStatement, StatementExecuteResult,
    StatementGeneratedKeys,
};
pub use pool::Pool;
pub use pool_state::PoolState;
pub use poolable_wrapper::PoolableWrapper;
pub use prepared_input_parameter::{PreparedInputParameter, PreparedTypeNameArgument};
pub use prepared_statement_cache_stats::PreparedStatementCacheStats;
pub use prepared_statement_holder::PreparedStatementHolder;
pub use prepared_statement_key::{PreparedStatementKey, PreparedStatementMethodType};
pub use prepared_statement_pool::PreparedStatementPool;
pub use proxy_attributes::{ProxyAttributeValue, ProxyAttributes};
pub use result_set_column_meta::ResultSetColumnMeta;
pub use result_set_column_type::ResultSetColumnType;
pub use result_set_filter::ResultSetFilter;
pub use result_set_filter_chain::ResultSetFilterChain;
pub use result_set_filter_context::ResultSetFilterContext;
pub use result_set_meta_data::ResultSetMetaData;
pub use result_set_meta_data_proxy::ResultSetMetaDataProxy;
pub use result_set_meta_data_proxy_impl::ResultSetMetaDataProxyImpl;
pub use result_set_nullability::ResultSetNullability;
pub use result_set_open_context::ResultSetOpenContext;
pub use result_set_statement::ResultSetStatement;
pub use result_set_update::ResultSetUpdate;
pub use row::Row;
pub use savepoint::Savepoint;
pub use sql_exception::{SqlException, SqlExceptionCause};
pub use sql_warning::SqlWarning;
pub use statement_event_listener::StatementEventListener;
pub use statement_execute_type::StatementExecuteType;
pub use statement_type::StatementType;
pub use sybase_exception_sorter::SybaseExceptionSorter;
pub use transaction_info::TransactionInfo;
pub use valid_connection_checker::ValidConnectionChecker;
pub use valid_connection_checker_adapter::ValidConnectionCheckerAdapter;
pub use value::Value;
pub use wrapper::{Unwrapped, Wrapper, WrapperExt};
pub use wrapper_adapter::WrapperAdapter;
