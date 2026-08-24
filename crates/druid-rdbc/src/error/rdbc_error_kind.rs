//! RDBC 错误的稳定分类。

/// RDBC 标准与 adapter 边界的错误分类。
///
/// 对应 Java：`SQLException` 谱系、JDBC 默认方法的
/// `UnsupportedOperationException`，以及数据库 adapter 的获取、校验和驱动错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdbcErrorKind {
    /// 连接池或外部资源池已关闭。
    PoolClosed,
    /// 获取连接超时。
    AcquireTimeout,
    /// Druid-compatible 获取超时诊断。
    GetConnectionTimeout,
    /// 物理驱动登录超时。
    LoginTimeout,
    /// 数据源被禁用。
    DataSourceDisabled,
    /// 数据源已经关闭。
    DataSourceClosed,
    /// 连接池容量已耗尽。
    PoolExhausted,
    /// 数据源名称未注册。
    DataSourceNotFound,
    /// 当前没有可用数据源节点。
    DataSourceNotAvailable,
    /// 等待获取连接的任务数超过上限。
    MaxWaitThreadCountExceeded,
    /// 连接已经失效或被丢弃。
    ConnectionDiscarded,
    /// logical connection 未按生命周期合同释放。
    ConnectionLeaked,
    /// 连接校验失败。
    ValidationFailed,
    /// 具体驱动或 adapter 执行失败。
    Driver,
    /// 普通 SQL 异常。
    SqlException,
    /// SQL feature 不受支持，SQLState 为 `0A000`。
    SqlFeatureNotSupported,
    /// 批处理异常。
    BatchUpdateException,
    /// 调用参数不满足标准合同。
    InvalidArgument,
    /// SQL 文本解析失败。
    SqlParse,
    /// SQL 防火墙拒绝语句。
    WallViolation,
    /// 没有注册匹配的驱动扩展。
    NoDriverExtension,
    /// Java JDBC 默认方法的 `UnsupportedOperationException` 映射。
    UnsupportedOperation,
    /// 未能进一步分类的 adapter 错误。
    Other,
}
