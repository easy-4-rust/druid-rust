use std::time::Duration;

use super::{DruidError, PhysicalConnection, ValidConnectionChecker, Value};

/// 使用验证 SQL 的通用连接校验器。
///
/// 对应 Java: `com.alibaba.druid.pool.ValidConnectionCheckerAdapter`。空验证 SQL
/// 直接返回 `true`；非空 SQL 在 raw `PhysicalConnection` 上执行，并以是否存在
/// 第一行作为有效性结果。
#[derive(Clone, Copy, Debug, Default)]
pub struct ValidConnectionCheckerAdapter;

impl ValidConnectionCheckerAdapter {
    /// 在物理连接上执行验证 SQL。
    pub async fn exec_valid_query(
        connection: &mut Box<dyn PhysicalConnection>,
        query: &str,
        validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        let fetch = connection.fetch(query, Vec::<Value>::new());
        if validation_query_timeout.is_zero() {
            return fetch.await.map(|rows| !rows.is_empty());
        }
        match tokio::time::timeout(validation_query_timeout, fetch).await {
            Ok(result) => result.map(|rows| !rows.is_empty()),
            Err(_) => Ok(false),
        }
    }
}

#[async_trait::async_trait]
impl ValidConnectionChecker for ValidConnectionCheckerAdapter {
    async fn is_valid_connection(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
        query: Option<&str>,
        validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        let Some(query) = query.filter(|query| !query.is_empty()) else {
            return Ok(true);
        };
        Self::exec_valid_query(connection, query, validation_query_timeout).await
    }
}
