use super::JdbcAgentOptionsError;
use sha2::{Digest, Sha256};
use std::ffi::{OsStr, OsString};
use std::fmt::Write;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// JDBC Agent 子进程启动和协议防护配置。
#[derive(Debug, Clone)]
pub struct JdbcAgentOptions {
    program: OsString,
    arguments: Vec<OsString>,
    request_timeout: Duration,
    max_frame_bytes: usize,
    idle_timeout: Duration,
    agent_key: String,
    artifact_version: String,
    jvm_options_hash: String,
    artifact_leases: Vec<Arc<File>>,
    contract_fault_injection: bool,
}

impl JdbcAgentOptions {
    /// 创建不经过 shell 的 Agent 命令配置。
    #[must_use]
    pub fn new(program: impl Into<OsString>) -> Self {
        let program = program.into();
        let agent_key = program.to_string_lossy().into_owned();
        let jvm_options_hash = Self::identity_hash([program.as_os_str()]);
        Self {
            program,
            arguments: Vec::new(),
            request_timeout: Duration::from_secs(30),
            max_frame_bytes: 16 * 1024 * 1024,
            idle_timeout: Duration::from_secs(60),
            agent_key,
            artifact_version: "unmanaged".to_owned(),
            jvm_options_hash,
            artifact_leases: Vec::new(),
            contract_fault_injection: false,
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

    /// 使用 allowlist 中的 JVM 参数创建跨平台 Java Agent 启动配置。
    ///
    /// 明确拒绝 classpath 覆盖、`javaagent`、`agentlib`、`agentpath`、模块路径和
    /// 任意主类/命令注入；classpath 始终由受管制品列表生成。
    pub fn java_with_jvm_options(
        java_program: impl Into<OsString>,
        class_path: impl IntoIterator<Item = PathBuf>,
        jvm_options: impl IntoIterator<Item = OsString>,
    ) -> Result<Self, JdbcAgentOptionsError> {
        let class_path = std::env::join_paths(class_path)?;
        let jvm_options = jvm_options.into_iter().collect::<Vec<_>>();
        for option in &jvm_options {
            if !Self::allowed_jvm_option(option) {
                return Err(JdbcAgentOptionsError::UnsafeJvmOption(
                    option.to_string_lossy().into_owned(),
                ));
            }
        }
        let mut options = Self::new(java_program);
        for option in jvm_options {
            options = options.argument(option);
        }
        Ok(options
            .argument("-cp")
            .argument(class_path)
            .argument("io.github.easy4rust.druid.agent.JdbcAgentMain"))
    }

    /// 追加一个原样传给子进程的参数。
    #[must_use]
    pub fn argument(mut self, argument: impl Into<OsString>) -> Self {
        self.arguments.push(argument.into());
        self.jvm_options_hash = Self::identity_hash(
            std::iter::once(self.program.as_os_str())
                .chain(self.arguments.iter().map(OsString::as_os_str)),
        );
        self
    }

    /// 固定共享进程身份；受管安装器使用已校验制品摘要填充这些字段。
    #[must_use]
    pub fn runtime_identity(
        mut self,
        agent_key: impl Into<String>,
        artifact_version: impl Into<String>,
        jvm_options_hash: impl Into<String>,
    ) -> Self {
        self.agent_key = agent_key.into();
        self.artifact_version = artifact_version.into();
        self.jvm_options_hash = jvm_options_hash.into();
        self
    }

    /// 附加由受管安装器持有的制品使用锁；共享 Agent 退出前不会释放。
    #[doc(hidden)]
    #[must_use]
    pub fn artifact_lease(mut self, lease: File) -> Self {
        self.artifact_leases.push(Arc::new(lease));
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

    /// 仅为隔离的真实合同进程启用 Agent 崩溃与坏帧注入。
    ///
    /// 普通应用不得启用；能力只通过本地 stdin/stdout 协议暴露，不影响数据库。
    #[doc(hidden)]
    #[must_use]
    pub const fn contract_fault_injection(mut self, enabled: bool) -> Self {
        self.contract_fault_injection = enabled;
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

    pub(crate) const fn contract_fault_injection_enabled(&self) -> bool {
        self.contract_fault_injection
    }

    pub(crate) fn artifact_leases(&self) -> Vec<Arc<File>> {
        self.artifact_leases.clone()
    }

    pub(crate) fn runtime_key(&self) -> String {
        format!(
            "agent_key={}\0artifact_version={}\0jvm_options_hash={}\0frame={}\0fault={}",
            self.agent_key,
            self.artifact_version,
            self.jvm_options_hash,
            self.max_frame_bytes,
            self.contract_fault_injection
        )
    }

    /// 返回共享进程的 Agent 身份键。
    #[must_use]
    pub fn agent_key(&self) -> &str {
        &self.agent_key
    }

    /// 返回固定的 Agent 与驱动制品版本身份。
    #[must_use]
    pub fn artifact_version(&self) -> &str {
        &self.artifact_version
    }

    /// 返回经过摘要的 JVM 启动参数身份。
    #[must_use]
    pub fn jvm_options_hash(&self) -> &str {
        &self.jvm_options_hash
    }

    fn identity_hash<'a>(parts: impl IntoIterator<Item = &'a OsStr>) -> String {
        let mut digest = Sha256::new();
        for part in parts {
            digest.update(part.to_string_lossy().as_bytes());
            digest.update([0]);
        }
        let mut result = String::with_capacity(64);
        for byte in digest.finalize() {
            write!(&mut result, "{byte:02x}").expect("writing into String cannot fail");
        }
        result
    }

    fn allowed_jvm_option(option: &OsStr) -> bool {
        let option = option.to_string_lossy();
        option == "-Dfile.encoding=UTF-8"
            || option
                .strip_prefix("-Xms")
                .is_some_and(Self::valid_memory_size)
            || option
                .strip_prefix("-Xmx")
                .is_some_and(Self::valid_memory_size)
            || option
                .strip_prefix("-XX:MaxRAMPercentage=")
                .is_some_and(|value| {
                    value
                        .parse::<f64>()
                        .is_ok_and(|value| value.is_finite() && (1.0..=100.0).contains(&value))
                })
            || option
                .strip_prefix("-Duser.timezone=")
                .is_some_and(|value| {
                    !value.is_empty()
                        && value.len() <= 64
                        && value.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric()
                                || matches!(byte, b'/' | b'_' | b'-' | b'+' | b':')
                        })
                })
    }

    fn valid_memory_size(value: &str) -> bool {
        let Some((last, digits)) = value
            .as_bytes()
            .split_last()
            .map(|(last, digits)| (*last, digits))
        else {
            return false;
        };
        matches!(last, b'k' | b'K' | b'm' | b'M' | b'g' | b'G')
            && !digits.is_empty()
            && digits.iter().all(u8::is_ascii_digit)
    }

    /// 返回 classpath 中建议使用的 Agent uber-jar 文件名。
    #[must_use]
    pub fn bundled_agent_jar(base_directory: impl AsRef<Path>) -> PathBuf {
        base_directory.as_ref().join("druid-jdbc-agent.jar")
    }
}
