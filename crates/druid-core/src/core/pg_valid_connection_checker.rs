use std::time::Duration;

use super::{
    DruidError, PhysicalConnection, ValidConnectionChecker, ValidConnectionCheckerAdapter,
};

/// PostgreSQL 连接校验器。
///
/// 对应 Java: `com.alibaba.druid.pool.vendor.PGValidConnectionChecker`。
#[derive(Clone, Copy, Debug, Default)]
pub struct PgValidConnectionChecker;

impl PgValidConnectionChecker {
    /// Java 默认验证 SQL。
    pub const DEFAULT_VALIDATE_QUERY: &'static str = "SELECT 'x'";
}

#[async_trait::async_trait]
impl ValidConnectionChecker for PgValidConnectionChecker {
    async fn is_valid_connection(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
        query: Option<&str>,
        validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        if connection.is_closed() {
            return Ok(false);
        }
        ValidConnectionCheckerAdapter::exec_valid_query(
            connection,
            query
                .filter(|query| !query.is_empty())
                .unwrap_or(Self::DEFAULT_VALIDATE_QUERY),
            validation_query_timeout,
        )
        .await
    }
}
