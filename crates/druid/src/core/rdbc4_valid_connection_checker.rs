use std::time::Duration;

use super::{Connection, DruidError, ValidConnectionChecker};

/// RDBC 4 `Connection#isValid` 的 Rust Adapter。
///
/// 对应 Java: `com.alibaba.druid.pool.RDBC4ValidConnectionChecker`。Rust
/// `Connection#ping` 是各驱动对原生 `isValid` 的最小等价边界。
#[derive(Clone, Copy, Debug, Default)]
pub struct Rdbc4ValidConnectionChecker;

#[async_trait::async_trait]
impl ValidConnectionChecker for Rdbc4ValidConnectionChecker {
    async fn is_valid_connection(
        &self,
        connection: &mut Box<dyn Connection>,
        _query: Option<&str>,
        validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        let timeout_seconds = u32::try_from(validation_query_timeout.as_secs()).unwrap_or(u32::MAX);
        if validation_query_timeout.is_zero() {
            return connection.is_valid(0).await.map_err(DruidError::from);
        }
        match tokio::time::timeout(
            validation_query_timeout,
            connection.is_valid(timeout_seconds),
        )
        .await
        {
            Ok(result) => result.map_err(DruidError::from),
            Err(_) => Ok(false),
        }
    }
}

/// 早期 druid-rust 名称兼容；canonical 类型为
/// [`Rdbc4ValidConnectionChecker`]。
pub use Rdbc4ValidConnectionChecker as PingConnectionChecker;
