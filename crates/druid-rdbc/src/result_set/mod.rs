//! JDBC-like 结果集标准。

mod owned_result_set;

#[expect(
    clippy::module_inception,
    reason = "one standard object per file keeps mod.rs declaration-only"
)]
mod result_set;
mod result_set_ext;
mod result_set_read;
mod result_set_update;

pub(crate) use result_set::default_read_object;

pub use result_set::{
    default_object_as, ColumnIndex, ResultSet, ResultSetNavigation, ResultSetStreamKind,
};
pub use result_set_ext::ResultSetExt;
pub use result_set_read::ResultSetRead;
pub use result_set_update::ResultSetUpdate;
