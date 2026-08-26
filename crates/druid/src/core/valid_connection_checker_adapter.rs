use std::time::Duration;

use super::{Connection, DruidError, ValidConnectionChecker};
use druid_rdbc::statement::StatementOptions;

/// 使用验证 SQL 的通用连接校验器。
///
/// 对应 Java: `com.alibaba.druid.pool.ValidConnectionCheckerAdapter`。空验证 SQL
/// 直接返回 `true`；非空 SQL 在 raw `Connection` 上执行，并以是否存在
/// 第一行作为有效性结果。
#[derive(Clone, Copy, Debug, Default)]
pub struct ValidConnectionCheckerAdapter;

impl ValidConnectionCheckerAdapter {
    /// 在物理连接上执行验证 SQL。
    pub async fn exec_valid_query(
        connection: &mut Box<dyn Connection>,
        query: &str,
        validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        let fetch = async {
            let mut statement = connection
                .create_statement(StatementOptions::default())
                .await?;
            if !validation_query_timeout.is_zero() {
                let seconds = i32::try_from(validation_query_timeout.as_secs()).unwrap_or(i32::MAX);
                statement.set_query_timeout(seconds)?;
            }
            let mut result_set = statement.execute_query(query).await?;
            let valid = result_set.next().await?;
            result_set.close().await?;
            statement.close().await?;
            Ok::<bool, druid_rdbc::error::RdbcError>(valid)
        };
        if validation_query_timeout.is_zero() {
            return fetch.await.map_err(DruidError::from);
        }
        match tokio::time::timeout(validation_query_timeout, fetch).await {
            Ok(result) => result.map_err(DruidError::from),
            Err(_) => Ok(false),
        }
    }
}

#[async_trait::async_trait]
impl ValidConnectionChecker for ValidConnectionCheckerAdapter {
    async fn is_valid_connection(
        &self,
        connection: &mut Box<dyn Connection>,
        query: Option<&str>,
        validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        let Some(query) = query.filter(|query| !query.is_empty()) else {
            return Ok(true);
        };
        Self::exec_valid_query(connection, query, validation_query_timeout).await
    }
}
