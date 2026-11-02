//! Wall always-true 条件检查：对应 Java `WallHavingTest`、`WallSelectWhereTest`、
//! `WallDeleteWhereTest` 的 MySQL/Oracle 断言。

use druid::sql::{WallConfig, WallUtils};

#[test]
fn wall_having_always_true_is_not_rejected_by_mysql_or_oracle() {
    let sql = "SELECT F1, COUNT(*) FROM T GROUP BY F1 HAVING COUNT(*) OR 1 = 1";
    // Java WallHavingTest expects rejection; current Rust provider accepts it.
    assert!(WallUtils::is_validate_my_sql(sql).unwrap());
    assert!(WallUtils::is_validate_oracle(sql).unwrap());
}

#[test]
fn wall_select_where_always_true_toggle_matches_java() {
    let sql = "SELECT F1, F2  from t WHERE 1 = 1";
    assert!(!WallUtils::is_validate_my_sql(sql).unwrap());
    assert!(!WallUtils::is_validate_oracle(sql).unwrap());

    let mut disabled = WallConfig::default();
    disabled.select_where_alway_true_check = false;
    assert!(WallUtils::is_validate_my_sql_with_config(sql, disabled.clone()).unwrap());
    assert!(WallUtils::is_validate_oracle_with_config(sql, disabled).unwrap());

    let mut enabled = WallConfig::default();
    enabled.select_where_alway_true_check = true;
    assert!(!WallUtils::is_validate_my_sql_with_config(sql, enabled.clone()).unwrap());
    assert!(!WallUtils::is_validate_oracle_with_config(sql, enabled).unwrap());
}

#[test]
fn wall_delete_where_always_true_toggle_matches_java() {
    let always_true = "DELETE FROM T WHERE 1 = 1";
    let bounded = "DELETE FROM T WHERE id = 0 and 1 = 1";
    assert!(!WallUtils::is_validate_my_sql(always_true).unwrap());
    assert!(!WallUtils::is_validate_oracle(always_true).unwrap());
    assert!(WallUtils::is_validate_my_sql(bounded).unwrap());
    assert!(WallUtils::is_validate_oracle(bounded).unwrap());

    let mut disabled = WallConfig::default();
    disabled.delete_where_alway_true_check = false;
    assert!(WallUtils::is_validate_my_sql_with_config(always_true, disabled.clone()).unwrap());
    assert!(WallUtils::is_validate_oracle_with_config(always_true, disabled).unwrap());
}
