//! Wall 多租户回调协议。
//!
//! 对应 Java：`com.alibaba.druid.wall.WallConfig.TenantCallBack` 及其内部
//! `StatementType`。Java 使用 `Object` 表达租户值；Rust 使用 RDBC 公共
//! [`Value`] 保留数据库标量类型。

use crate::core::Value;

/// 多租户 SQL 操作类型。
///
/// 对应 Java：`WallConfig.TenantCallBack.StatementType`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TenantStatementType {
    /// SELECT。
    Select,
    /// UPDATE。
    Update,
    /// INSERT。
    Insert,
    /// DELETE。
    Delete,
}

/// 为 Wall SQL 改写和 ResultSet 租户列过滤提供业务回调。
///
/// 对应 Java：`WallConfig.TenantCallBack`。回调由调用方保证线程安全，因而可在
/// Tokio 多线程运行时中共享；它不依赖 Java `ThreadLocal`。
pub trait TenantCallBack: Send + Sync {
    /// 返回指定语句与表的租户值；`None` 对应 Java `null`。
    fn tenant_value(&self, statement_type: TenantStatementType, table_name: &str) -> Option<Value>;

    /// 返回指定语句与表的租户列名；`None` 对应 Java `null`。
    fn tenant_column(
        &self,
        statement_type: TenantStatementType,
        table_name: &str,
    ) -> Option<String>;

    /// 返回 ResultSet 中应隐藏的物理列名；`None` 对应 Java `null`。
    fn hidden_column(&self, table_name: &str) -> Option<String>;

    /// 在成功移动到一行且结果中包含租户列时接收该列值。
    ///
    /// 对应 Java：`TenantCallBack#filterResultsetTenantColumn(Object)`。
    fn filter_resultset_tenant_column(&self, value: &Value);
}
