//! 结果集诊断共享夹具：真实 SQLite 游标与 statement 构造。

use super::*;

/// 执行原五行 SQL，再由实际标准资源准入和池化代理接管驱动游标。
/// 参数为空；返回游标、创建语句与仍有效的真实驱动连接。
pub(crate) async fn task6_sqlite_diagnostic_cursor() -> (
    DruidPooledResultSet,
    DruidPooledStatement,
    Box<dyn Connection>,
) {
    task6_sqlite_diagnostic_cursor_with_filters(None).await
}

pub(crate) async fn task6_sqlite_diagnostic_cursor_with_filters(
    filters: Option<Arc<crate::core::FilterChain>>,
) -> (
    DruidPooledResultSet,
    DruidPooledStatement,
    Box<dyn Connection>,
) {
    task6_sqlite_diagnostic_cursor_with_sql_and_filters("SELECT * FROM t", filters).await
}

pub(crate) async fn task6_sqlite_diagnostic_cursor_with_sql_and_filters(
    sql: &str,
    filters: Option<Arc<crate::core::FilterChain>>,
) -> (
    DruidPooledResultSet,
    DruidPooledStatement,
    Box<dyn Connection>,
) {
    let (statement, connection) = task6_sqlite_diagnostic_statement_with_filters(filters).await;
    let admission = statement.inner.begin_standard_admission().unwrap();
    let raw = statement
        .inner
        .statement
        .lock()
        .await
        .execute_query(sql)
        .await
        .unwrap();
    let rows = DruidPooledResultSet::new_standard(Arc::clone(&statement.inner), raw, &admission)
        .await
        .unwrap();
    drop(admission);
    (rows, statement, connection)
}

/// 只装配真实五行 SQLite、标准 Statement 和 registry，不预先执行查询。
pub(crate) async fn task6_sqlite_diagnostic_statement_with_filters(
    filters: Option<Arc<crate::core::FilterChain>>,
) -> (DruidPooledStatement, Box<dyn Connection>) {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    connection
        .exec(
            "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, val REAL)",
            vec![],
        )
        .await
        .unwrap();
    for i in 1..=5 {
        connection
            .exec(
                &format!("INSERT INTO t (id, name, val) VALUES ({i}, 'row{i}', {i}.5)"),
                vec![],
            )
            .await
            .unwrap();
    }
    let resources = Arc::new(StandardResourceRegistry::default());
    let generation = resources.current_generation();
    let lease_active = Arc::new(AtomicBool::new(true));
    let policy = StandardLogicalPolicy::new(
        1,
        "sqlite-diagnostics".to_owned(),
        Arc::clone(&lease_active),
        Arc::new(AtomicBool::new(false)),
        filters.clone(),
        Some(true),
        generation,
        Arc::new(OwnerDispositionControl::new(Box::new(|_| {}))),
    );
    let admission = resources.begin_admission(generation).unwrap();
    let raw_statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let statement = DruidPooledStatement::new_standard(
        raw_statement,
        StatementOptions::default(),
        Arc::clone(&resources),
        policy,
        1,
        1,
        Arc::new(AtomicU64::new(1)),
        Arc::new(AtomicU64::new(1)),
        lease_active,
        filters,
        None,
    );
    resources
        .register(&admission, Arc::clone(&statement.inner))
        .await
        .unwrap();
    drop(admission);
    (statement, connection)
}

/// 创建指定原上下文的真实SQLite标准语句，登记实际资源所有权。
pub(crate) async fn task6_sqlite_metadata_statement(
    factory: &ToastyConnectionFactory,
    id: u64,
    name: &str,
    filters: Arc<crate::core::FilterChain>,
) -> (DruidPooledStatement, Box<dyn Connection>) {
    let mut connection = factory.create().await.unwrap();
    let resources = Arc::new(StandardResourceRegistry::default());
    let generation = resources.current_generation();
    let lease_active = Arc::new(AtomicBool::new(true));
    let policy = StandardLogicalPolicy::new(
        id,
        name.to_owned(),
        Arc::clone(&lease_active),
        Arc::new(AtomicBool::new(false)),
        Some(Arc::clone(&filters)),
        Some(true),
        generation,
        Arc::new(OwnerDispositionControl::new(Box::new(|_| {}))),
    );
    let admission = resources.begin_admission(generation).unwrap();
    let raw = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let statement = DruidPooledStatement::new_standard(
        raw,
        StatementOptions::default(),
        Arc::clone(&resources),
        policy,
        id,
        1,
        Arc::new(AtomicU64::new(1)),
        Arc::new(AtomicU64::new(1)),
        lease_active,
        Some(filters),
        None,
    );
    resources
        .register(&admission, Arc::clone(&statement.inner))
        .await
        .unwrap();
    drop(admission);
    (statement, connection)
}
