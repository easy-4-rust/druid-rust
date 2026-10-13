use crate::error::RdbcError;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

/// 可跨线程共享的 RDBC 诊断接收器；不绑定具体日志框架。
pub type RdbcLogWriter = Arc<parking_lot::Mutex<Box<dyn Write + Send>>>;

/// `javax.sql.CommonDataSource` 的共享配置标准。
pub trait CommonDataSource: Send + Sync {
    /// 返回旧版 login timeout；新调用方应使用 [`Self::try_login_timeout`] 保留读取错误。
    ///
    /// `None` 表示实现没有显式配置，调用方沿用系统或驱动默认值；`Some(Duration::ZERO)`
    /// 表示显式不限制登录等待。对应 Java: `CommonDataSource#getLoginTimeout`。
    #[deprecated(note = "use try_login_timeout to preserve data-source read errors")]
    fn login_timeout(&self) -> Option<Duration> {
        None
    }

    /// 可失败地读取 login timeout；旧实现默认保留其 getter 返回值。
    ///
    /// 实现若能从底层数据源读取错误，必须覆盖此方法并保留原始错误。`None` 表示未显式配置，
    /// `Some(Duration::ZERO)` 表示无限等待；正值按原始精度返回。Java 秒数映射不得将负数、
    /// 溢出或底层读取错误静默转换为默认值。
    fn try_login_timeout(&self) -> Result<Option<Duration>, RdbcError> {
        #[allow(deprecated)]
        let timeout = self.login_timeout();
        Ok(timeout)
    }

    /// 可失败地设置 login timeout；`Duration::ZERO` 表示无限等待。
    ///
    /// 参数为希望应用的超时；成功返回 `Ok(())`，底层拒绝、转换越界或设置失败时保留错误。
    fn set_login_timeout(&mut self, _timeout: Duration) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "common_data_source_set_login_timeout",
        })
    }

    /// 返回该数据源的日志接收器；未配置或已禁用时返回 `None`。
    fn get_log_writer(&self) -> Option<RdbcLogWriter> {
        None
    }

    /// 设置该数据源的日志接收器；传入 `None` 禁用日志。
    ///
    /// 默认实现明确报告不支持，避免旧实现静默丢弃调用方配置。
    fn set_log_writer(&self, _writer: Option<RdbcLogWriter>) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported(
            "common_data_source_set_log_writer",
        ))
    }

    /// 返回父日志命名空间；不支持时精确返回 SQLState `0A000`。
    fn get_parent_logger(&self) -> Result<&'static str, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "common_data_source_get_parent_logger",
        ))
    }
}
