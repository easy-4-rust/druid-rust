use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// JDBC Agent 子进程启动和协议防护配置。
#[derive(Debug, Clone)]
pub struct JdbcAgentOptions {
    program: OsString,
    arguments: Vec<OsString>,
    request_timeout: Duration,
    max_frame_bytes: usize,
    idle_timeout: Duration,
}

impl JdbcAgentOptions {
    /// 创建不经过 shell 的 Agent 命令配置。
    #[must_use]
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            arguments: Vec::new(),
            request_timeout: Duration::from_secs(30),
            max_frame_bytes: 16 * 1024 * 1024,
            idle_timeout: Duration::from_secs(60),
        }
    }

    /// 创建跨平台 Java classpath 启动配置。
    pub fn java(
        java_program: impl Into<OsString>,
        class_path: impl IntoIterator<Item = PathBuf>,
    ) -> Result<Self, std::env::JoinPathsError> {
        let class_path = std::env::join_paths(class_path)?;
        Ok(Self::new(java_program)
            .argument("-cp")
            .argument(class_path)
            .argument("io.github.easy4rust.druid.agent.JdbcAgentMain"))
    }

    /// 追加一个原样传给子进程的参数。
    #[must_use]
    pub fn argument(mut self, argument: impl Into<OsString>) -> Self {
        self.arguments.push(argument.into());
        self
    }

    /// 设置单次协议请求超时。
    #[must_use]
    pub const fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// 设置单帧最大字节数，防止失控 Agent 消耗无限内存。
    #[must_use]
    pub const fn max_frame_bytes(mut self, max_frame_bytes: usize) -> Self {
        self.max_frame_bytes = max_frame_bytes;
        self
    }

    /// 设置最后一个 session 关闭后保留共享 Agent 进程的时间。
    #[must_use]
    pub const fn idle_timeout(mut self, idle_timeout: Duration) -> Self {
        self.idle_timeout = idle_timeout;
        self
    }

    pub(crate) fn program(&self) -> &OsStr {
        &self.program
    }

    pub(crate) fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    pub(crate) const fn timeout(&self) -> Duration {
        self.request_timeout
    }

    pub(crate) const fn frame_limit(&self) -> usize {
        self.max_frame_bytes
    }

    pub(crate) const fn runtime_idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    pub(crate) fn runtime_key(&self) -> String {
        let mut key = self.program.to_string_lossy().into_owned();
        for argument in &self.arguments {
            key.push('\0');
            key.push_str(&argument.to_string_lossy());
        }
        key.push_str(&format!("\0frame={}", self.max_frame_bytes));
        key
    }

    /// 返回 classpath 中建议使用的 Agent uber-jar 文件名。
    #[must_use]
    pub fn bundled_agent_jar(base_directory: impl AsRef<Path>) -> PathBuf {
        base_directory.as_ref().join("druid-jdbc-agent.jar")
    }
}
