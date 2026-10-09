use super::connection::HandoffConnection;
use super::driver_state::DriverState;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use std::sync::Arc;

/// 为 owned getter 交接合同提供真实标准连接工厂。
/// 对应 Java: java.sql.Driver 的测试工厂。
pub struct HandoffFactory(pub Arc<DriverState>);
#[async_trait::async_trait]
impl ConnectionFactory for HandoffFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(HandoffConnection(Arc::clone(&self.0))))
    }
}
