//! 对应 Java：`com.alibaba.druid.proxy.rdbc.NClobProxy`。

use super::{ClobProxy, RdbcNClob};

/// Druid 对 RDBC NClob 的代理身份。
pub trait NClobProxy: ClobProxy {
    /// 返回原始 RDBC NClob 句柄。
    fn raw_n_clob(&self) -> &RdbcNClob;
}
