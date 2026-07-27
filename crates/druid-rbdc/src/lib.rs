//! druid-rust rbdc adapter: bridges rbdc connection into druid-core traits.
//!
//! **Status: design-stage placeholder.** The rbdc dependency is deferred to V2.
//! See ADR-001 in druid-rust-Architecture.zh_CN.md.

/// rbdc connection adapter (planned for V2).
///
/// Will implement `druid_core::Connection` for `rbdc::db::Connection`.
pub struct RbdcAdapter {
    _placeholder: (),
}

impl RbdcAdapter {
    /// Create a new adapter placeholder.
    pub fn new() -> Self { Self { _placeholder: () } }
}

impl Default for RbdcAdapter {
    fn default() -> Self { Self::new() }
}
