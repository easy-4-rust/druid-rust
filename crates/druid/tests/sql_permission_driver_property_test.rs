use druid::core::DriverProperty;
use druid::rdbc::SqlPermission;

// ── SqlPermission ──────────────────────────────────────────────

#[test]
fn sql_permission_new() {
    let p = SqlPermission::new("setLog", None);
    assert_eq!(p.name(), "setLog");
    assert!(p.actions().is_none());
}

#[test]
fn sql_permission_with_actions() {
    let p = SqlPermission::new("setLog", Some("write".to_owned()));
    assert_eq!(p.name(), "setLog");
    assert_eq!(p.actions(), Some("write"));
}

#[test]
fn sql_permission_clone_eq() {
    let p1 = SqlPermission::new("test", Some("a".to_owned()));
    let p2 = p1.clone();
    assert_eq!(p1, p2);
}

#[test]
fn sql_permission_debug() {
    let p = SqlPermission::new("test", None);
    let dbg = format!("{:?}", p);
    assert!(dbg.contains("SqlPermission"));
    assert!(dbg.contains("test"));
}

// ── DriverProperty ─────────────────────────────────────────────

#[test]
fn driver_property_new() {
    let p = DriverProperty::new("user", Some("admin".to_owned()));
    assert_eq!(p.name, "user");
    assert_eq!(p.value, Some("admin".to_owned()));
    assert!(p.description.is_none());
    assert!(!p.required);
    assert!(p.choices.is_empty());
}

#[test]
fn driver_property_new_none_value() {
    let p = DriverProperty::new("password", None);
    assert_eq!(p.name, "password");
    assert!(p.value.is_none());
}

#[test]
fn driver_property_clone_eq() {
    let p1 = DriverProperty::new("host", Some("localhost".to_owned()));
    let p2 = p1.clone();
    assert_eq!(p1, p2);
}

#[test]
fn driver_property_debug() {
    let p = DriverProperty::new("port", Some("5432".to_owned()));
    let dbg = format!("{:?}", p);
    assert!(dbg.contains("DriverProperty"));
    assert!(dbg.contains("port"));
}

#[test]
fn driver_property_custom_fields() {
    let mut p = DriverProperty::new("ssl", None);
    p.description = Some("Enable SSL".to_owned());
    p.required = true;
    p.choices = vec!["true".to_owned(), "false".to_owned()];
    assert_eq!(p.description, Some("Enable SSL".to_owned()));
    assert!(p.required);
    assert_eq!(p.choices.len(), 2);
}
