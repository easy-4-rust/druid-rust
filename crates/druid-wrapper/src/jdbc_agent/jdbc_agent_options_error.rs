/// JDBC Agent 启动参数构造失败。
#[derive(Debug, thiserror::Error)]
pub enum JdbcAgentOptionsError {
    /// Java classpath 包含平台无法表示的路径。
    #[error("invalid Java classpath: {0}")]
    ClassPath(#[from] std::env::JoinPathsError),
    /// JVM 参数不在受管 allowlist 内。
    #[error("JVM option is not allowed for the managed JDBC Agent: {0}")]
    UnsafeJvmOption(String),
}
