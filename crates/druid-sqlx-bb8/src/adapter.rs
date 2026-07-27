//! PLANNED_BLOCKED: sqlx-bb8 dependency deferred to V2. See ADR-001.
#![allow(dead_code)]

pub struct SqlxBb8Adapter { _placeholder: () }
impl SqlxBb8Adapter {
    pub fn new() -> Self { Self { _placeholder: () } }
}
impl Default for SqlxBb8Adapter {
    fn default() -> Self { Self::new() }
}
