//! druid-rust SQLx + deadpool 外部连接池桥接。

pub mod sqlx_deadpool_connection_manager;
pub mod sqlx_deadpool_pool;

pub use sqlx_deadpool_connection_manager::SqlxDeadpoolConnectionManager;
pub use sqlx_deadpool_pool::SqlxDeadpoolPool;
/// 旧设计阶段名称兼容导出；新代码应使用 `SqlxDeadpoolPool`。
#[deprecated(note = "use SqlxDeadpoolPool; this object is a Pool, not a ConnectionFactory")]
pub use sqlx_deadpool_pool::SqlxDeadpoolPool as SqlxDeadpoolAdapter;
