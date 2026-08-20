//! Druid 池化对象 Wrapper。

use super::wrapper::{Unwrapped, Wrapper};
use std::any::{Any, TypeId};

/// 为池化连接、语句和结果集保留底层对象解包语义。
///
/// 对应 Java: `com.alibaba.druid.pool.PoolableWrapper`。对象依次处理空
/// wrapper、空类型令牌、`DruidStatementConnection` 的底层连接、被包装对象、
/// 当前 `PoolableWrapper`、普通对象直接实例判断，最后委托被包装对象。
pub struct PoolableWrapper {
    wrapper: Option<Box<dyn Wrapper>>,
}

impl std::fmt::Debug for PoolableWrapper {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PoolableWrapper")
            .field("has_wrapper", &self.wrapper.is_some())
            .finish()
    }
}

impl PoolableWrapper {
    /// 包装一个非空底层对象。
    ///
    /// 参数 `wrapper` 对应 Java 构造器参数 `wraaper`（原源码拼写）；
    /// 返回拥有该对象的池化 Wrapper。
    pub fn new(wrapper: impl Wrapper + 'static) -> Self {
        Self {
            wrapper: Some(Box::new(wrapper)),
        }
    }

    /// 使用可空底层对象创建 Wrapper。
    ///
    /// 参数 `wrapper` 为 `None` 时精确保留 Java 构造器接收 `null` 后的行为。
    pub fn from_optional(wrapper: Option<Box<dyn Wrapper>>) -> Self {
        Self { wrapper }
    }

    /// 返回被包装对象；底层为空时返回 `None`。
    pub fn wrapped(&self) -> Option<&dyn Wrapper> {
        self.wrapper.as_deref()
    }
}

impl Wrapper for PoolableWrapper {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_wrapper_for(&self, iface: Option<TypeId>) -> bool {
        let (Some(wrapper), Some(iface)) = (self.wrapper.as_deref(), iface) else {
            return false;
        };

        // Java 优先把 DruidStatementConnection 暴露为其持有的物理连接。
        if wrapper
            .statement_connection()
            .is_some_and(|connection| connection.type_id() == iface)
        {
            return true;
        }

        if wrapper.as_any().type_id() == iface || self.as_any().type_id() == iface {
            return true;
        }

        // WrapperProxy 必须走自身 FilterChain；普通 Wrapper 可直接暴露实例。
        if !wrapper.is_wrapper_proxy() && wrapper.is_instance_of(iface) {
            return true;
        }

        wrapper.is_wrapper_for(Some(iface))
    }

    fn unwrap(&self, iface: Option<TypeId>) -> Option<Unwrapped<'_>> {
        let (Some(wrapper), Some(iface)) = (self.wrapper.as_deref(), iface) else {
            return None;
        };

        if let Some(connection) = wrapper.statement_connection() {
            if connection.type_id() == iface {
                return Some(Unwrapped::Object(connection));
            }
        }

        if wrapper.as_any().type_id() == iface {
            return Some(Unwrapped::Object(wrapper.as_any()));
        }

        if self.as_any().type_id() == iface {
            return Some(Unwrapped::Object(self.as_any()));
        }

        if !wrapper.is_wrapper_proxy() && wrapper.is_instance_of(iface) {
            return Some(Unwrapped::Object(wrapper.as_any()));
        }

        wrapper.unwrap(Some(iface))
    }
}
