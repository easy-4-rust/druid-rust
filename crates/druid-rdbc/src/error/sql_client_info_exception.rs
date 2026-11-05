use super::SqlException;
use std::collections::HashMap;

/// 设置一个或多个连接客户端信息属性失败时的无损诊断。
///
/// 对应 Java: java.sql.SQLClientInfoException。failed_properties 只保存被拒绝的
/// 属性及其状态名；属性名大小写、状态文本和原 SQL 异常均原样保留。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlClientInfoException {
    /// 原始 SQL 异常，保留 SQLState、厂商代码、消息和异常链。
    pub exception: SqlException,
    /// 被拒绝的属性名及状态名，保留既有字符串 payload 合同。
    pub failed_properties: HashMap<String, String>,
}

impl SqlClientInfoException {
    /// 以完整的 SQL 异常与被拒绝属性集创建诊断。
    ///
    /// 对应 Java: java.sql.SQLClientInfoException 的全部 9 个构造重载——reason、
    /// SQLState、vendor code 与 cause 已封装在 `exception` 中，调用方按所要模拟的
    /// Java 重载选择 `SqlException::new(...)` 的参数形状。
    pub fn new(exception: SqlException, failed_properties: HashMap<String, String>) -> Self {
        Self {
            exception,
            failed_properties,
        }
    }

    /// 返回被拒绝属性集的只读视图。
    ///
    /// 对应 Java: java.sql.SQLClientInfoException#getFailedProperties。
    pub fn failed_properties(&self) -> &HashMap<String, String> {
        &self.failed_properties
    }
}
