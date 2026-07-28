//! Java Druid PreparedStatement pool 纵向契约。
//!
//! Java oracle：
//! - `DruidConnectionHolderTest4#test_toString`
//! - `PSCacheTest3#test_pscache`
//! - `DruidDataSourceTest_clearCache#test_clearStatementCache`

use druid_core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalPreparedStatement, PreparedStatementKey,
    Row, SqlTextPreparedStatement, Value,
};
use druid_pool::DruidPool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

struct PreparedConnection {
    prepare_count: Arc<AtomicU64>,
    schema: String,
    closed: bool,
}

#[async_trait::async_trait]
impl PhysicalConnection for PreparedConnection {
    async fn exec(&mut self, sql: &str, _params: Vec<Value>) -> Result<ExecResult, DruidError> {
        if sql == "FAIL" {
            return Err(DruidError::DriverError("expected failure".to_string()));
        }
        Ok(ExecResult {
            rows_affected: 1,
            last_insert_id: None,
            row_count: None,
        })
    }

    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(vec![Row::new(vec![Value::Int(1)])])
    }

    async fn prepare_physical_statement(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        self.prepare_count.fetch_add(1, Ordering::Relaxed);
        Ok(Arc::new(SqlTextPreparedStatement::new(key.sql())))
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed
    }

    fn schema(&self) -> Option<&str> {
        Some(&self.schema)
    }

    async fn set_schema(&mut self, schema: &str) -> Result<(), DruidError> {
        self.schema = schema.to_string();
        Ok(())
    }
}

struct PreparedFactory {
    prepare_count: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl druid_core::PhysicalConnectionFactory for PreparedFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        Ok(Box::new(PreparedConnection {
            prepare_count: self.prepare_count.clone(),
            schema: "main".to_string(),
            closed: false,
        }))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }

    async fn close(&self, connection: &mut Box<dyn PhysicalConnection>) -> Result<(), DruidError> {
        connection.close().await
    }
}

async fn prepared_pool(max_statements: usize) -> (DruidPool, Arc<AtomicU64>) {
    let prepare_count = Arc::new(AtomicU64::new(0));
    let pool = DruidPool::builder()
        .name("prepared")
        .db_type_name("mysql")
        .factory(Arc::new(PreparedFactory {
            prepare_count: prepare_count.clone(),
        }))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(max_statements)
        .build()
        .await
        .unwrap();
    (pool, prepare_count)
}

