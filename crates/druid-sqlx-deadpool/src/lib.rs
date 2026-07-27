//! druid-rust sqlx + deadpool adapter: builds druid-pool on top of sqlx Pool via deadpool manager.
//!
//! **Status: design-stage placeholder.**
//! See ADR-001 in druid-rust-Architecture.zh_CN.md.

/// sqlx-deadpool connection adapter (planned for V2).
///
/// Will implement `druid_core::ConnectionFactory` using deadpool's managed pool.
pub struct SqlxDeadpoolAdapter {
    _placeholder: (),
}

impl SqlxDeadpoolAdapter {
    pub fn new() -> Self { Self { _placeholder: () } }
}

impl Default for SqlxDeadpoolAdapter {
    fn default() -> Self { Self::new() }
}
