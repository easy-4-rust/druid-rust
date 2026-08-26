//! JDBC-like 结果集标准。

#[expect(
    clippy::module_inception,
    reason = "one standard object per file keeps mod.rs declaration-only"
)]
mod result_set;
mod result_set_ext;
mod result_set_update;

pub use result_set::{ColumnIndex, ResultSet, ResultSetNavigation, ResultSetStreamKind};
pub use result_set_ext::ResultSetExt;
pub use result_set_update::ResultSetUpdate;
