use druid::sql::{WallFunctionStat, WallSqlFunctionStat};

#[test]
fn wall_function_stat_default() {
    let stat = WallFunctionStat::default();
    let val = stat.stat_value("test".to_owned(), false);
    assert_eq!(val.invoke_count, 0);
    assert_eq!(val.name, "test");
}

#[test]
fn wall_function_stat_increment() {
    let stat = WallFunctionStat::default();
    stat.increment_invoke_count();
    stat.increment_invoke_count();
    let val = stat.stat_value("fn".to_owned(), false);
    assert_eq!(val.invoke_count, 2);
}

#[test]
fn wall_function_stat_reset() {
    let stat = WallFunctionStat::default();
    stat.increment_invoke_count();
    stat.increment_invoke_count();
    let val = stat.stat_value("fn".to_owned(), true);
    assert_eq!(val.invoke_count, 2);
    let val2 = stat.stat_value("fn".to_owned(), false);
    assert_eq!(val2.invoke_count, 0);
}

#[test]
fn wall_function_stat_add_sql_function_stat() {
    let stat = WallFunctionStat::default();
    let sql_stat = WallSqlFunctionStat { invoke_count: 5 };
    stat.add_sql_function_stat(sql_stat);
    let val = stat.stat_value("fn".to_owned(), false);
    assert_eq!(val.invoke_count, 5);
}

#[test]
fn wall_sql_function_stat_default() {
    let stat = WallSqlFunctionStat::default();
    assert_eq!(stat.invoke_count, 0);
}

#[test]
fn wall_sql_function_stat_increment() {
    let mut stat = WallSqlFunctionStat::default();
    stat.increment_invoke_count();
    stat.increment_invoke_count();
    stat.increment_invoke_count();
    assert_eq!(stat.invoke_count, 3);
}

#[test]
fn wall_sql_function_stat_clone_copy() {
    let stat = WallSqlFunctionStat { invoke_count: 10 };
    let stat2 = stat;
    assert_eq!(stat, stat2);
}

#[test]
fn wall_sql_function_stat_debug() {
    let stat = WallSqlFunctionStat { invoke_count: 7 };
    let dbg = format!("{:?}", stat);
    assert!(dbg.contains("WallSqlFunctionStat"));
    assert!(dbg.contains("7"));
}
