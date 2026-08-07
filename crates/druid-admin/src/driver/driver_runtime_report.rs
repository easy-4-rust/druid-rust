use druid_wrapper::jdbc_agent::JdbcAgentRuntimeMetricsSnapshot;
use serde::Serialize;

/// 单个数据库产品的 JDBC Agent 就绪度诊断报告。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct DriverRuntimeReport {
    profile_id: String,
    catalog_status: String,
    runtime_mode: String,
    artifact_id: String,
    catalog_artifact_version: Option<String>,
    agent_artifact_version: Option<String>,
    driver_artifact_version: Option<String>,
    agent_installed: bool,
    driver_installed: bool,
    java_available: bool,
    java_version: Option<u16>,
    java_baseline_met: bool,
    ready: bool,
    runtime_metrics: JdbcAgentRuntimeMetricsSnapshot,
    messages: Vec<String>,
}

impl DriverRuntimeReport {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        profile_id: String,
        catalog_status: String,
        runtime_mode: String,
        artifact_id: String,
        catalog_artifact_version: Option<String>,
        agent_artifact_version: Option<String>,
        driver_artifact_version: Option<String>,
        agent_installed: bool,
        driver_installed: bool,
        java_version: Option<u16>,
        runtime_metrics: JdbcAgentRuntimeMetricsSnapshot,
        messages: Vec<String>,
    ) -> Self {
        let java_available = java_version.is_some();
        let java_baseline_met = java_version.is_some_and(|version| version >= 17);
        Self {
            profile_id,
            catalog_status,
            runtime_mode,
            artifact_id,
            catalog_artifact_version,
            agent_artifact_version,
            driver_artifact_version,
            agent_installed,
            driver_installed,
            java_available,
            java_version,
            java_baseline_met,
            ready: agent_installed && driver_installed && java_baseline_met,
            runtime_metrics,
            messages,
        }
    }

    /// 返回运行时是否具备启动条件；不代表数据库网络已连通。
    #[must_use]
    pub const fn ready(&self) -> bool {
        self.ready
    }

    /// 返回探测到的 Java 主版本。
    #[must_use]
    pub const fn java_version(&self) -> Option<u16> {
        self.java_version
    }
}
