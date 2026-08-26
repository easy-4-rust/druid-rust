use std::fmt;

/// Opaque XML result representation supplied by an adapter.
pub trait XmlResult: fmt::Debug + Send + Sync {}
