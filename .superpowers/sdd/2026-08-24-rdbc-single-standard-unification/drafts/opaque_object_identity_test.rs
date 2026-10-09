//! 完整原厂商对象身份案例；当前全仓输入保持不变，终态后接入。
/// 验证原自定义对象类型、引用身份、Debug、全部显示和 SQL NULL 合同。
/// 对应 Java: java.sql.ResultSet#getObject 的厂商对象及平台标量。
#[test]
fn task6_rdbc_object_and_opaque_object_preserve_type_identity_and_display_contract() {
    use druid_rdbc::types::{RdbcObject, RdbcOpaqueObject, Value};
    use vendor_object_probe::VendorObjectProbe;

    let physical = Arc::new(VendorObjectProbe { id: 99 });
    let opaque = RdbcOpaqueObject::new(physical.clone());
    let same = opaque.clone();
    let other = RdbcOpaqueObject::new(Arc::new(VendorObjectProbe { id: 99 }));

    assert_eq!(opaque.class_name(), "com.example.VendorObject");
    assert_eq!(
        opaque.downcast_ref::<VendorObjectProbe>(),
        Some(&VendorObjectProbe { id: 99 })
    );
    assert!(opaque.downcast_ref::<String>().is_none());
    assert_eq!(opaque.value().class_name(), "com.example.VendorObject");
    assert!(format!("{opaque:?}").contains("com.example.VendorObject"));
    assert_eq!(opaque, same);
    assert_ne!(opaque, other);

    let values = [
        (RdbcObject::String("text".to_string()), "text"),
        (RdbcObject::Boolean(true), "true"),
        (RdbcObject::Byte(-1), "-1"),
        (RdbcObject::Short(-2), "-2"),
        (RdbcObject::Integer(-3), "-3"),
        (RdbcObject::Long(-4), "-4"),
        (RdbcObject::Float(1.25), "1.25"),
        (RdbcObject::Double(2.5), "2.5"),
        (RdbcObject::Bytes(vec![0, 1, 2]), "<3 bytes>"),
        (RdbcObject::Custom(opaque), "<com.example.VendorObject>"),
    ];
    for (value, expected) in values {
        assert_eq!(value.to_string(), expected);
        assert!(!value.is_null());
    }
    assert!(RdbcObject::from(Value::Null).is_null());
}