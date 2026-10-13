use std::sync::Arc;

/// 可由同一真实资源管理器的多个 XA 资源共享的身份令牌。
///
/// 令牌只在显式克隆后表示同一资源管理器；诊断名称、连接 URL 或资源对象地址不能替代它。
#[derive(Clone, Debug)]
pub struct XaResourceManagerIdentity(Arc<()>);

impl XaResourceManagerIdentity {
    /// 创建新的资源管理器身份。
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(()))
    }
}

impl Default for XaResourceManagerIdentity {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for XaResourceManagerIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for XaResourceManagerIdentity {}
