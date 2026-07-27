//! PLANNED_BLOCKED: axum dependency deferred to V3. See ADR-007.
#![allow(dead_code)]

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
