//! 对应 Java 类：`com.alibaba.druid.stat.RdbcTraceManager`。

use std::sync::OnceLock;

/// 已废弃的 RDBC trace 管理器兼容单例。
///
/// 对应 Java: `com.alibaba.druid.stat.RdbcTraceManager`。原 Java
/// `RdbcTraceManagerMBean` 是空接口且 JMX 为 JVM 宿主边界，因此 Rust 只保留
/// 可观察的稳定单例身份，不创建 MBean 或 SLF4J 对象。
#[deprecated(note = "Java RdbcTraceManager 仅保留空 MBean 单例")]
pub struct RdbcTraceManager;

#[allow(deprecated)]
impl RdbcTraceManager {
    /// 返回进程级稳定单例。
    #[must_use]
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<RdbcTraceManager> = OnceLock::new();
        INSTANCE.get_or_init(|| RdbcTraceManager)
    }
}
