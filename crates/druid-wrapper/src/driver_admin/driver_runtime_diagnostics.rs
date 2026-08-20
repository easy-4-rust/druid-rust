use super::{DriverInstaller, DriverInstallerError, DriverRuntimeReport};
use crate::driver::{DatabaseProfileId, DruidDriverRegistry};
use crate::jdbc_agent::JdbcAgentRuntimeMetrics;
use std::ffi::OsString;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

/// 检查 catalog、Agent、驱动 JAR 和 Java 进程四个独立证据层。
#[derive(Debug, Clone)]
pub struct DriverRuntimeDiagnostics {
    installer: DriverInstaller,
    java_program: OsString,
}

impl DriverRuntimeDiagnostics {
    /// 创建使用 `java` 命令的诊断器。
    #[must_use]
    pub fn new(installer: DriverInstaller) -> Self {
        Self {
            installer,
            java_program: OsString::from("java"),
        }
    }

    /// 替换 Java 可执行文件路径。
    #[must_use]
    pub fn java_program(mut self, java_program: impl Into<OsString>) -> Self {
        self.java_program = java_program.into();
        self
    }

    /// 诊断本机启动条件，不尝试连接真实数据库。
    pub async fn check(
        &self,
        profile_id: &str,
    ) -> Result<DriverRuntimeReport, DriverInstallerError> {
        let registry = DruidDriverRegistry::builtin()?;
        let profile_id_value = DatabaseProfileId::new(profile_id)?;
        let profile = registry.profile(&profile_id_value)?;
        let catalog_status = format!("{:?}", profile.support_status());
        let agent = self.installer.active_installation("jdbc-agent").await;
        let driver = self.installer.active_installation(profile_id).await;
        let java_version = Self::java_version(&self.java_program).await;
        let mut messages = Vec::new();
        if agent.is_err() {
            messages.push("JDBC Agent uber-jar is not installed".to_owned());
        }
        if driver.is_err() {
            messages.push(format!("JDBC driver for '{profile_id}' is not installed"));
        }
        if java_version.is_none() {
            messages.push("Java runtime is not executable".to_owned());
        } else if java_version.is_some_and(|version| version < 17) {
            messages.push(format!(
                "Java 17 or newer is required; detected Java {}",
                java_version.unwrap_or_default()
            ));
        }
        if agent.is_ok() && driver.is_ok() && java_version.is_some_and(|version| version >= 17) {
            messages.push(
                "local runtime is ready; live database connectivity remains unverified".to_owned(),
            );
        }
        Ok(DriverRuntimeReport::new(
            profile_id.to_owned(),
            catalog_status,
            format!("{:?}", profile.runtime_mode()),
            profile.artifact_id().to_owned(),
            profile.artifact_version().map(ToOwned::to_owned),
            agent
                .as_ref()
                .ok()
                .map(|installation| installation.artifact_version().to_owned()),
            driver
                .as_ref()
                .ok()
                .map(|installation| installation.artifact_version().to_owned()),
            agent.is_ok(),
            driver.is_ok(),
            java_version,
            JdbcAgentRuntimeMetrics::snapshot(),
            messages,
        ))
    }

    async fn java_version(java_program: &OsString) -> Option<u16> {
        let mut command = Command::new(java_program);
        command
            .arg("-version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(5), command.output())
            .await
            .ok()?
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stderr);
        let text = if text.trim().is_empty() {
            String::from_utf8_lossy(&output.stdout)
        } else {
            text
        };
        let version = text.split('"').nth(1)?.split('.').next()?;
        let major = version.parse::<u16>().ok()?;
        if major == 1 {
            text.split('"').nth(1)?.split('.').nth(1)?.parse().ok()
        } else {
            Some(major)
        }
    }
}
