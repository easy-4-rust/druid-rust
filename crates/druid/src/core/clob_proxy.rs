//! 对应 Java：`com.alibaba.druid.proxy.rdbc.ClobProxy`。

use super::RdbcClob;

/// Druid 对 RDBC Clob 的代理身份。
///
/// 与平台 `RdbcClob` 不同，本协议额外保存创建它的 Druid 连接身份，使 Filter
/// 能区分同一物理 LOB 经不同逻辑连接访问的事件。
pub trait ClobProxy: Send + Sync {
    /// 返回创建该 Proxy 的 Druid 连接 ID。
    fn connection_id(&self) -> u64;

    /// 返回原始 RDBC Clob 句柄。
    fn raw_clob(&self) -> &RdbcClob;
}
