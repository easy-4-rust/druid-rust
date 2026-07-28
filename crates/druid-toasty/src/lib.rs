//! druid-rust 内置 Toasty 数据源实现。
//!
//! Toasty 是内置标准驱动边界；本 crate 直接使用 Toasty 的
//! `Driver -> Connection` SPI，不包装 Toasty 自带连接池，避免 pool-in-pool。
//! SQLx、RBDC、bb8 与 deadpool 保持在 `druid-wrapper` 中作为扩展。

pub mod toasty_connection_adapter;
pub mod toasty_connection_factory;

pub use toasty_connection_adapter::ToastyConnectionAdapter;
pub use toasty_connection_factory::ToastyConnectionFactory;
