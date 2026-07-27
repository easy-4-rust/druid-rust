//! druid-rust sqlx + bb8 adapter: builds druid-pool on top of sqlx Pool via bb8 manager.
//!
//! **Status: design-stage placeholder.**
//! See ADR-001 in druid-rust-Architecture.zh_CN.md.

/// sqlx-bb8 connection adapter (planned for V2).
///
/// Will implement `druid_core::ConnectionFactory` using bb8's pool manager.
pub struct SqlxBb8Adapter {
    _placeholder: (),
}

impl SqlxBb8Adapter {
    pub fn new() -> Self { Self { _placeholder: () } }
}

impl Default for SqlxBb8Adapter {
    fn default() -> Self { Self::new() }
}
