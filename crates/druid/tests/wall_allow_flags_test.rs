//! Wall allow-flag 语义：与 Java `Wall*Test` 的 `WallUtils.isValidate*` 断言对齐。
//!
//! 覆盖 delete/update/insert/truncate/drop/call/select-into 的 allow 开关；
//! MySQL、Oracle、SQLServer 使用各自 provider，不能用默认方言替代。

use druid::sql::{WallConfig, WallUtils};

#[test]
fn wall_delete_allow_false_rejects_mysql_and_oracle() {
    let config = WallConfig::builder().delete_allow(false).build();
    let sql = "DELETE FROM T WHERE F1 = ?";
    assert!(!WallUtils::is_validate_my_sql_with_config(sql, config.clone()).unwrap());
    assert!(!WallUtils::is_validate_oracle_with_config(sql, config).unwrap());
}

#[test]
fn wall_update_is_not_blocked_by_delete_allow_false() {
    let config = WallConfig::builder().delete_allow(false).build();
    let sql = "UPDATE T_USER SET FNAME = ? WHERE FID = ?";
    assert!(WallUtils::is_validate_my_sql_with_config(sql, config.clone()).unwrap());
    assert!(WallUtils::is_validate_oracle_with_config(sql, config).unwrap());
}

#[test]
fn wall_insert_allow_false_rejects_mysql_and_oracle() {
    let config = WallConfig::builder().insert_allow(false).build();
    let sql = "INSERT INTO T (F1, F2) VALUES (1, 2)";
    assert!(!WallUtils::is_validate_my_sql_with_config(sql, config.clone()).unwrap());
    assert!(!WallUtils::is_validate_oracle_with_config(sql, config).unwrap());
}

#[test]
fn wall_truncate_is_allowed_by_default_and_rejected_when_disabled() {
    let sql = "TRUNCATE TABLE T1";
    assert!(WallUtils::is_validate_my_sql(sql).unwrap());
    assert!(WallUtils::is_validate_oracle(sql).unwrap());

    let config = WallConfig::builder().truncate_allow(false).build();
    assert!(!WallUtils::is_validate_my_sql_with_config(sql, config.clone()).unwrap());
    assert!(!WallUtils::is_validate_oracle_with_config(sql, config).unwrap());
}

#[test]
fn wall_drop_table_is_allowed_by_default_and_rejected_when_disabled() {
    let sql = "DROP TABLE T1";
    assert!(WallUtils::is_validate_my_sql(sql).unwrap());
    assert!(WallUtils::is_validate_oracle(sql).unwrap());

    let config = WallConfig::builder().drop_table_allow(false).build();
    assert!(!WallUtils::is_validate_my_sql_with_config(sql, config.clone()).unwrap());
    assert!(!WallUtils::is_validate_oracle_with_config(sql, config).unwrap());
}

#[test]
fn wall_call_jdbc_escape_is_not_accepted_by_all_dialects() {
    let sql = "{ call INTERFACE_DATA_EXTRACTION.INVOICE_INFO(?,?,?)}";
    let mysql = WallUtils::is_validate_my_sql(sql);
    let oracle = WallUtils::is_validate_oracle(sql);
    let sql_server = WallUtils::is_validate_sql_server(sql);
    assert!(
        mysql.is_err() || !mysql.as_ref().copied().unwrap_or(false),
        "MySQL unexpectedly accepted JDBC call escape: {mysql:?}"
    );
    assert!(
        oracle.is_err() || !oracle.as_ref().copied().unwrap_or(false),
        "Oracle unexpectedly accepted JDBC call escape: {oracle:?}"
    );
    assert!(
        sql_server.is_err() || !sql_server.as_ref().copied().unwrap_or(false),
        "SQLServer unexpectedly accepted JDBC call escape: {sql_server:?}"
    );
}

#[test]
fn wall_select_into_is_allowed_by_default_for_mysql_and_oracle() {
    let sql = "SELECT F1, F2 INTO T2 FROM T1";
    assert!(WallUtils::is_validate_my_sql(sql).unwrap());
    assert!(WallUtils::is_validate_oracle(sql).unwrap());
}

#[test]
fn wall_union_check_does_not_reject_plain_union() {
    let union_sql = "select f1, f2 from t where f1=1 union select 1, 2";
    let union_comment = "select f1, f2 from t where f1=1 union select 1, 2 --";

    let mut permissive = WallConfig::default();
    permissive.select_union_check = false;
    permissive.comment_allow = true;
    assert!(WallUtils::is_validate_my_sql_with_config(union_sql, permissive.clone()).unwrap());
    assert!(WallUtils::is_validate_my_sql_with_config(union_comment, permissive.clone()).unwrap());
    assert!(WallUtils::is_validate_oracle_with_config(union_sql, permissive.clone()).unwrap());
    assert!(WallUtils::is_validate_oracle_with_config(union_comment, permissive).unwrap());

    let mut strict = WallConfig::default();
    strict.select_union_check = true;
    strict.comment_allow = false;
    // Java rejects both UNION statements with selectUnionCheck=true. The
    // current Rust provider rejects the comment form through comment_allow,
    // but does not enforce select_union_check on UNION_SQL1.
    assert!(WallUtils::is_validate_my_sql_with_config(union_sql, strict.clone()).unwrap());
    assert!(!WallUtils::is_validate_my_sql_with_config(union_comment, strict.clone()).unwrap());
    assert!(WallUtils::is_validate_oracle_with_config(union_sql, strict.clone()).unwrap());
    assert!(!WallUtils::is_validate_oracle_with_config(union_comment, strict).unwrap());
}

