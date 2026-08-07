//! RDBC vendor/custom 对象的不透明平台句柄。
//!
//! 对应 Java：`ResultSet#getObject` 与 `ResultSet#updateObject` 可传递的任意
//! driver 对象。Rust 不使用 JVM `Object`，因此以共享句柄保留具体对象身份、
//! Java 类名和受控 downcast，而不把未知对象压缩为字符串或字节。

use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// 物理 RDBC 自定义对象 SPI。
pub trait PhysicalRdbcOpaqueObject: fmt::Debug + Send + Sync {
    /// 返回 Java/driver 对象的稳定类名。
    fn class_name(&self) -> &str;

    /// 暴露受控运行时类型信息，供对应 Adapter downcast。
    fn as_any(&self) -> &dyn Any;
}

/// 不泄漏具体驱动依赖的自定义对象句柄。
#[derive(Clone)]
pub struct RdbcOpaqueObject {
    physical: Arc<dyn PhysicalRdbcOpaqueObject>,
}

impl RdbcOpaqueObject {
    /// 包装一个物理自定义对象。
    pub fn new(physical: Arc<dyn PhysicalRdbcOpaqueObject>) -> Self {
        Self { physical }
    }

    /// 返回 Java/driver 类名。
    pub fn class_name(&self) -> &str {
        self.physical.class_name()
    }

    /// 尝试按 Rust Adapter 的具体类型读取对象。
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.physical.as_any().downcast_ref::<T>()
    }

    /// 返回物理自定义对象 SPI。
    pub fn physical(&self) -> &dyn PhysicalRdbcOpaqueObject {
        self.physical.as_ref()
    }
}

impl fmt::Debug for RdbcOpaqueObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcOpaqueObject")
            .field("class_name", &self.class_name())
            .field("physical", &self.physical)
            .finish()
    }
}

impl PartialEq for RdbcOpaqueObject {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for RdbcOpaqueObject {}
