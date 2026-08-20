extern crate druid_core as druid;
use druid_core::sql::WallTableStatValue;

fn sample_value() -> WallTableStatValue {
    WallTableStatValue {
        name: "users".to_owned(),
        select_count: 10,
        select_into_count: 2,
        insert_count: 3,
        update_count: 4,
        delete_count: 5,
        truncate_count: 1,
        create_count: 1,
        alter_count: 99,
        drop_count: 1,
        replace_count: 2,
        delete_data_count: 100,
        update_data_count: 200,
        insert_data_count: 300,
        fetch_row_count: 500,
        fetch_row_histogram: [1, 2, 3, 4, 5, 6],
        update_data_histogram: [10, 20, 30, 40, 50, 60],
        delete_data_histogram: [7, 8, 9, 10, 11, 12],
    }
}

#[test]
fn wall_table_stat_value_total_execute_count() {
    let v = sample_value();
    // Java 不含 alterCount: 10+2+3+4+5+1+1+1+2 = 29
    assert_eq!(v.total_execute_count(), 29);
}

#[test]
fn wall_table_stat_value_total_execute_count_zero() {
    let v = WallTableStatValue {
        name: "empty".to_owned(),
        select_count: 0,
        select_into_count: 0,
        insert_count: 0,
        update_count: 0,
        delete_count: 0,
        truncate_count: 0,
        create_count: 0,
        alter_count: 0,
        drop_count: 0,
        replace_count: 0,
        delete_data_count: 0,
        update_data_count: 0,
        insert_data_count: 0,
        fetch_row_count: 0,
        fetch_row_histogram: [0; 6],
        update_data_histogram: [0; 6],
        delete_data_histogram: [0; 6],
    };
    assert_eq!(v.total_execute_count(), 0);
}

#[test]
fn wall_table_stat_value_to_map_keys() {
    let v = sample_value();
    let map = v.to_map();
    assert_eq!(map.get("name").unwrap().as_str().unwrap(), "users");
    assert_eq!(map.get("selectCount").unwrap().as_u64().unwrap(), 10);
    assert_eq!(map.get("selectIntoCount").unwrap().as_u64().unwrap(), 2);
    assert_eq!(map.get("insertCount").unwrap().as_u64().unwrap(), 3);
    assert_eq!(map.get("updateCount").unwrap().as_u64().unwrap(), 4);
    assert_eq!(map.get("deleteCount").unwrap().as_u64().unwrap(), 5);
    assert_eq!(map.get("truncateCount").unwrap().as_u64().unwrap(), 1);
    assert_eq!(map.get("createCount").unwrap().as_u64().unwrap(), 1);
    assert_eq!(map.get("alterCount").unwrap().as_u64().unwrap(), 99);
    assert_eq!(map.get("dropCount").unwrap().as_u64().unwrap(), 1);
    assert_eq!(map.get("replaceCount").unwrap().as_u64().unwrap(), 2);
    assert_eq!(map.get("deleteDataCount").unwrap().as_u64().unwrap(), 100);
    assert_eq!(map.get("updateDataCount").unwrap().as_u64().unwrap(), 200);
    assert_eq!(map.get("insertDataCount").unwrap().as_u64().unwrap(), 300);
    assert_eq!(map.get("fetchRowCount").unwrap().as_u64().unwrap(), 500);
    assert!(map.contains_key("fetchRowHistogram"));
    assert!(map.contains_key("updateDataHistogram"));
    assert!(map.contains_key("deleteDataHistogram"));
}

#[test]
fn wall_table_stat_value_clone_eq() {
    let v1 = sample_value();
    let v2 = v1.clone();
    assert_eq!(v1, v2);
}

#[test]
fn wall_table_stat_value_debug() {
    let v = sample_value();
    let dbg = format!("{:?}", v);
    assert!(dbg.contains("WallTableStatValue"));
    assert!(dbg.contains("users"));
}

#[test]
fn wall_table_stat_value_saturating_add() {
    let v = WallTableStatValue {
        name: "overflow".to_owned(),
        select_count: u64::MAX,
        select_into_count: 1,
        insert_count: 0,
        update_count: 0,
        delete_count: 0,
        truncate_count: 0,
        create_count: 0,
        alter_count: 0,
        drop_count: 0,
        replace_count: 0,
        delete_data_count: 0,
        update_data_count: 0,
        insert_data_count: 0,
        fetch_row_count: 0,
        fetch_row_histogram: [0; 6],
        update_data_histogram: [0; 6],
        delete_data_histogram: [0; 6],
    };
    assert_eq!(v.total_execute_count(), u64::MAX);
}
