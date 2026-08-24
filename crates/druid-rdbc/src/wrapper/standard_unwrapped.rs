use std::any::Any;

/// 标准 [`Any`] 对象的只读解包结果。
///
/// 对应 Java：`Wrapper#unwrap(Class)`。该对象不含产品代理或 transitional 接口变体。
#[derive(Clone, Copy)]
pub struct StandardUnwrapped<'a> {
    object: &'a dyn Any,
}

impl<'a> StandardUnwrapped<'a> {
    /// 从一个具体运行时对象创建标准解包视图。
    pub const fn new(object: &'a dyn Any) -> Self {
        Self { object }
    }

    /// 尝试将运行时对象下转为 `T`。
    pub fn downcast_ref<T: Any>(&self) -> Option<&'a T> {
        self.object.downcast_ref::<T>()
    }
}

impl std::fmt::Debug for StandardUnwrapped<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("StandardUnwrapped(..)")
    }
}
