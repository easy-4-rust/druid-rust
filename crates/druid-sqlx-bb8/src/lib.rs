//! druid-rust SQLx + bb8 外部连接池桥接。

pub mod sqlx_bb8_connection_manager;
pub mod sqlx_bb8_pool;

pub use sqlx_bb8_connection_manager::SqlxBb8ConnectionManager;
pub use sqlx_bb8_pool::SqlxBb8Pool;
/// 旧设计阶段名称兼容导出；新代码应使用 `SqlxBb8Pool`。
#[deprecated(note = "use SqlxBb8Pool; this object is a Pool, not a ConnectionFactory")]
pub use sqlx_bb8_pool::SqlxBb8Pool as SqlxBb8Adapter;
