//! druid-rust admin: axum-based HTTP endpoints.
//!
//! **Status: design-stage placeholder.**
//! See druid-rust-Architecture.zh_CN.md §19.

pub mod admin_state;

pub use admin_state::AdminState;

/// Returns the planned endpoint list as a JSON string.
pub fn endpoint_list() -> &'static str {
    r#"["/druid/api/datasources","/druid/api/sql/top","/druid/api/sql/slow","/druid/api/wall","/druid/api/active","/metrics"]"#
}
