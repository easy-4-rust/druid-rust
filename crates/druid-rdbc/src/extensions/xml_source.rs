use std::fmt;

/// Opaque XML source representation supplied by an adapter.
pub trait XmlSource: fmt::Debug + Send + Sync {}
