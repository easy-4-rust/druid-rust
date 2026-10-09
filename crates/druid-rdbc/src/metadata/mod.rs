//! JDBC-like live metadata 与 immutable snapshot 标准。

mod database_meta_data;
mod database_meta_data_info_ext;
mod database_meta_data_limits_ext;
mod database_meta_data_query_ext;
mod database_meta_data_request;
mod database_meta_data_sql_capabilities_ext;
mod parameter_meta_data;
mod pseudo_column_usage;
mod result_set_meta_data;

pub use database_meta_data::{DatabaseMetaData, TablesQuery};
pub use database_meta_data_info_ext::DatabaseMetaDataInfoExt;
pub use database_meta_data_limits_ext::DatabaseMetaDataLimitsExt;
pub use database_meta_data_query_ext::DatabaseMetaDataQueryExt;
pub use database_meta_data_request::{
    DatabaseMetaDataArgument, DatabaseMetaDataOperation, DatabaseMetaDataRequest,
};
pub use database_meta_data_sql_capabilities_ext::DatabaseMetaDataSqlCapabilitiesExt;
pub use parameter_meta_data::{
    ParameterDescriptor, ParameterMetaData, ParameterMode, ParameterNullability,
};
pub use pseudo_column_usage::PseudoColumnUsage;
pub use result_set_meta_data::{
    ColumnDescriptor, ColumnNullability, ResultSetColumnMeta, ResultSetColumnType,
    ResultSetMetaData, ResultSetMetaDataSnapshot, ResultSetNullability, RowIdLifetime,
};
