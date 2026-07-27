//! AdminState and endpoint_list tests.
#[test]
fn test_admin_state_new() {
    let s = druid_admin::AdminState::new("main", "postgres");
    assert_eq!(s.pool_name, "main");
    assert_eq!(s.driver_name, "postgres");
}
#[test]
fn test_admin_state_clone() {
    let s = druid_admin::AdminState::new("test", "mysql");
    let s2 = s.clone();
    assert_eq!(s2.pool_name, "test");
    assert_eq!(s2.driver_name, "mysql");
}
#[test]
fn test_admin_state_debug() {
    let s = druid_admin::AdminState::new("x", "y");
    assert!(format!("{:?}", s).contains("x"));
}
#[test]
fn test_endpoint_list() {
    let list = druid_admin::endpoint_list();
    assert!(list.contains("/druid/api/datasources"));
    assert!(list.contains("/druid/api/sql/top"));
    assert!(list.contains("/druid/api/sql/slow"));
    assert!(list.contains("/druid/api/wall"));
    assert!(list.contains("/druid/api/active"));
    assert!(list.contains("/metrics"));
}
