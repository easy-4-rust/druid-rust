//! 物理预编译语句 SPI。
//!
//! 对应 Java 平台依赖：`java.sql.PreparedStatement`。

use super::{DruidError, PhysicalCallableStatement};
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};

/// 驱动预编译语句句柄。
///
/// 该对象只承载驱动 prepare 的结果；参数、Filter、缓存命中和逻辑关闭属于
/// `DruidPooledPreparedStatement`。SQLx/RBDC Adapter 可以保存各自的 statement
/// 元数据，但不得把驱动类型泄漏到公共 Druid API。
pub trait PhysicalPreparedStatement: Send + Sync {
    /// 返回原始 SQL。
    fn sql(&self) -> &str;

    /// 返回驱动 Adapter 用于类型检查的只读动态对象。
    fn as_any(&self) -> &dyn Any;

    /// 返回 CallableStatement 能力；普通 PreparedStatement 返回 `None`。
    fn as_callable(&self) -> Option<&dyn PhysicalCallableStatement> {
        None
    }

    /// 清理上一次执行设置的参数。
    ///
    /// Rust API 每次执行显式传入参数；Adapter 有额外缓存时应覆盖本方法。
    fn clear_parameters(&self) -> Result<(), DruidError> {
        Ok(())
    }

    /// 清理批处理参数。
    fn clear_batch(&self) -> Result<(), DruidError> {
        Ok(())
    }

    /// 关闭物理语句句柄。
    fn close(&self) -> Result<(), DruidError>;

    /// 返回语句是否已经关闭。
    fn is_closed(&self) -> bool;
}

/// 仅保存 SQL 文本的驱动语句句柄。
///
/// 用于 RBDC 等由连接执行入口内部完成 prepare/cache 的生态接口；它不是公开
/// pooled statement，也不伪造执行结果。
pub struct SqlTextPreparedStatement {
    sql: String,
    closed: AtomicBool,
}

impl SqlTextPreparedStatement {
    /// 创建 SQL 文本句柄。
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            closed: AtomicBool::new(false),
        }
    }
}

impl PhysicalPreparedStatement for SqlTextPreparedStatement {
    fn sql(&self) -> &str {
        &self.sql
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn close(&self) -> Result<(), DruidError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}
