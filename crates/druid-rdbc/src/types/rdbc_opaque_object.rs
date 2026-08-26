//! RDBC vendor/custom 对象的不透明平台句柄。
//!
//! 对应 Java：`ResultSet#getObject` 与 `ResultSet#updateObject` 可传递的任意
//! driver 对象。Rust 不使用 JVM `Object`，因此以共享句柄保留具体对象身份、
//! Java 类名和受控 downcast，而不把未知对象压缩为字符串或字节。

use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// Adapter 提供的不透明 RDBC 自定义对象值。
pub trait OpaqueObjectValue: fmt::Debug + Send + Sync {
    /// 返回 Java/driver 对象的稳定类名。
    fn class_name(&self) -> &str;

    /// 暴露受控运行时类型信息，供对应 Adapter downcast。
    fn as_any(&self) -> &dyn Any;
}

/// 不泄漏具体驱动依赖的自定义对象句柄。
#[derive(Clone)]
pub struct RdbcOpaqueObject {
    value: Arc<dyn OpaqueObjectValue>,
}

impl RdbcOpaqueObject {
    /// 包装一个 adapter 自定义对象。
    pub fn new(value: Arc<dyn OpaqueObjectValue>) -> Self {
        Self { value }
    }

    /// 返回 Java/driver 类名。
    pub fn class_name(&self) -> &str {
        self.value.class_name()
    }

    /// 尝试按 Rust Adapter 的具体类型读取对象。
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.value.as_any().downcast_ref::<T>()
    }

    /// 返回底层 adapter value。
    pub fn value(&self) -> &dyn OpaqueObjectValue {
        self.value.as_ref()
    }
}

impl fmt::Debug for RdbcOpaqueObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcOpaqueObject")
            .field("class_name", &self.class_name())
            .field("value", &self.value)
            .finish()
    }
}

impl PartialEq for RdbcOpaqueObject {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }
}

impl Eq for RdbcOpaqueObject {}
