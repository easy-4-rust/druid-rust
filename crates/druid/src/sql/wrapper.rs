/// Type inspection and delegate unwrapping for RDBC proxy objects.
///
/// Corresponds to Java: `java.sql.Wrapper`. `is_wrapper_for` checks whether the requested
/// implementation can be obtained, and `unwrap` returns it or a database access error.
pub use crate::core::{Unwrapped, Wrapper, WrapperExt};
