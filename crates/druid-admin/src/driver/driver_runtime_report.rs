use serde::Serialize;

/// 单个数据库产品的 JDBC Agent 就绪度诊断报告。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct DriverRuntimeReport {
    profile_id: String,
    catalog_status: String,
    agent_installed: bool,
    driver_installed: bool,
    java_available: bool,
    ready: bool,
    messages: Vec<String>,
}

impl DriverRuntimeReport {
    pub(crate) fn new(
        profile_id: String,
        catalog_status: String,
        agent_installed: bool,
        driver_installed: bool,
        java_available: bool,
        messages: Vec<String>,
    ) -> Self {
        Self {
            profile_id,
            catalog_status,
            agent_installed,
            driver_installed,
            java_available,
            ready: agent_installed && driver_installed && java_available,
            messages,
        }
    }

    /// 返回运行时是否具备启动条件；不代表数据库网络已连通。
    #[must_use]
    pub const fn ready(&self) -> bool {
        self.ready
    }
}
