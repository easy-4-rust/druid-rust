//! `druid-core` — trait-only abstraction layer for `druid-rust`.
//!
//! **Status: design-stage.** This crate currently exposes no public types.
//! See [`../../druid-rust-Architecture.zh_CN.md`](../../druid-rust-Architecture.zh_CN.md)
//! §8 for the planned trait surface (`Connection`, `Driver`, `Pool`,
//! `BeforeFilter`, `AfterFilter`, `ConnectionFactory`).
//!
//! This crate must remain free of driver-, parser- and runtime-specific
//! dependencies. Any code that would otherwise pull in `sqlx`, `tokio-postgres`,
//! `deadpool`, `sqlparser` or a TLS backend belongs in an adapter crate, not
//! here.

#![doc = "druid-core: design-stage placeholder."]