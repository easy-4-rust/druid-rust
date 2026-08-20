//! Druid stable public facade.
//!
//! This crate re-exports the public API from `druid-core` so that downstream
//! consumers depend on `druid` (the stable name) while the implementation
//! lives in `druid-core`.
//!
//! # Feature-gated modules
//!
//! | Feature | Module | Upstream crate |
//! |---------|--------|----------------|
//! | `metrics` | [`metrics`] | `druid-metrics` |
//! | `wrapper` | [`wrapper`] | `druid-wrapper` |
//!
//! All features are disabled by default. Enable them in `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! druid = { version = "...", features = ["metrics", "wrapper"] }
//! ```

// --- Core re-exports (always available) ---
pub use druid_core::{core, dynamic, pool, spi, sql};

pub mod stats {
    pub use druid_core::stats::*;
}

// --- Optional: metrics runtime ---
#[cfg(feature = "metrics")]
#[cfg_attr(docsrs, doc(cfg(feature = "metrics")))]
pub use druid_metrics as metrics;

// --- Optional: driver wrappers and adapters ---
#[cfg(feature = "wrapper")]
#[cfg_attr(docsrs, doc(cfg(feature = "wrapper")))]
pub use druid_wrapper as wrapper;
