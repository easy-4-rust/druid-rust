//! 对应 Java 类：`com.alibaba.druid.stat.JdbcTraceManager`。

use std::sync::OnceLock;

/// 已废弃的 JDBC trace 管理器兼容单例。
///
/// 对应 Java: `com.alibaba.druid.stat.JdbcTraceManager`。原 Java
/// `JdbcTraceManagerMBean` 是空接口且 JMX 为 JVM 宿主边界，因此 Rust 只保留
/// 可观察的稳定单例身份，不创建 MBean 或 SLF4J 对象。
#[deprecated(note = "Java JdbcTraceManager 仅保留空 MBean 单例")]
pub struct JdbcTraceManager;

#[allow(deprecated)]
impl JdbcTraceManager {
    /// 返回进程级稳定单例。
    #[must_use]
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<JdbcTraceManager> = OnceLock::new();
        INSTANCE.get_or_init(|| JdbcTraceManager)
    }
}
