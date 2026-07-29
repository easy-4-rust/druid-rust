use std::collections::HashMap;
use std::time::Duration;

use super::{
    DruidError, PhysicalConnection, ValidConnectionChecker, ValidConnectionCheckerAdapter,
};

/// MySQL 连接校验器。
///
/// 对应 Java: `com.alibaba.druid.pool.vendor.MySqlValidConnectionChecker`。
#[derive(Clone, Copy, Debug)]
pub struct MySqlValidConnectionChecker {
    use_ping_method: bool,
}

impl MySqlValidConnectionChecker {
    /// Java 默认验证 SQL。
    pub const DEFAULT_VALIDATION_QUERY: &'static str = "/* ping */ SELECT 1";
    /// Java 默认验证超时秒数。
    pub const DEFAULT_VALIDATION_QUERY_TIMEOUT: u64 = 1;

    /// 创建默认启用 ping SQL 的校验器。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            use_ping_method: true,
        }
    }

    /// 返回是否强制使用 MySQL ping SQL。
    #[must_use]
    pub const fn is_use_ping_method(&self) -> bool {
        self.use_ping_method
    }

    /// 设置是否强制使用 MySQL ping SQL。
    pub fn set_use_ping_method(&mut self, use_ping_method: bool) {
        self.use_ping_method = use_ping_method;
    }
}

impl Default for MySqlValidConnectionChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ValidConnectionChecker for MySqlValidConnectionChecker {
    async fn is_valid_connection(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
        query: Option<&str>,
        validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        if connection.is_closed() {
            return Ok(false);
        }
        let query = if self.use_ping_method {
            Self::DEFAULT_VALIDATION_QUERY
        } else {
            query
                .filter(|query| !query.is_empty())
                .unwrap_or(Self::DEFAULT_VALIDATION_QUERY)
        };
        ValidConnectionCheckerAdapter::exec_valid_query(connection, query, validation_query_timeout)
            .await
    }

    fn config_from_properties(&mut self, properties: &HashMap<String, String>) {
        match properties
            .get("druid.mysql.usePingMethod")
            .map(String::as_str)
        {
            Some("true") => self.set_use_ping_method(true),
            Some("false") => self.set_use_ping_method(false),
            _ => {}
        }
    }
}
