use druid::stats::RdbcStatContext;

#[test]
fn stat_context_new_default() {
    let ctx = RdbcStatContext::new();
    assert!(!ctx.is_trace_enable());
    assert!(ctx.request_id().is_none());
    assert!(ctx.name().is_none());
    assert!(ctx.file().is_none());
    assert!(ctx.sql().is_none());
}

#[test]
fn stat_context_trace_enable() {
    let mut ctx = RdbcStatContext::new();
    assert!(!ctx.is_trace_enable());
    ctx.set_trace_enable(true);
    assert!(ctx.is_trace_enable());
    ctx.set_trace_enable(false);
    assert!(!ctx.is_trace_enable());
}

#[test]
fn stat_context_request_id() {
    let mut ctx = RdbcStatContext::new();
    assert!(ctx.request_id().is_none());
    ctx.set_request_id(Some("req-123".to_owned()));
    assert_eq!(ctx.request_id(), Some("req-123"));
    ctx.set_request_id(None);
    assert!(ctx.request_id().is_none());
}

#[test]
fn stat_context_name() {
    let mut ctx = RdbcStatContext::new();
    assert!(ctx.name().is_none());
    ctx.set_name(Some("query-1".to_owned()));
    assert_eq!(ctx.name(), Some("query-1"));
    ctx.set_name(None);
    assert!(ctx.name().is_none());
}

#[test]
fn stat_context_file() {
    let mut ctx = RdbcStatContext::new();
    assert!(ctx.file().is_none());
    ctx.set_file(Some("migration.sql".to_owned()));
    assert_eq!(ctx.file(), Some("migration.sql"));
    ctx.set_file(None);
    assert!(ctx.file().is_none());
}

#[test]
fn stat_context_sql() {
    let mut ctx = RdbcStatContext::new();
    assert!(ctx.sql().is_none());
    ctx.set_sql(Some("SELECT 1".to_owned()));
    assert_eq!(ctx.sql(), Some("SELECT 1"));
    ctx.set_sql(None);
    assert!(ctx.sql().is_none());
}

#[test]
fn stat_context_clone_eq() {
    let mut ctx = RdbcStatContext::new();
    ctx.set_name(Some("test".to_owned()));
    ctx.set_sql(Some("SELECT 1".to_owned()));
    ctx.set_trace_enable(true);
    let ctx2 = ctx.clone();
    assert_eq!(ctx, ctx2);
}

#[test]
fn stat_context_debug() {
    let ctx = RdbcStatContext::new();
    let dbg = format!("{ctx:?}");
    assert!(dbg.contains("RdbcStatContext"));
}

#[test]
fn stat_context_all_fields() {
    let mut ctx = RdbcStatContext::new();
    ctx.set_trace_enable(true);
    ctx.set_request_id(Some("req".to_owned()));
    ctx.set_name(Some("name".to_owned()));
    ctx.set_file(Some("file".to_owned()));
    ctx.set_sql(Some("sql".to_owned()));
    assert!(ctx.is_trace_enable());
    assert_eq!(ctx.request_id(), Some("req"));
    assert_eq!(ctx.name(), Some("name"));
    assert_eq!(ctx.file(), Some("file"));
    assert_eq!(ctx.sql(), Some("sql"));
}
