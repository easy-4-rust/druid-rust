//! 默认对象读取的自定义对象身份探针。
use druid_rdbc::types::OpaqueObjectValue;
use std::any::Any;

/// 不可被字符串化替代的真实自定义对象。
/// 对应 Java: driver vendor 自定义 Object。
#[derive(Debug)]
pub(super) struct ObjectPayload(pub(super) String);

impl OpaqueObjectValue for ObjectPayload {
    fn class_name(&self) -> &str {
        "example.ObjectPayload"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
