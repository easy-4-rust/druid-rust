use std::collections::HashMap;
use std::time::Duration;

use super::{
    DruidError, PhysicalConnection, ValidConnectionChecker, ValidConnectionCheckerAdapter,
};

/// Oracle 连接校验器。
///
/// 对应 Java: `com.alibaba.druid.pool.vendor.OracleValidConnectionChecker`。
#[derive(Clone, Copy, Debug)]
pub struct OracleValidConnectionChecker {
    timeout: Duration,
}

impl OracleValidConnectionChecker {
    /// Java 默认验证 SQL。
    pub const DEFAULT_VALIDATE_QUERY: &'static str = "SELECT 'x' FROM DUAL";

    /// 创建默认一秒超时的校验器。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            timeout: Duration::from_secs(1),
        }
    }

    /// 设置默认 ping 超时秒数。
    pub fn set_timeout(&mut self, seconds: u64) {
        self.timeout = Duration::from_secs(seconds);
    }
}

impl Default for OracleValidConnectionChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ValidConnectionChecker for OracleValidConnectionChecker {
    async fn is_valid_connection(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
        query: Option<&str>,
        validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        if connection.is_closed() {
            return Ok(false);
        }
        let timeout = if validation_query_timeout.is_zero() {
            self.timeout
        } else {
            validation_query_timeout
        };
        ValidConnectionCheckerAdapter::exec_valid_query(
            connection,
            query
                .filter(|query| !query.is_empty())
                .unwrap_or(Self::DEFAULT_VALIDATE_QUERY),
            timeout,
        )
        .await
    }

    fn config_from_properties(&mut self, properties: &HashMap<String, String>) {
        if let Some(seconds) = properties
            .get("druid.oracle.pingTimeout")
            .filter(|value| !value.is_empty())
            .and_then(|value| value.parse::<u64>().ok())
        {
            self.set_timeout(seconds);
        }
    }
}
