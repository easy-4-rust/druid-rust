//! Druid stable public facade.
//!
//! This crate re-exports the public API from `druid-core` so that downstream
//! consumers depend on `druid` (the stable name) while the implementation
//! lives in `druid-core`.

pub use druid_core::{core, dynamic, pool, spi, sql};

pub mod stats {
    pub use druid_core::stats::*;
}
