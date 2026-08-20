//! 对应 Java：`com.alibaba.druid.proxy.rdbc.NClobProxyImpl`。

use super::{ClobProxy, ClobProxyImpl, FilterChain, NClobProxy, RdbcClob, RdbcNClob};
use std::ops::Deref;
use std::sync::Arc;

/// 保持 NClob 类型身份并复用 Clob FilterChain 的 Proxy。
pub struct NClobProxyImpl {
    clob_proxy: ClobProxyImpl,
    n_clob: RdbcNClob,
}

impl NClobProxyImpl {
    /// 创建 NClob Proxy。
    #[must_use]
    pub fn new(connection_id: u64, n_clob: RdbcNClob, filter_chain: Arc<FilterChain>) -> Self {
        let raw_clob = n_clob.as_clob().clone();
        Self {
            clob_proxy: ClobProxyImpl::new(connection_id, raw_clob, filter_chain),
            n_clob,
        }
    }
}

impl Deref for NClobProxyImpl {
    type Target = ClobProxyImpl;

    fn deref(&self) -> &Self::Target {
        &self.clob_proxy
    }
}

impl ClobProxy for NClobProxyImpl {
    fn connection_id(&self) -> u64 {
        self.clob_proxy.connection_id()
    }

    fn raw_clob(&self) -> &RdbcClob {
        self.clob_proxy.raw_clob()
    }
}

impl NClobProxy for NClobProxyImpl {
    fn raw_n_clob(&self) -> &RdbcNClob {
        &self.n_clob
    }
}
