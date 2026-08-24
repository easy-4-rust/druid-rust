use crate::data_source::{CommonDataSource, XaConnection};
use crate::error::RdbcError;

/// `javax.sql.XADataSource` 的 acquisition 标准。
#[async_trait::async_trait]
pub trait XaDataSource: CommonDataSource {
    /// 获取一个 owned XA connection。
    async fn xa_connection(&self) -> Result<Box<dyn XaConnection>, RdbcError>;

    /// 使用 caller credentials 获取 owned XA connection。
    async fn xa_connection_with_credentials(
        &self,
        _user: &str,
        _password: &str,
    ) -> Result<Box<dyn XaConnection>, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "xa_data_source_xa_connection_with_credentials",
        ))
    }
}
