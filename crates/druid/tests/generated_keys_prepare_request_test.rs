//! 标准 generated-keys 准备请求的传播与缓存隔离；受控 raw fixture 不替代实库键值验证。

use druid::core::RdbcError;
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{
    GeneratedKeys, PreparedStatement, ResultSetConcurrency, ResultSetHoldability, ResultSetType,
    Statement, StatementOptions,
};
use std::sync::{Arc, Mutex};

type Requests = Arc<Mutex<Vec<(String, StatementOptions, GeneratedKeys)>>>;

struct RequestConnection {
    requests: Requests,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(RequestConnection => Connection);

#[async_trait::async_trait]
impl Connection for RequestConnection {
    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        self.prepare_statement_with_generated_keys(sql, options, GeneratedKeys::None)
            .await
    }

    async fn prepare_statement_with_generated_keys(
        &mut self,
        sql: &str,
        options: StatementOptions,
        keys: GeneratedKeys,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        if self.closed {
            return Err(RdbcError::ConnectionDiscarded);
        }
        self.requests
            .lock()
            .unwrap()
            .push((sql.to_owned(), options, keys));
        Ok(Box::new(RequestPrepared { closed: false }))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

struct RequestPrepared {
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(RequestPrepared => PreparedStatement);

#[async_trait::async_trait]
impl Statement for RequestPrepared {
    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for RequestPrepared {
    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
}

struct RequestFactory(Requests);

#[async_trait::async_trait]
impl ConnectionFactory for RequestFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(RequestConnection {
            requests: self.0.clone(),
            closed: false,
        }))
    }
    fn name(&self) -> &str {
        "generated-keys-request-fixture"
    }
}

#[tokio::test]
async fn pooled_prepare_forwards_all_requests_and_reuses_only_identical_keys() {
    let requests = Requests::default();
    let pool = DruidPool::builder()
        .factory(Arc::new(RequestFactory(requests.clone())))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(8)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let modes = vec![
        GeneratedKeys::None,
        GeneratedKeys::ReturnGeneratedKeys,
        GeneratedKeys::ColumnIndexes(vec![
            ColumnIndex::new(1).unwrap(),
            ColumnIndex::new(2).unwrap(),
        ]),
        GeneratedKeys::ColumnIndexes(vec![
            ColumnIndex::new(2).unwrap(),
            ColumnIndex::new(1).unwrap(),
        ]),
        GeneratedKeys::ColumnNames(vec!["id".into(), "value".into()]),
        GeneratedKeys::ColumnNames(vec!["value".into(), "id".into()]),
    ];
    let sql = "INSERT INTO item(value) VALUES (?)";
    let options = StatementOptions::new(
        ResultSetType::ScrollInsensitive,
        ResultSetConcurrency::ReadOnly,
        ResultSetHoldability::HoldOverCommit,
    );
    for keys in &modes {
        let mut prepared = connection
            .prepare_statement_with_generated_keys(sql, options, keys.clone())
            .await
            .unwrap();
        prepared.close().await.unwrap();
    }
    assert_eq!(
        *requests.lock().unwrap(),
        modes
            .iter()
            .cloned()
            .map(|keys| (sql.to_owned(), options, keys))
            .collect::<Vec<_>>()
    );
    assert_eq!(pool.state().cached_prepared_statement_count, 6);
    for keys in &modes {
        let mut prepared = connection
            .prepare_statement_with_generated_keys(sql, options, keys.clone())
            .await
            .unwrap();
        prepared.close().await.unwrap();
    }
    assert_eq!(
        requests.lock().unwrap().len(),
        6,
        "各请求须命中自己的缓存条目"
    );
    assert_eq!(pool.state().cached_prepared_statement_hit_count, 6);
    // 显式 None 与普通标准入口代表同一准备请求，不能形成重复缓存条目。
    let mut plain = connection.prepare_statement(sql, options).await.unwrap();
    plain.close().await.unwrap();
    assert_eq!(requests.lock().unwrap().len(), 6);
    assert_eq!(pool.state().cached_prepared_statement_hit_count, 7);
    let mut different_options = connection
        .prepare_statement_with_generated_keys(
            sql,
            StatementOptions::default(),
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .unwrap();
    different_options.close().await.unwrap();
    assert_eq!(
        requests.lock().unwrap().last(),
        Some(&(
            sql.to_owned(),
            StatementOptions::default(),
            GeneratedKeys::ReturnGeneratedKeys
        ))
    );
    assert_eq!(
        requests.lock().unwrap().len(),
        7,
        "结果集选项变化不能错误复用同 SQL/mode 缓存条目"
    );
    assert_eq!(pool.state().cached_prepared_statement_count, 7);
    connection.close().await.unwrap();
    pool.close().await;
}

struct LegacyOnlyConnection {
    calls: usize,
}
druid_rdbc::impl_standard_wrapper!(LegacyOnlyConnection => Connection);
#[async_trait::async_trait]
impl Connection for LegacyOnlyConnection {
    async fn prepare_statement(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        self.calls += 1;
        Ok(Box::new(RequestPrepared { closed: false }))
    }
}

#[tokio::test]
async fn canonical_default_preserves_none_and_rejects_unhandled_requests_before_prepare() {
    let mut connection = LegacyOnlyConnection { calls: 0 };
    for keys in [
        GeneratedKeys::ReturnGeneratedKeys,
        GeneratedKeys::ColumnIndexes(vec![ColumnIndex::new(1).unwrap()]),
        GeneratedKeys::ColumnNames(vec!["id".into()]),
    ] {
        let result = connection
            .prepare_statement_with_generated_keys("INSERT", StatementOptions::default(), keys)
            .await;
        assert!(matches!(
            result,
            Err(RdbcError::UnsupportedOperation {
                operation: "connection_prepare_statement_generated_keys"
            })
        ));
        assert_eq!(
            connection.calls, 0,
            "不支持时不得先创建 raw statement 再丢弃请求"
        );
    }
    connection
        .prepare_statement_with_generated_keys(
            "INSERT",
            StatementOptions::default(),
            GeneratedKeys::None,
        )
        .await
        .unwrap()
        .close()
        .await
        .unwrap();
    assert_eq!(connection.calls, 1);
}
