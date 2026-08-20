//! 对应 Java 类：com.alibaba.druid.pool.ha.selector.DataSourceSelector（路由提示）

/// SQL 路由提示。
///
/// 对应 Druid Java 中 DataSourceSelector 的选择策略。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlHint {
    /// 写操作，路由到主库
    Write,
    /// 读操作，路由到从库
    Read,
    /// 自动判断（根据 SQL 类型）
    Auto,
}
