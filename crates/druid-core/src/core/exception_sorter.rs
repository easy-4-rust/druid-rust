//! 数据库异常致命性判定协议。

use super::SqlException;
use std::collections::BTreeMap;

/// Java `Properties` 到 Rust 的确定性键值映射。
pub type ExceptionSorterProperties = BTreeMap<String, String>;

/// 判断 SQL 异常是否意味着物理连接不可继续复用。
///
/// 对应 Java: `com.alibaba.druid.pool.ExceptionSorter`。输入保留
/// `SQLException` 的 error code、SQLState、具体异常类型、消息与 cause 链，
/// 不能再用 `(error_code, message)` 近似。
pub trait ExceptionSorter: Send + Sync {
    /// 返回具体 Rust Sorter 类型名，供管理快照识别实际装配。
    fn class_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    /// 返回异常是否为致命连接异常。
    ///
    /// 参数 `exception` 对应 Java `SQLException e`；返回 `true` 时连接池必须
    /// 丢弃物理连接。
    fn is_exception_fatal(&self, exception: &SqlException) -> bool;

    /// 从连接属性配置 sorter。
    ///
    /// 参数 `properties` 对应 Java 可空 `Properties`。无配置对象也必须显式
    /// 接受 `None`，以保留 Java 调用边界。
    fn config_from_properties(&mut self, properties: Option<&ExceptionSorterProperties>);
}
