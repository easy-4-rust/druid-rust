extern crate druid_core as druid;
use druid_core::core::CallableOutParameter;

#[test]
fn callable_out_parameter_new() {
    let p = CallableOutParameter::new(4);
    assert_eq!(p.sql_type(), 4);
    assert!(p.scale().is_none());
    assert!(p.type_name().is_none());
}

#[test]
fn callable_out_parameter_with_scale() {
    let p = CallableOutParameter::with_scale(3, 10);
    assert_eq!(p.sql_type(), 3);
    assert_eq!(p.scale(), Some(10));
    assert!(p.type_name().is_none());
}

#[test]
fn callable_out_parameter_with_type_name() {
    let p = CallableOutParameter::with_type_name(12, "VARCHAR");
    assert_eq!(p.sql_type(), 12);
    assert!(p.scale().is_none());
    assert_eq!(p.type_name(), Some("VARCHAR"));
}

#[test]
fn callable_out_parameter_clone_eq() {
    let p1 = CallableOutParameter::with_scale(3, 5);
    let p2 = p1.clone();
    assert_eq!(p1, p2);
}

#[test]
fn callable_out_parameter_debug() {
    let p = CallableOutParameter::new(4);
    let dbg = format!("{:?}", p);
    assert!(dbg.contains("CallableOutParameter"));
}
