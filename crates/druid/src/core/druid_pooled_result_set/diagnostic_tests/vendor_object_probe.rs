/// 验证厂商对象身份的原 BASE 对象。对应 Java: java.sql.ResultSet#updateObject。
#[derive(Debug, PartialEq)]
pub(super) struct VendorObjectProbe {
    pub(super) id: i32,
}
impl druid_rdbc::types::OpaqueObjectValue for VendorObjectProbe {
    fn class_name(&self) -> &str {
        "com.example.VendorObject"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
