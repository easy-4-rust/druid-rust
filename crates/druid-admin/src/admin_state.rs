//! 对应 Java 类：com.alibaba.druid.support.http.StatViewServlet（管理状态）

/// Admin 状态，传递给 axum handlers。
#[derive(Clone, Debug)]
pub struct AdminState {
    pub pool_name: String,
    pub driver_name: String,
}

impl AdminState {
    pub fn new(pool_name: impl Into<String>, driver_name: impl Into<String>) -> Self {
        Self { pool_name: pool_name.into(), driver_name: driver_name.into() }
    }
}