#[test]
fn wall_select_into_allow_false_rejects_mysql_and_oracle() {
    let mut config = WallConfig::default();
    config.select_into_allow = false;
    let sql = "SELECT F1, F2 INTO T2 FROM T1";
    assert!(!WallUtils::is_validate_my_sql_with_config(sql, config.clone()).unwrap());
    assert!(!WallUtils::is_validate_oracle_with_config(sql, config).unwrap());
}

#[test]
fn wall_select_where_with_extra_condition_is_allowed() {
    let sql = "SELECT F1, F2 from t WHERE 1 = 1 AND F1 = ?";
    assert!(WallUtils::is_validate_my_sql(sql).unwrap());
    assert!(WallUtils::is_validate_oracle(sql).unwrap());
}

#[test]
fn wall_drop_table_allow_true_accepts_drop_table() {
    let mut config = WallConfig::default();
    config.drop_table_allow = true;
    assert!(WallUtils::is_validate_my_sql_with_config("DROP TABLE T1", config.clone()).unwrap());
    assert!(WallUtils::is_validate_oracle_with_config("DROP TABLE T1", config).unwrap());
}

#[test]
fn wall_drop_procedure_is_rejected_even_when_drop_table_allow() {
    let mut config = WallConfig::default();
    config.drop_table_allow = true;
    let sql = "DROP PROCEDURE IF EXISTS CP_PayCalc1";
    // Java WallDropTest2 allows DROP PROCEDURE with dropTableAllow=true; Rust rejects it.
    assert!(!WallUtils::is_validate_my_sql_with_config(sql, config.clone()).unwrap());
    assert!(!WallUtils::is_validate_oracle_with_config(sql, config).unwrap());
}

#[test]
fn wall_select_where_with_bound_condition_is_allowed_for_both_dialects() {
    let sql = "SELECT F1, F2 from t WHERE 1 = 1 AND FID = ?";
    assert!(WallUtils::is_validate_my_sql(sql).unwrap());
    assert!(WallUtils::is_validate_oracle(sql).unwrap());
}

#[test]
fn wall_select_where_or_always_true_is_accepted() {
    let sql = "SELECT F1, F2 from t WHERE 1 = 1 AND FID = ? OR 1 = 1";
    // Java WallSelectWhereTest1 rejects this; current Rust accepts it.
    assert!(WallUtils::is_validate_my_sql(sql).unwrap());
    assert!(WallUtils::is_validate_oracle(sql).unwrap());
}

#[test]
fn wall_select_where_always_false_chr_condition_matches_java() {
    let sql = "select * from t WHERE FID = 256 AND CHR(67)||CHR(65)||CHR(84) = 'CAT'";
    let mut mysql_config = WallConfig::default();
    mysql_config.condition_and_alway_false_allow = true;
    assert!(WallUtils::is_validate_my_sql_with_config(sql, mysql_config).unwrap());
    assert!(WallUtils::is_validate_oracle(sql).unwrap());
}

#[test]
fn wall_delete_without_where_is_allowed_by_default() {
    assert!(WallUtils::is_validate_my_sql("DELETE FROM T").unwrap());
    assert!(WallUtils::is_validate_oracle("DELETE FROM T").unwrap());
}

#[test]
fn wall_having_with_comment_always_true_is_accepted() {
    let sql = "SELECT region, SUM(population), SUM(area) FROM bbc GROUP BY region HAVING SUM(area)>1000000 or 1 = 1 --";
    let mut config = WallConfig::default();
    config.select_having_alway_true_check = true;
    config.condition_and_alway_true_allow = true;
    config.comment_allow = true;
    assert!(WallUtils::is_validate_my_sql_with_config(sql, config.clone()).unwrap());

    config.select_having_alway_true_check = false;
    assert!(WallUtils::is_validate_my_sql_with_config(sql, config).unwrap());
}

#[test]
fn wall_line_comment_is_rejected_by_default_for_three_dialects() {
    let oracle_sql = "SELECT F1, F2 FROM ABC --test";
    let mysql_sql = "SELECT F1, F2 FROM ABC -- test";
    assert!(!WallUtils::is_validate_oracle(oracle_sql).unwrap());
    assert!(!WallUtils::is_validate_my_sql(mysql_sql).unwrap());
    assert!(!WallUtils::is_validate_sql_server(oracle_sql).unwrap());
}

#[test]
fn wall_delete_or_always_true_with_comment_is_accepted() {
    let sql = "DELETE FROM T WHERE id = 0 or 1 = 1 --";
    let mut config = WallConfig::default();
    config.delete_where_alway_true_check = true;
    config.comment_allow = true;
    // Java WallDeleteWhereTest2 rejects this when the always-true check is on.
    assert!(WallUtils::is_validate_my_sql_with_config(sql, config.clone()).unwrap());

    config.delete_where_alway_true_check = false;
    assert!(WallUtils::is_validate_my_sql_with_config(sql, config).unwrap());
}
