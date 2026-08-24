use crate::wrapper::StandardUnwrapped;
use std::any::{Any, TypeId};

/// JDBC `Wrapper` 的 object-safe Rust 标准。
///
/// 泛型便捷方法位于 [`WrapperExt`]，避免破坏 dyn compatibility。
pub trait Wrapper: Any + Send {
    /// 返回具体运行时对象视图。
    fn as_any(&self) -> &dyn Any;

    /// 判断对象能否解包为给定运行时类型。
    fn is_wrapper_for(&self, target: TypeId) -> bool {
        self.as_any().type_id() == target
    }

    /// 返回 object-safe 的标准解包视图。
    fn unwrap(&self, target: TypeId) -> Option<StandardUnwrapped<'_>> {
        self.is_wrapper_for(target)
            .then(|| StandardUnwrapped::new(self.as_any()))
    }
}

/// [`Wrapper`] 的强类型调用扩展。
pub trait WrapperExt: Wrapper {
    /// 判断对象能否解包为 `T`。
    fn is_wrapper_for_type<T: Any>(&self) -> bool {
        self.is_wrapper_for(TypeId::of::<T>())
    }

    /// 尝试把对象解包为 `T` 的只读引用。
    fn unwrap_ref<T: Any>(&self) -> Option<&T> {
        self.unwrap(TypeId::of::<T>())?.downcast_ref::<T>()
    }
}

impl<T> WrapperExt for T where T: Wrapper + ?Sized {}
