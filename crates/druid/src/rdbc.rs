//! Rust facade for Java SQL and DataSource contracts, adapted as RDBC 4.2.
//!
//! Corresponds to Java 8 [`java.sql`](https://docs.oracle.com/javase/8/docs/api/java/sql/package-summary.html)
//! and [`javax.sql`](https://docs.oracle.com/javase/8/docs/api/javax/sql/package-summary.html).
//! It covers connection creation, SQL execution, tabular results, SQL type mappings,
//! user-defined types, metadata, and chained SQL exceptions.
//!
//! **Compatibility notice:** from 2026 Q3 the canonical public entry point is
//! `crate::sql::*`; this module is kept as a thin re-export for backward compatibility
//! and is marked `#[doc(hidden)]` + `#[deprecated]` at the module level in `lib.rs`.
//!
//! This module does not create a second pool. `Connection`, `Statement`, and `ResultSet`
//! delegate to Druid's pooled objects, while `PhysicalConnection` adapters handle database
//! differences. An optional API type does not certify support for a database product.

// This module is an intentionally unused crate-internal migration alias. Keeping the
// re-exports together prevents old internal paths from creating a second set of types.
#![allow(unused_imports)]

// ──────────────────── 直接从唯一真源 crate::sql:: 重新导出（不再通过 #[path] 私有挂载副本） ────────────────────
pub use crate::sql::array::{Array, RdbcArray};
pub use crate::sql::blob::{Blob, RdbcBlob};
pub use crate::sql::callable_statement::CallableStatement;
pub use crate::sql::client_info_status::ClientInfoStatus;
pub use crate::sql::clob::{Clob, RdbcClob};
pub use crate::sql::common_data_source::{CommonDataSource, RdbcLogWriter};
pub use crate::sql::connection::Connection;
pub use crate::sql::data_source::DataSource;
pub use crate::sql::database_meta_data::DatabaseMetaData;
pub use crate::sql::date::Date;
pub use crate::sql::driver::Driver;
pub use crate::sql::driver_action::DriverAction;
pub use crate::sql::driver_manager::DriverManager;
pub use crate::sql::driver_property_info::DriverPropertyInfo;
pub use crate::sql::exceptions::{
    BatchUpdateException, DataTruncation, SqlClientInfoException, SqlDataException, SqlException,
    SqlExceptionKind, SqlFeatureNotSupportedException, SqlIntegrityConstraintViolationException,
    SqlInvalidAuthorizationSpecException, SqlNonTransientConnectionException,
    SqlNonTransientException, SqlRecoverableException, SqlSyntaxErrorException,
    SqlTimeoutException, SqlTransactionRollbackException, SqlTransientConnectionException,
    SqlTransientException, SqlWarning,
};
pub use crate::sql::exceptions::{SqlException as SQLException, SqlWarning as SQLWarning};
pub use crate::sql::n_clob::{NClob, RdbcNClob};
pub use crate::sql::parameter_meta_data::{ParameterMetaData, ParameterMode, ParameterNullability};
pub use crate::sql::prepared_statement::PreparedStatement;
pub use crate::sql::pseudo_column_usage::PseudoColumnUsage;
pub use crate::sql::rdbc_sql_type::SqlType;
pub use crate::sql::rdbc_sql_type::SqlType as SQLType;
pub use crate::sql::rdbc_type::RdbcType;
pub use crate::sql::rdbc_type::RdbcType as RDBCType;
pub use crate::sql::rdbc_url::RdbcUrl;
pub use crate::sql::ref_value::{RdbcRef, Ref};
pub use crate::sql::result_set::ResultSet;
pub use crate::sql::result_set_meta_data::ResultSetMetaData;
pub use crate::sql::row_id::RowId;
pub use crate::sql::row_id_lifetime::RowIdLifetime;
pub use crate::sql::savepoint::Savepoint;
pub use crate::sql::sql_data::SqlData;
pub use crate::sql::sql_data::SqlData as SQLData;
pub use crate::sql::sql_input::SqlInput;
pub use crate::sql::sql_input::SqlInput as SQLInput;
pub use crate::sql::sql_output::SqlOutput;
pub use crate::sql::sql_output::SqlOutput as SQLOutput;
pub use crate::sql::sql_permission::SqlPermission;
pub use crate::sql::sql_xml::SqlXml as SQLXML;
pub use crate::sql::sql_xml::{RdbcSqlXml, SqlXml};
pub use crate::sql::statement::Statement;
pub use crate::sql::struct_value::Struct;
pub use crate::sql::time::Time;
pub use crate::sql::timestamp::Timestamp;
pub use crate::sql::types::Types;
pub use crate::sql::wrapper::{Unwrapped, Wrapper, WrapperExt};

// RDBC resource handles preserve Java SQL stream and UTF-16 value semantics while exposing
// Rust-native ownership and naming. They remain single physical resources; cloning a handle
// shares its cursor and closed state.
pub use crate::core::{
    RdbcCharacterLength, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcStreamLength,
    RdbcString, RdbcWriter,
};
