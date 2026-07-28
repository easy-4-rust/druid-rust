//! druid-rust 核心 SPI 与强类型 JDBC 语义层。
//!
//! 对应 Druid Java 的核心接口层：Connection、DataSource、Filter、
//! FilterChain、ExceptionSorter、ValidConnectionChecker 等。
//!
//! 本模块不依赖具体数据库 driver；除 trait 契约外，还暴露 Decimal、日期时间和
//! Callable 重载所需的强类型平台值，由 `toasty` 或 `druid-wrapper` Adapter
//! 实现具体驱动语义。

pub mod callable_calendar;
pub mod callable_input_parameter;
pub mod callable_out_parameter;
pub mod callable_output_value;
pub mod callable_parameter;
pub mod callable_statement_unwrap;
pub mod callable_target_type;
pub mod callable_type_map;
pub mod config;
pub mod conn_state;
pub mod connection;
pub mod connection_defaults;
pub mod connection_ext;
pub mod connection_factory;
pub mod connection_recycle_disposition;
pub mod driver;
pub mod druid_connection_holder;
pub mod druid_pooled_callable_statement;
pub mod druid_pooled_connection;
pub mod druid_pooled_prepared_statement;
pub mod error;
pub mod exception_sorter;
pub mod exec_result;
pub mod filter;
pub mod filter_chain;
pub mod java_string;
pub mod jdbc_array;
pub mod jdbc_blob;
pub mod jdbc_clob;
pub mod jdbc_input_stream;
pub mod jdbc_n_clob;
pub mod jdbc_output_stream;
pub mod jdbc_reader;
pub mod jdbc_ref;
pub mod jdbc_result_set;
pub mod jdbc_row_id;
pub mod jdbc_sql_xml;
pub mod jdbc_url;
pub mod jdbc_writer;
pub mod jdbc_xml_representation_type;
pub mod jdbc_xml_result;
pub mod jdbc_xml_source;
pub mod meta_data;
pub mod physical_callable_statement;
pub mod physical_connection;
pub mod physical_connection_capabilities;
pub mod physical_connection_factory;
pub mod physical_connection_lease;
pub mod physical_prepared_statement;
pub mod pool;
pub mod pool_state;
pub mod pooled_connection;
pub mod prepared_statement_cache_stats;
pub mod prepared_statement_holder;
pub mod prepared_statement_key;
pub mod prepared_statement_pool;
pub mod row;
pub mod savepoint;
pub mod statement_type;
pub mod valid_connection_checker;
pub mod value;
pub mod wrapper;

pub use callable_calendar::{CallableCalendar, CallableCalendarArgument};
pub use callable_input_parameter::CallableInputParameter;
pub use callable_out_parameter::CallableOutParameter;
pub use callable_output_value::CallableOutputValue;
pub use callable_parameter::CallableParameter;
pub use callable_statement_unwrap::{CallableStatementUnwrapTarget, CallableStatementUnwrapped};
pub use callable_target_type::CallableTargetType;
pub use callable_type_map::CallableTypeMap;
pub use config::{PoolConfig, PoolConfigBuilder};
pub use conn_state::ConnState;
pub use connection_defaults::ConnectionDefaults;
pub use connection_ext::ConnectionExt;
pub use connection_recycle_disposition::ConnectionRecycleDisposition;
pub use driver::Driver;
pub use druid_connection_holder::{
    ConnectionState, DruidConnectionHolder, DruidConnectionHolder as ConnectionHolder,
};
pub use druid_pooled_callable_statement::DruidPooledCallableStatement;
pub use druid_pooled_connection::DruidPooledConnection;
pub use druid_pooled_connection::DruidPooledConnection as PooledConnection;
pub use druid_pooled_prepared_statement::DruidPooledPreparedStatement;
pub use error::DruidError;
pub use exception_sorter::{
    ExceptionSorter, MySqlExceptionSorter, NullExceptionSorter, PgExceptionSorter,
};
pub use exec_result::ExecResult;
pub use filter::{AfterFilter, BeforeFilter, ExecContext};
pub use filter::{ClobEvent, DataSourceEvent, ExtendedFilter, StatementPropertyEvent};
pub use filter::{ConnectionEvent, ResultSetEvent, StatementEvent};
pub use filter_chain::FilterChain;
pub use java_string::JavaString;
pub use jdbc_array::{JdbcArray, PhysicalArray};
pub use jdbc_blob::{JdbcBlob, PhysicalBlob};
pub use jdbc_clob::{JdbcClob, PhysicalClob};
pub use jdbc_input_stream::{JdbcInputStream, JdbcStreamLength};
pub use jdbc_n_clob::{JdbcNClob, PhysicalNClob};
pub use jdbc_output_stream::JdbcOutputStream;
pub use jdbc_reader::{JdbcCharacterLength, JdbcReader, PhysicalCharacterReader};
pub use jdbc_ref::{JdbcRef, PhysicalRef};
pub use jdbc_result_set::{JdbcResultSet, PhysicalResultSet};
pub use jdbc_row_id::JdbcRowId;
pub use jdbc_sql_xml::{JdbcSqlXml, PhysicalSqlXml};
pub use jdbc_url::JdbcUrl;
pub use jdbc_writer::{JdbcWriter, PhysicalCharacterWriter};
pub use jdbc_xml_representation_type::JdbcXmlRepresentationType;
pub use jdbc_xml_result::{JdbcXmlResult, PhysicalXmlResult};
pub use jdbc_xml_source::{JdbcXmlSource, PhysicalXmlSource};
pub use meta_data::MetaData;
pub use physical_callable_statement::PhysicalCallableStatement;
pub use physical_connection::PhysicalConnection;
pub use physical_connection::PhysicalConnection as Connection;
pub use physical_connection_capabilities::PhysicalConnectionCapabilities;
pub use physical_connection_factory::PhysicalConnectionFactory;
pub use physical_connection_factory::PhysicalConnectionFactory as ConnectionFactory;
pub use physical_connection_lease::PhysicalConnectionLease;
pub use physical_prepared_statement::{PhysicalPreparedStatement, SqlTextPreparedStatement};
pub use pool::Pool;
pub use pool_state::PoolState;
pub use prepared_statement_cache_stats::PreparedStatementCacheStats;
pub use prepared_statement_holder::PreparedStatementHolder;
pub use prepared_statement_key::{PreparedStatementKey, PreparedStatementMethodType};
pub use prepared_statement_pool::PreparedStatementPool;
pub use row::Row;
pub use savepoint::Savepoint;
pub use statement_type::StatementType;
pub use valid_connection_checker::{PingConnectionChecker, ValidConnectionChecker};
pub use value::Value;
pub use wrapper::Wrapper;
