//! Wall 注释拒绝语义：精确复刻 Java `WallLineCommentTest` 与
//! `WallMultiLineCommentTest` 的原始 SQL 与 Oracle/MySQL 断言。

use druid::sql::WallUtils;

#[test]
fn wall_line_comment_deny_exact_java_sql() {
    let sql = "select f1 from t -- ";
    assert!(!WallUtils::is_validate_oracle(sql).unwrap());
    assert!(!WallUtils::is_validate_my_sql(sql).unwrap());
}

#[test]
fn wall_multi_line_comment_deny_exact_java_sql() {
    let sql = "select f1 from t where a=1 /* and b=1 */";
    assert!(!WallUtils::is_validate_oracle(sql).unwrap());
    assert!(!WallUtils::is_validate_my_sql(sql).unwrap());
}
