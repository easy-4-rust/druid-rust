use std::time::Duration;

use super::{
    DruidError, PhysicalConnection, ValidConnectionChecker, ValidConnectionCheckerAdapter,
};

/// Microsoft SQL Server 连接校验器。
///
/// 对应 Java: `com.alibaba.druid.pool.vendor.MSSQLValidConnectionChecker`。
#[derive(Clone, Copy, Debug, Default)]
pub struct MsSqlValidConnectionChecker;

impl MsSqlValidConnectionChecker {
    /// Java 默认验证 SQL。
    pub const DEFAULT_VALIDATION_QUERY: &'static str = "SELECT 1";
}

#[async_trait::async_trait]
impl ValidConnectionChecker for MsSqlValidConnectionChecker {
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
                .unwrap_or(Self::DEFAULT_VALIDATION_QUERY),
            validation_query_timeout,
        )
        .await
    }
}
