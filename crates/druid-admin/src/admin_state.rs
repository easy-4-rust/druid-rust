/// Axum 管理端暴露给处理器的最小展示状态。
///
/// Rust-only 对象：用于保留早期 `druid-admin` 门面 API；Java 管理端没有直接
/// 对应类，不能替代 `MonitorStatService` 或 `MonitorViewServlet`。
#[derive(Clone, Debug)]
pub struct AdminState {
    /// 连接池名称。
    pub pool_name: String,
    /// 数据库驱动名称。
    pub driver_name: String,
}

impl AdminState {
    /// 创建管理展示状态。
    ///
    /// `pool_name` 是连接池名称，`driver_name` 是驱动名称。
    #[must_use]
    pub fn new(pool_name: impl Into<String>, driver_name: impl Into<String>) -> Self {
        Self {
            pool_name: pool_name.into(),
            driver_name: driver_name.into(),
        }
    }
}
