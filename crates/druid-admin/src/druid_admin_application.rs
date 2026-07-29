use topcoat::router::tower::TowerRoute;
use topcoat::router::{Methods, Path, PathBuf, Router, RouterBuilderDiscoverExt};

use crate::config::MonitorProperties;
use crate::servlet::monitor_view_servlet::MonitorViewServlet;

/// Druid 管理端应用装配入口。
///
/// 对应 Java: `com.alibaba.druid.admin.DruidAdminApplication`。Topcoat 提供
/// SSR 页面与资源外壳，Axum 路由作为 Tower service 挂载到同一 Tokio 服务。
pub struct DruidAdminApplication {
    properties: MonitorProperties,
    monitor_view_servlet: MonitorViewServlet,
}

impl DruidAdminApplication {
    /// 创建管理应用。
    #[must_use]
    pub fn new(properties: MonitorProperties, monitor_view_servlet: MonitorViewServlet) -> Self {
        let monitor_view_servlet = monitor_view_servlet
            .with_credentials(
                properties.login_username.clone(),
                properties.login_password.clone(),
            )
            .with_context_path(properties.context_path.clone());
        Self {
            properties,
            monitor_view_servlet,
        }
    }

    /// 返回 Java Servlet 注册规则对应的路由映射。
    ///
    /// # Errors
    ///
    /// 非法 context path 返回 Java 原错误。
    pub fn url_mapping(&self) -> Result<String, &'static str> {
        self.properties.url_mapping()
    }

    /// 构建 Topcoat + Axum 的统一路由。
    #[must_use]
    pub fn router(&self) -> Router {
        let context_path = self
            .properties
            .context_path
            .as_deref()
            .filter(|path| !path.is_empty())
            .unwrap_or("/druid");
        let catch_all = format!("{context_path}/{{*rest}}");
        let context_path = owned_path(context_path);
        let catch_all = owned_path(&catch_all);
        let axum = self.monitor_view_servlet.clone().router();
        Router::builder()
            .discover()
            .route(TowerRoute::new(Methods::Any, context_path, axum.clone()))
            .route(TowerRoute::new(Methods::Any, catch_all, axum.clone()))
            .route(TowerRoute::new(Methods::Any, Path::new("/metrics"), axum))
            .build()
    }

    /// 在 Tokio 上启动统一管理服务。
    ///
    /// # Errors
    ///
    /// 绑定监听地址或服务循环失败时返回 Topcoat 错误。
    pub async fn run(&self) -> std::io::Result<()> {
        topcoat::start(self.router()).await
    }
}

fn owned_path(path: &str) -> PathBuf {
    Path::new(path).segments().collect()
}
