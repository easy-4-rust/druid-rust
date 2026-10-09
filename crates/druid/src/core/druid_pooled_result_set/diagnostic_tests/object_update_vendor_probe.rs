use druid_rdbc::types::OpaqueObjectValue;
use std::any::Any;

/// 保留原 updateObject 的具体对象身份与 Java 类名。
/// 对应 Java: java.sql.ResultSet#updateObject 的 vendor Object。
#[derive(Debug)]
pub(super) struct FilterVendorObjectProbe {
    pub(super) id: u64,
}

impl OpaqueObjectValue for FilterVendorObjectProbe {
    fn class_name(&self) -> &str {
        "example.FilterVendorObject"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
