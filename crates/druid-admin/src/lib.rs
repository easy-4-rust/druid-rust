//! druid-admin: axum-based HTTP endpoints (design-stage placeholder).
#![allow(dead_code)]

pub mod admin_state;
pub use admin_state::AdminState;

pub fn endpoint_list() -> &'static str {
    r#"["/druid/api/datasources","/druid/api/sql/top","/druid/api/sql/slow","/druid/api/wall","/druid/api/active","/metrics"]"#
}
