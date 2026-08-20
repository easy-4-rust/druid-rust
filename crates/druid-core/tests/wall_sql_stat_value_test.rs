extern crate druid_core as druid;
use druid::sql::WallSqlStatValue;

fn sample_value() -> WallSqlStatValue {
    WallSqlStatValue {
        sql: "SELECT 1".to_owned(),
        sql_hash: 12345,
        sql_sample: "SELECT 1".to_owned(),
        sql_sample_hash: 12345,
        execute_count: 10,
        execute_error_count: 2,
        fetch_row_count: 50,
        update_count: 5,
        syntax_error: false,
        violation_message: Some("test violation".to_owned()),
    }
}

#[test]
fn wall_sql_stat_value_default() {
    let v = WallSqlStatValue::default();
    assert!(v.sql.is_empty());
    assert_eq!(v.execute_count, 0);
    assert!(!v.syntax_error);
    assert!(v.violation_message.is_none());
}

#[test]
fn wall_sql_stat_value_to_map_full() {
    let v = sample_value();
    let map = v.to_map();
    assert_eq!(map.get("sql").unwrap().as_str().unwrap(), "SELECT 1");
    assert_eq!(map.get("executeCount").unwrap().as_u64().unwrap(), 10);
    assert_eq!(map.get("executeErrorCount").unwrap().as_u64().unwrap(), 2);
    assert_eq!(map.get("fetchRowCount").unwrap().as_u64().unwrap(), 50);
    assert_eq!(map.get("updateCount").unwrap().as_u64().unwrap(), 5);
    assert_eq!(
        map.get("violationMessage").unwrap().as_str().unwrap(),
        "test violation"
    );
}

#[test]
fn wall_sql_stat_value_to_map_sparse() {
    let v = WallSqlStatValue {
        sql: "SELECT 1".to_owned(),
        execute_count: 1,
        ..Default::default()
    };
    let map = v.to_map();
    assert_eq!(map.get("executeCount").unwrap().as_u64().unwrap(), 1);
    assert!(!map.contains_key("executeErrorCount"));
    assert!(!map.contains_key("fetchRowCount"));
    assert!(!map.contains_key("updateCount"));
    assert!(!map.contains_key("violationMessage"));
}

#[test]
fn wall_sql_stat_value_to_map_sample_differ() {
    let v = WallSqlStatValue {
        sql: "SELECT * FROM t WHERE id = ?".to_owned(),
        sql_sample: "SELECT * FROM t WHERE id = 1".to_owned(),
        execute_count: 1,
        ..Default::default()
    };
    let map = v.to_map();
    assert_eq!(
        map.get("sample").unwrap().as_str().unwrap(),
        "SELECT * FROM t WHERE id = 1"
    );
}

#[test]
fn wall_sql_stat_value_to_map_sample_same() {
    let v = WallSqlStatValue {
        sql: "SELECT 1".to_owned(),
        sql_sample: "SELECT 1".to_owned(),
        execute_count: 1,
        ..Default::default()
    };
    let map = v.to_map();
    assert!(!map.contains_key("sample"));
}

#[test]
fn wall_sql_stat_value_clone_eq() {
    let v1 = sample_value();
    let v2 = v1.clone();
    assert_eq!(v1, v2);
}

#[test]
fn wall_sql_stat_value_serialize() {
    let v = WallSqlStatValue {
        sql: "SELECT 1".to_owned(),
        execute_count: 5,
        syntax_error: true,
        ..Default::default()
    };
    let json = serde_json::to_string(&v).unwrap();
    assert!(json.contains("SELECT 1"));
    assert!(json.contains("executeCount"));
    assert!(json.contains("syntaxError"));
}
