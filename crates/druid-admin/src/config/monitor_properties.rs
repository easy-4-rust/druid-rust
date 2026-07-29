use serde::{Deserialize, Serialize};

/// 管理端监控配置。
///
/// 对应 Java: `com.alibaba.druid.admin.config.MonitorProperties`。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorProperties {
    /// 需要采集的应用名称。
    #[serde(default)]
    pub applications: Vec<String>,
    /// 登录用户名。
    pub login_username: Option<String>,
    /// 登录密码。
    pub login_password: Option<String>,
    /// 管理端上下文路径。
    pub context_path: Option<String>,
    /// Kubernetes kubeconfig 路径。
    pub kube_config_file_path: Option<String>,
    /// Kubernetes 命名空间。
    pub k8s_namespace: Option<String>,
}

impl MonitorProperties {
    /// 返回与 Java Servlet 注册逻辑一致的路由映射。
    ///
    /// # Errors
    ///
    /// 当非空路径不以 `/` 开始或以 `/` 结束时返回原 Java 错误消息。
    pub fn url_mapping(&self) -> Result<String, &'static str> {
        let context_path = self.context_path.as_deref().unwrap_or_default();
        if context_path.is_empty() {
            return Ok("/druid/*".to_owned());
        }
        if !context_path.starts_with('/') || context_path.ends_with('/') {
            return Err("Druid ContextPath must start with '/' and not end with '/'");
        }
        Ok(format!("{context_path}/*"))
    }
}
