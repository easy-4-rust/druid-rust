//! PLANNED_BLOCKED: sqlx-deadpool dependency deferred to V2. See ADR-001.
#![allow(dead_code)]

pub struct SqlxDeadpoolAdapter { _placeholder: () }
impl SqlxDeadpoolAdapter {
    pub fn new() -> Self { Self { _placeholder: () } }
}
impl Default for SqlxDeadpoolAdapter {
    fn default() -> Self { Self::new() }
}
