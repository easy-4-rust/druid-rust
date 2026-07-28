//! 物理 JDBC `ResultSet` 资源句柄。
//!
//! 对应 Java 平台对象：`java.sql.ResultSet`。本对象先承载 Array 返回行集所需的
//! 身份和关闭生命周期；游标、metadata 与 pooled trace 继续由 ResultSet 专项迁移。

use super::DruidError;
use std::fmt;
use std::sync::Arc;

/// 物理结果集最小资源 SPI。
pub trait PhysicalResultSet: fmt::Debug + Send + Sync {
    /// 关闭物理结果集。
    fn close(&self) -> Result<(), DruidError>;

    /// 返回结果集是否关闭。
    fn is_closed(&self) -> bool;
}

/// 不泄漏具体驱动类型的结果集句柄。
#[derive(Clone)]
pub struct JdbcResultSet {
    physical: Arc<dyn PhysicalResultSet>,
}

impl JdbcResultSet {
    /// 包装物理结果集。
    pub fn new(physical: Arc<dyn PhysicalResultSet>) -> Self {
        Self { physical }
    }

    /// 关闭结果集。
    pub fn close(&self) -> Result<(), DruidError> {
        self.physical.close()
    }

    /// 返回是否关闭。
    pub fn is_closed(&self) -> bool {
        self.physical.is_closed()
    }

    /// 返回物理结果集 SPI。
    pub fn physical(&self) -> &dyn PhysicalResultSet {
        self.physical.as_ref()
    }
}

impl fmt::Debug for JdbcResultSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JdbcResultSet")
            .field("physical", &self.physical)
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl PartialEq for JdbcResultSet {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for JdbcResultSet {}
