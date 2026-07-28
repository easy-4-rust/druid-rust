//! druid-core — trait-only abstraction layer for druid-rust.
//!
//! 对应 Druid Java 的核心接口层：Connection、DataSource、Filter、
//! FilterChain、ExceptionSorter、ValidConnectionChecker 等。
//!
//! 本 crate 不依赖任何数据库 driver、SQL 解析器或异步运行时，
//! 仅暴露 trait 契约，由下游 crate 实现。

pub mod config;
pub mod conn_state;
pub mod connection;
pub mod connection_ext;
pub mod connection_factory;
pub mod connection_holder;
pub mod driver;
pub mod druid_pooled_connection;
pub mod error;
pub mod exception_sorter;
pub mod exec_result;
pub mod filter;
pub mod filter_chain;
pub mod meta_data;
pub mod physical_connection;
pub mod physical_connection_capabilities;
pub mod physical_connection_factory;
pub mod physical_connection_lease;
pub mod pool;
pub mod pool_state;
pub mod pooled_connection;
pub mod row;
pub mod savepoint;
pub mod statement_type;
pub mod valid_connection_checker;
pub mod value;
pub mod wrapper;

pub use config::{PoolConfig, PoolConfigBuilder};
pub use conn_state::ConnState;
pub use connection_ext::ConnectionExt;
pub use connection_holder::{ConnectionHolder, ConnectionState};
pub use driver::Driver;
pub use druid_pooled_connection::DruidPooledConnection;
pub use druid_pooled_connection::DruidPooledConnection as PooledConnection;
pub use error::DruidError;
pub use exception_sorter::{
    ExceptionSorter, MySqlExceptionSorter, NullExceptionSorter, PgExceptionSorter,
};
pub use exec_result::ExecResult;
pub use filter::{AfterFilter, BeforeFilter, ExecContext};
pub use filter::{ClobEvent, DataSourceEvent, ExtendedFilter, StatementPropertyEvent};
pub use filter::{ConnectionEvent, ResultSetEvent, StatementEvent};
pub use filter_chain::FilterChain;
pub use meta_data::MetaData;
pub use physical_connection::PhysicalConnection;
pub use physical_connection::PhysicalConnection as Connection;
pub use physical_connection_capabilities::PhysicalConnectionCapabilities;
pub use physical_connection_factory::PhysicalConnectionFactory;
pub use physical_connection_factory::PhysicalConnectionFactory as ConnectionFactory;
pub use physical_connection_lease::PhysicalConnectionLease;
pub use pool::Pool;
pub use pool_state::PoolState;
pub use row::Row;
pub use savepoint::Savepoint;
pub use statement_type::StatementType;
pub use valid_connection_checker::{PingConnectionChecker, ValidConnectionChecker};
pub use value::Value;
pub use wrapper::Wrapper;
