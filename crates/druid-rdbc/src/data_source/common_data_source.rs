use crate::error::RdbcError;
use std::time::Duration;

/// `javax.sql.CommonDataSource` 的共享配置标准。
pub trait CommonDataSource: Send + Sync {
    /// 返回 login timeout；`None` 表示实现未配置。
    fn login_timeout(&self) -> Option<Duration> {
        None
    }

    /// 设置 login timeout。
    fn set_login_timeout(&mut self, _timeout: Duration) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "common_data_source_set_login_timeout",
        })
    }
}
