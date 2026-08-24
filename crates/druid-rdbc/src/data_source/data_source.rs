use crate::connection::Connection;
use crate::data_source::CommonDataSource;
use crate::error::RdbcError;
use crate::wrapper::Wrapper;

/// `javax.sql.DataSource` 的 standard acquisition contract。
#[async_trait::async_trait]
pub trait DataSource: CommonDataSource + Wrapper + Sync {
    /// 获取一个 owned standard connection。
    async fn connection(&self) -> Result<Box<dyn Connection>, RdbcError>;

    /// 使用 caller credentials 获取 owned connection。
    async fn connection_with_credentials(
        &self,
        _user: &str,
        _password: &str,
    ) -> Result<Box<dyn Connection>, RdbcError> {
        self.connection().await
    }
}
