use std::fmt;
use std::ops::{Deref, DerefMut};

/// 已由当前连接资源注册表接管的 LOB 创建结果。
/// 对应 Java: FilterChainImpl#createBlob/createClob/createNClob 的受管返回值。
pub struct RegisteredLob<T: ?Sized> {
    value: Box<T>,
}

impl<T: ?Sized> RegisteredLob<T> {
    pub(crate) fn new(value: Box<T>) -> Self {
        Self { value }
    }

    /// 将已登记的结果交给调用者；连接关闭仍由注册表监管该资源。
    pub(crate) fn into_inner(self) -> Box<T> {
        self.value
    }
}

impl<T: ?Sized> fmt::Debug for RegisteredLob<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegisteredLob")
            .finish_non_exhaustive()
    }
}

impl<T: ?Sized> Deref for RegisteredLob<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: ?Sized> DerefMut for RegisteredLob<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