#[tokio::test]
async fn holder_statement_pool_is_lazy_and_debug_does_not_initialize_it() {
    let (pool, _) = prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    assert!(connection
        .connection_holder()
        .unwrap()
        .statement_pool_direct()
        .is_none());

    let _ = format!("{:?}", connection.connection_holder().unwrap());
    assert!(connection
        .connection_holder()
        .unwrap()
        .statement_pool_direct()
        .is_none());

    let statement_pool = connection.connection_holder_mut().unwrap().statement_pool();
    assert_eq!(
        statement_pool
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .size(),
        0
    );
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn prepared_statement_reuses_physical_handle_and_updates_java_stats() {
    let (pool, prepare_count) = prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();

    let mut first = connection.prepare_statement("select 1").await.unwrap();
    assert!(first.prepared_statement_holder().is_in_use());
    assert!(!first.prepared_statement_holder().is_pooling());
    assert_eq!(first.fetch(&mut connection, vec![]).await.unwrap().len(), 1);
    first.close().unwrap();
    assert!(!first.prepared_statement_holder().is_in_use());
    assert!(first.prepared_statement_holder().is_pooling());
    drop(first);

    let mut second = connection.prepare_statement("select 1").await.unwrap();
    assert_eq!(prepare_count.load(Ordering::Relaxed), 1);
    assert_eq!(second.prepared_statement_holder().hit_count(), 1);
    second.exec(&mut connection, vec![]).await.unwrap();
    second.close().unwrap();
    drop(second);

    let state = pool.state();
    assert_eq!(state.prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_hit_count, 1);
    assert_eq!(state.cached_prepared_statement_miss_count, 1);
    assert_eq!(state.cached_prepared_statement_access_count, 2);

    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn concurrent_logical_statements_preserve_java_in_use_and_lru_semantics() {
    let (pool, _) = prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();

    let mut first = connection.prepare_statement("select 0").await.unwrap();
    first.close().unwrap();
    drop(first);

    let mut active = connection.prepare_statement("select 0").await.unwrap();
    assert!(active.prepared_statement_holder().is_pooling());
    assert!(active.prepared_statement_holder().is_in_use());

    for sql in ["select 1", "select 2", "select 3", "select 4"] {
        let mut statement = connection.prepare_statement(sql).await.unwrap();
        statement.close().unwrap();
    }
    let active_holder = active.prepared_statement_holder() as *const _;
    assert!(!active.prepared_statement_holder().is_pooling());
    active.close().unwrap();
    assert!(active.prepared_statement_holder().is_pooling());
    assert_eq!(
        active.prepared_statement_holder() as *const _,
        active_holder
    );
    drop(active);

    let statement_pool = connection
        .connection_holder()
        .unwrap()
        .statement_pool_direct()
        .unwrap();
    assert_eq!(
        statement_pool
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keys_in_lru_order()
            .iter()
            .map(PreparedStatementKey::sql)
            .collect::<Vec<_>>(),
        vec!["select 3", "select 4", "select 0"]
    );

    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn mysql_schema_change_clears_existing_statement_cache() {
    let (pool, _) = prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection.prepare_statement("select 1").await.unwrap();
    statement.close().unwrap();
    drop(statement);
    assert_eq!(pool.state().cached_prepared_statement_count, 1);

    connection.set_schema("tenant").await.unwrap();
    let state = pool.state();
    assert_eq!(state.cached_prepared_statement_count, 0);
    assert_eq!(state.cached_prepared_statement_delete_count, 1);
    assert_eq!(state.closed_prepared_statement_count, 1);

    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn statement_execution_error_removes_handle_instead_of_reusing_it() {
    let (pool, _) = prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection.prepare_statement("FAIL").await.unwrap();
    assert!(statement.exec(&mut connection, vec![]).await.is_err());
    statement.close().unwrap();
    assert!(!statement.prepared_statement_holder().is_pooling());
    drop(statement);

    let state = pool.state();
    // Java closePreapredStatement 对尚未入缓存的失败语句也无条件递减。
    assert_eq!(state.cached_prepared_statement_count, -1);
    assert_eq!(state.cached_prepared_statement_delete_count, 1);
    assert_eq!(state.closed_prepared_statement_count, 1);

    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn prepared_statement_cannot_cross_connection_lease_boundary() {
    let (pool, prepare_count) = prepared_pool(3).await;
    let mut first_connection = pool.get().await.unwrap();

    let mut seed = first_connection
        .prepare_statement("select 1")
        .await
        .unwrap();
    seed.close().unwrap();
    let mut leaked = first_connection
        .prepare_statement("select 1")
        .await
        .unwrap();
    assert_eq!(leaked.prepared_statement_holder().hit_count(), 1);
    first_connection.close().await.unwrap();

    let mut second_connection = pool.get().await.unwrap();
    assert!(leaked.exec(&mut second_connection, vec![]).await.is_err());
    let mut replacement = second_connection
        .prepare_statement("select 1")
        .await
        .unwrap();
    replacement.close().unwrap();
    assert_eq!(prepare_count.load(Ordering::Relaxed), 2);

    drop(leaked);
    let state = pool.state();
    assert_eq!(state.cached_prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_delete_count, 1);
    assert_eq!(state.closed_prepared_statement_count, 1);

    second_connection.close().await.unwrap();
    pool.close().await;
}
