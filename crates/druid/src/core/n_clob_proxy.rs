//! 对应 Java：`com.alibaba.druid.proxy.jdbc.NClobProxy`。

use super::{ClobProxy, JdbcNClob};

/// Druid 对 JDBC NClob 的代理身份。
pub trait NClobProxy: ClobProxy {
    /// 返回原始 JDBC NClob 句柄。
    fn raw_n_clob(&self) -> &JdbcNClob;
}
