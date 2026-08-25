//! Druid pool/filter Wrapper proxy hooks。

use std::any::Any;

/// 只在 Druid logical proxy 内使用的 wrapper hook 集合。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct WrapperProxyHooks {
    wrapper_proxy: bool,
}

#[allow(dead_code)]
impl WrapperProxyHooks {
    pub(crate) const fn proxy() -> Self {
        Self {
            wrapper_proxy: true,
        }
    }

    pub(crate) const fn is_wrapper_proxy(self) -> bool {
        self.wrapper_proxy
    }

    pub(crate) fn statement_connection(connection: &dyn Any) -> &dyn Any {
        connection
    }
}
