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
        Err(RdbcError::feature_not_supported(
            "data_source_connection_with_credentials",
        ))
    }

    /// 使用可空用户名与密码获取有所有权的标准连接。
    /// 对应 Java: `javax.sql.DataSource#getConnection(String, String)` 的 NULL 参数。
    /// 两个 None 委托无凭据入口；两个 Some（包括空串）原样委托既有凭据校验。
    /// 部分 NULL 默认返回 0A000，不借用连接；适配器可覆盖以提供精确驱动语义。
    async fn connection_with_nullable_credentials(
        &self,
        user: Option<&str>,
        password: Option<&str>,
    ) -> Result<Box<dyn Connection>, RdbcError> {
        match (user, password) {
            (None, None) => self.connection().await,
            (Some(user), Some(password)) => self.connection_with_credentials(user, password).await,
            _ => Err(RdbcError::feature_not_supported(
                "data_source_connection_with_nullable_credentials",
            )),
        }
    }
}
