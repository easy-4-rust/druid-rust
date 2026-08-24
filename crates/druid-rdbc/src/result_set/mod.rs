//! JDBC-like 结果集标准。

#[expect(
    clippy::module_inception,
    reason = "one standard object per file keeps mod.rs declaration-only"
)]
mod result_set;

pub use result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
