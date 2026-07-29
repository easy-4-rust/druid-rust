use std::time::Duration;

use super::{
    DruidError, PhysicalConnection, ValidConnectionChecker, ValidConnectionCheckerAdapter,
};

/// OceanBase 连接校验器。
///
/// 对应 Java: `com.alibaba.druid.pool.vendor.OceanBaseValidConnectionChecker`。
#[derive(Clone, Copy, Debug, Default)]
pub struct OceanBaseValidConnectionChecker {
    mysql_mode: bool,
}

impl OceanBaseValidConnectionChecker {
    /// Oracle 兼容模式默认 SQL。
    pub const COMMON_VALIDATE_QUERY: &'static str = "SELECT 'x' FROM DUAL";
    /// MySQL 兼容模式默认 SQL。
    pub const MYSQL_VALIDATE_QUERY: &'static str = "/* ping */ SELECT 1";

    /// 创建 Oracle 兼容模式校验器。
    #[must_use]
    pub const fn new() -> Self {
        Self { mysql_mode: false }
    }

    /// 创建 MySQL 兼容模式校验器。
    #[must_use]
    pub const fn mysql_mode() -> Self {
        Self { mysql_mode: true }
    }
}

#[async_trait::async_trait]
impl ValidConnectionChecker for OceanBaseValidConnectionChecker {
    async fn is_valid_connection(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
        query: Option<&str>,
        validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        if connection.is_closed() {
            return Ok(false);
        }
        let default_query = if self.mysql_mode {
            Self::MYSQL_VALIDATE_QUERY
        } else {
            Self::COMMON_VALIDATE_QUERY
        };
        ValidConnectionCheckerAdapter::exec_valid_query(
            connection,
            query
                .filter(|query| !query.is_empty())
                .unwrap_or(default_query),
            validation_query_timeout,
        )
        .await
    }
}
