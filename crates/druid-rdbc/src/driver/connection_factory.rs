use crate::connection::Connection;
use crate::error::RdbcError;

/// Rust connection acquisition extension。
///
/// 与 JDBC [`crate::driver::Driver`] 的 URL negotiation 分离；每次返回 owned connection。
#[async_trait::async_trait]
pub trait ConnectionFactory: Send + Sync + 'static {
    /// 创建一个 owned standard connection。
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError>;

    /// 返回 factory name。
    fn name(&self) -> &str {
        ""
    }
}
