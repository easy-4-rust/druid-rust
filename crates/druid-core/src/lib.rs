//! druid-core — trait-only abstraction layer for druid-rust.
//!
//! 对应 Druid Java 的核心接口层：Connection、DataSource、Filter、
//! FilterChain、ExceptionSorter、ValidConnectionChecker 等。
//!
//! 本 crate 不依赖任何数据库 driver、SQL 解析器或异步运行时，
//! 仅暴露 trait 契约，由下游 crate 实现。

pub mod error;
pub mod value;
pub mod connection;
pub mod driver;
pub mod pool;
pub mod config;
pub mod pool_state;
pub mod connection_holder;
pub mod connection_factory;
pub mod pooled_connection;
pub mod filter;
pub mod filter_chain;
pub mod exception_sorter;
pub mod valid_connection_checker;
pub mod wrapper;

pub use config::{PoolConfig, PoolConfigBuilder};
pub use connection::{Connection, ExecResult, Row};
pub use connection_factory::ConnectionFactory;
pub use connection_holder::{ConnectionHolder, ConnectionState};
pub use connection::{ConnState, Savepoint};
pub use driver::Driver;
pub use error::DruidError;
pub use exception_sorter::{ExceptionSorter, MySqlExceptionSorter, NullExceptionSorter, PgExceptionSorter};
pub use filter::{AfterFilter, BeforeFilter, ExecContext};
pub use filter_chain::FilterChain;
pub use pool::Pool;
pub use pool_state::PoolState;
pub use pooled_connection::PooledConnection;
pub use valid_connection_checker::{PingConnectionChecker, ValidConnectionChecker};
pub use value::Value;
pub use wrapper::Wrapper;
