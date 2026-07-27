//! PLANNED_BLOCKED: rbdc dependency deferred to V2. See ADR-001.
#![allow(dead_code)]

pub struct RbdcAdapter { _placeholder: () }
impl RbdcAdapter {
    pub fn new() -> Self { Self { _placeholder: () } }
}
impl Default for RbdcAdapter {
    fn default() -> Self { Self::new() }
}
