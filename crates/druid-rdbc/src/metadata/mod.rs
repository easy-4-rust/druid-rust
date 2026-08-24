//! JDBC-like live metadata 与 immutable snapshot 标准。

mod database_meta_data;
mod parameter_meta_data;
mod result_set_meta_data;

pub use database_meta_data::{DatabaseMetaData, TablesQuery};
pub use parameter_meta_data::{
    ParameterDescriptor, ParameterMetaData, ParameterMode, ParameterNullability,
};
pub use result_set_meta_data::{
    ColumnDescriptor, ColumnNullability, ResultSetColumnMeta, ResultSetColumnType,
    ResultSetMetaData, ResultSetMetaDataSnapshot, ResultSetNullability, RowIdLifetime,
};
