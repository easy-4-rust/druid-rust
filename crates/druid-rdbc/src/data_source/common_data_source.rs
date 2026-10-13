use crate::error::RdbcError;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

/// 可跨线程共享的 RDBC 诊断接收器；不绑定具体日志框架。
pub type RdbcLogWriter = Arc<parking_lot::Mutex<Box<dyn Write + Send>>>;

/// 写入不包含原始错误消息、URL 或连接属性的安全数据源诊断。
///
/// `operation` 只接受 ASCII 字母、数字、下划线、连字符和点；错误内容只输出稳定分类，
/// SQLState 必须是五位 ASCII 字母数字码。日志接收器错误只包含标准 I/O 错误类别。
///
/// # 参数
/// - `writer`：当前数据源的可选日志接收器；`None` 表示诊断已禁用。
/// - `operation`：固定操作标识，不得传入用户数据。
/// - `error`：连接或配置操作的原始标准错误；其消息不会写入日志。
///
/// # 返回
/// 成功写入或日志被禁用时返回 `Ok(())`；日志 I/O 失败返回不含接收器原始消息的错误。
pub fn write_data_source_diagnostic(
    writer: Option<RdbcLogWriter>,
    operation: &'static str,
    error: &RdbcError,
) -> Result<(), RdbcError> {
    let Some(writer) = writer else {
        return Ok(());
    };

    let operation = operation
        .bytes()
        .take(48)
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.') {
                char::from(byte)
            } else {
                '_'
            }
        })
        .collect::<String>();
    let sql_state = error
        .sql_exception()
        .and_then(|exception| exception.sql_state())
        .filter(|state| state.len() == 5 && state.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        .unwrap_or("unknown");

    use std::io::Write as _;
    let result = writeln!(
        writer.lock(),
        "rdbc-datasource operation={operation} error_kind={:?} sql_state={sql_state}",
        error.kind()
    )
    .map_err(|error| RdbcError::Other(format!("RDBC diagnostic sink failed ({:?})", error.kind())));
    result
}

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

    /// 使用当前实例的日志接收器记录安全诊断。
    ///
    /// 原始错误消息、URL、用户名、密码和连接属性不会写出。调用方必须显式处理 I/O
    /// 错误；若该诊断伴随另一个主操作错误，必须保留主操作错误，并单独报告本方法的错误。
    ///
    /// # 参数
    /// - `operation`：固定操作标识，不得包含用户数据。
    /// - `error`：连接或配置失败的原始错误。
    ///
    /// # 返回
    /// 成功写入或日志未启用时返回 `Ok(())`；writer 失败时返回可观察的诊断 I/O 错误。
    fn write_diagnostic(
        &self,
        operation: &'static str,
        error: &RdbcError,
    ) -> Result<(), RdbcError> {
        write_data_source_diagnostic(self.get_log_writer(), operation, error)
    }

    /// 返回父日志命名空间；不支持时精确返回 SQLState `0A000`。
    fn get_parent_logger(&self) -> Result<&'static str, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "common_data_source_get_parent_logger",
        ))
    }
}
