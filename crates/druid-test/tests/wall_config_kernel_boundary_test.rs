//! ADR-CRATE-003：内核配置必须通过原门面进入真实 Wall 执行路径。
//! 防止下沉时生成两种 WallConfig、丢失嵌入资源或复制回调状态。
use druid::sql::{DbType, Wall, WallViolation};
use druid_rdbc::governance::wall::{
    TenantCallBack, TenantStatementType, WallConfig, WallUpdateCheckHandler,
};
use druid_rdbc::types::Value;
use std::sync::{Arc, Mutex};

struct Tenant(Mutex<Vec<Value>>);
impl TenantCallBack for Tenant {
    fn tenant_value(&self, kind: TenantStatementType, table: &str) -> Option<Value> {
        (kind == TenantStatementType::Select && table == "orders").then_some(Value::Int(7))
    }
    fn tenant_column(&self, _: TenantStatementType, table: &str) -> Option<String> {
        (table == "orders").then(|| "tenant_id".to_owned())
    }
    fn hidden_column(&self, table: &str) -> Option<String> {
        (table == "orders").then(|| "tenant_id".to_owned())
    }
    fn filter_resultset_tenant_column(&self, value: &Value) {
        self.0.lock().unwrap().push(value.clone());
    }
}
struct Check;
impl WallUpdateCheckHandler for Check {
    fn check(&self, table: &str, column: &str, value: &Value, old: &[Value]) -> bool {
        table == "orders" && column == "status" && value == &Value::Int(2) && old == [Value::Int(1)]
    }
}

#[test]
fn canonical_config_drives_existing_wall_and_old_module_paths() {
    let kernel = WallConfig::builder().delete_allow(false).build();
    let facade: druid::sql::wall_config::WallConfig = kernel;
    let kernel: WallConfig = facade;
    let wall = Wall::with_db_type(kernel, DbType::MySql);
    assert_eq!(
        wall.check("DELETE FROM orders WHERE id = 1"),
        Err(vec![WallViolation::OperationNotAllowed(
            "DELETE".to_owned()
        )])
    );
    assert_eq!(wall.check("SELECT id FROM orders"), Ok(()));
}

#[test]
fn callback_and_handler_identity_survive_facade_round_trip() {
    let tenant = Arc::new(Tenant(Mutex::new(Vec::new())));
    let callback: Arc<dyn TenantCallBack> = tenant.clone();
    let old_callback: Arc<dyn druid::sql::tenant_call_back::TenantCallBack> = callback.clone();
    let check: Arc<dyn WallUpdateCheckHandler> = Arc::new(Check);
    let old_check: Arc<dyn druid::sql::wall_update_check_handler::WallUpdateCheckHandler> =
        check.clone();
    let config = WallConfig::builder()
        .tenant_call_back(old_callback)
        .update_check_handler(old_check)
        .update_check_column("Orders.Status")
        .build();
    let facade: druid::sql::WallConfig = config.clone();
    assert!(Arc::ptr_eq(&callback, &facade.tenant_call_back().unwrap()));
    assert!(Arc::ptr_eq(&check, &facade.update_check_handler().unwrap()));
    let callback = facade.tenant_call_back().unwrap();
    assert_eq!(
        callback.tenant_value(TenantStatementType::Select, "orders"),
        Some(Value::Int(7))
    );
    assert_eq!(
        callback.tenant_value(TenantStatementType::Delete, "orders"),
        None
    );
    assert_eq!(
        callback.tenant_column(TenantStatementType::Select, "orders"),
        Some("tenant_id".into())
    );
    assert_eq!(callback.hidden_column("orders"), Some("tenant_id".into()));
    callback.filter_resultset_tenant_column(&Value::Int(7));
    assert_eq!(*tenant.0.lock().unwrap(), vec![Value::Int(7)]);
    assert!(facade.update_check_handler().unwrap().check(
        "orders",
        "status",
        &Value::Int(2),
        &[Value::Int(1)]
    ));
    assert!(!facade.update_check_handler().unwrap().check(
        "orders",
        "status",
        &Value::Int(3),
        &[Value::Int(1)]
    ));
    facade.set_tenant_call_back(None);
    facade.set_update_check_handler(None);
    assert!(config.tenant_call_back().is_none());
    assert!(config.update_check_handler().is_none());
}

#[test]
fn ordered_update_columns_and_clone_visibility_are_preserved() {
    let config = WallConfig::default();
    config.add_update_check_columns("Orders.Status");
    let facade: druid::sql::WallConfig = config.clone();
    facade.add_update_check_columns("orders.Version");
    facade.add_update_check_columns("orders.status");
    for invalid in ["orders", "schema.orders.status"] {
        facade.add_update_check_columns(invalid);
    }
    assert_eq!(
        config
            .update_check_table("ORDERS")
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>(),
        vec!["status".to_owned(), "version".to_owned()]
    );
    assert!(!config.is_update_check_table("missing"));
    assert_eq!(config.update_check_table("missing"), None);
}

#[test]
fn kernel_owns_bundled_rules_and_keeps_append_semantics() {
    let mut config = WallConfig::with_config_dir("/META-INF/druid/wall/mysql/");
    assert!(config.inited);
    assert_eq!(config.dir.as_deref(), Some("/META-INF/druid/wall/mysql"));
    assert!(config.deny_functions.contains(&"sleep".to_owned()));
    let first = config.deny_functions.clone();
    config.load_config("/META-INF/druid/wall/mysql/");
    assert_eq!(config.deny_functions, first);
    config.deny_functions.push("custom_blocked".into());
    config.load_config("META-INF/druid/wall/mysql");
    assert!(config.deny_functions.contains(&"custom_blocked".to_owned()));
    let expected = config.deny_functions.clone();
    config.load_config("missing");
    assert_eq!(config.deny_functions, expected);
    assert!(config.inited);
    let facade: druid::sql::WallConfig = config;
    let violations = Wall::with_db_type(facade, DbType::MySql)
        .check("SELECT sleep(1)")
        .unwrap_err();
    assert!(violations.contains(&WallViolation::DeniedFunction("sleep".into())));
}
