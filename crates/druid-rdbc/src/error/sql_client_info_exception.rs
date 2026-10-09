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
