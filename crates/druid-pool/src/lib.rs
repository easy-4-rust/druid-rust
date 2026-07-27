//! `druid-pool` — HikariCP-style async connection pool that consumes the
//! driver-agnostic traits defined in `druid-core`.
//!
//! **Status: design-stage.** The pool state machine, scheduler and idle queue
//! described in `druid-rust-Architecture.zh_CN.md` §9 and §11 are not yet
//! implemented. Adapters (`druid-rbdc`, `druid-sqlx-deadpool`, `druid-sqlx-bb8`)
//! will plug their own `ConnectionFactory` implementations into this pool.

#![doc = "druid-pool: design-stage placeholder."]