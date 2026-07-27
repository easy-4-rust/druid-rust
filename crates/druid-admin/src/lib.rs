//! druid-rust admin: axum-based HTTP endpoints for datasource, SQL, wall and /metrics.
//!
//! **Status: design-stage placeholder.**
//! See druid-rust-Architecture.zh_CN.md §19 and doc/9、druid-rust-视觉与交互DNA规范.md.

/// Admin state passed to axum handlers.
#[derive(Clone)]
pub struct AdminState {
    /// Pool name
    pub pool_name: String,
    /// Driver name
    pub driver_name: String,
}

impl AdminState {
    pub fn new(pool_name: impl Into<String>, driver_name: impl Into<String>) -> Self {
        Self { pool_name: pool_name.into(), driver_name: driver_name.into() }
    }
}

/// Returns the planned endpoint list as a JSON string (for testing).
pub fn endpoint_list() -> &'static str {
    r#"["/druid/api/datasources","/druid/api/sql/top","/druid/api/sql/slow","/druid/api/wall","/druid/api/active","/metrics"]"#
}
