use druid::stats::{RdbcStatContext, RdbcStatManager};

#[test]
fn stat_manager_global_singleton() {
    let m1 = RdbcStatManager::global();
    let m2 = RdbcStatManager::global();
    assert!(std::ptr::eq(m1, m2));
}

#[test]
fn stat_manager_generate_sql_id() {
    let m = RdbcStatManager::global();
    let id1 = m.generate_sql_id();
    let id2 = m.generate_sql_id();
    assert_ne!(id1, id2);
}

#[test]
fn stat_manager_connection_stat() {
    let m = RdbcStatManager::global();
    let _ = m.connection_stat();
}

#[test]
fn stat_manager_statement_stat() {
    let m = RdbcStatManager::global();
    let _ = m.statement_stat();
}

#[test]
fn stat_manager_result_set_stat() {
    let m = RdbcStatManager::global();
    let _ = m.result_set_stat();
}

#[test]
fn stat_manager_stat_context_none() {
    let m = RdbcStatManager::global();
    // 默认无线程上下文。
    assert!(m.stat_context().is_none());
}

#[test]
fn stat_manager_set_stat_context() {
    let m = RdbcStatManager::global();
    let mut ctx = RdbcStatContext::new();
    ctx.set_name(Some("test".to_owned()));
    m.set_stat_context(Some(ctx.clone()));
    let retrieved = m.stat_context();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().name(), Some("test"));
    m.set_stat_context(None);
    assert!(m.stat_context().is_none());
}

#[test]
fn stat_manager_create_stat_context() {
    let m = RdbcStatManager::global();
    let ctx = m.create_stat_context();
    assert!(ctx.name().is_none());
}

#[test]
fn stat_manager_reset() {
    let m = RdbcStatManager::global();
    let count_before = m.reset_count();
    m.reset();
    assert!(m.reset_count() > count_before);
}
