//! Rust facade for Java SQL and DataSource contracts, adapted as RDBC 4.2.
//!
//! Corresponds to Java 8 [`java.sql`](https://docs.oracle.com/javase/8/docs/api/java/sql/package-summary.html)
//! and [`javax.sql`](https://docs.oracle.com/javase/8/docs/api/javax/sql/package-summary.html).
//! It covers connection creation, SQL execution, tabular results, SQL type mappings,
//! user-defined types, metadata, and chained SQL exceptions.
//!
//! This module does not create a second pool. `Connection`, `Statement`, and `ResultSet`
//! delegate to Druid's pooled objects, while `PhysicalConnection` adapters handle database
//! differences. An optional API type does not certify support for a database product.

#[path = "sql/array.rs"]
mod array;
#[path = "sql/blob.rs"]
mod blob;
#[path = "sql/callable_statement.rs"]
mod callable_statement;
#[path = "sql/client_info_status.rs"]
mod client_info_status;
#[path = "sql/clob.rs"]
mod clob;
#[path = "sql/common_data_source.rs"]
mod common_data_source;
#[path = "sql/connection.rs"]
mod connection;
#[path = "sql/data_source.rs"]
mod data_source;
#[path = "sql/database_meta_data.rs"]
mod database_meta_data;
#[path = "sql/date.rs"]
mod date;
#[path = "sql/driver.rs"]
mod driver;
#[path = "sql/driver_action.rs"]
mod driver_action;
#[path = "sql/driver_manager.rs"]
mod driver_manager;
#[path = "sql/driver_property_info.rs"]
mod driver_property_info;
#[path = "sql/exceptions.rs"]
mod exceptions;
#[path = "sql/n_clob.rs"]
mod n_clob;
#[path = "sql/parameter_meta_data.rs"]
mod parameter_meta_data;
#[path = "sql/prepared_statement.rs"]
mod prepared_statement;
#[path = "sql/pseudo_column_usage.rs"]
mod pseudo_column_usage;
#[path = "sql/rdbc_type.rs"]
mod rdbc_type;
#[path = "sql/rdbc_url.rs"]
mod rdbc_url;
#[path = "sql/ref_value.rs"]
mod ref_value;
#[path = "sql/result_set.rs"]
mod result_set;
#[path = "sql/result_set_meta_data.rs"]
mod result_set_meta_data;
#[path = "sql/row_id.rs"]
mod row_id;
#[path = "sql/row_id_lifetime.rs"]
mod row_id_lifetime;
#[path = "sql/savepoint.rs"]
mod savepoint;
#[path = "sql/sql_data.rs"]
mod sql_data;
#[path = "sql/sql_input.rs"]
mod sql_input;
#[path = "sql/sql_output.rs"]
mod sql_output;
#[path = "sql/sql_permission.rs"]
mod sql_permission;
#[path = "sql/rdbc_sql_type.rs"]
mod sql_type;
#[path = "sql/sql_xml.rs"]
mod sql_xml;
#[path = "sql/statement.rs"]
mod statement;
#[path = "sql/struct_value.rs"]
mod struct_value;
#[path = "sql/time.rs"]
mod time;
#[path = "sql/timestamp.rs"]
mod timestamp;
#[path = "sql/types.rs"]
mod types;
#[path = "sql/wrapper.rs"]
mod wrapper;

pub use array::Array;
pub use blob::Blob;
pub use callable_statement::CallableStatement;
pub use client_info_status::ClientInfoStatus;
pub use clob::Clob;
pub use common_data_source::{CommonDataSource, RdbcLogWriter};
pub use connection::Connection;
pub use data_source::DataSource;
pub use database_meta_data::DatabaseMetaData;
pub use date::Date;
pub use driver::Driver;
pub use driver_action::DriverAction;
pub use driver_manager::DriverManager;
pub use driver_property_info::DriverPropertyInfo;
pub use exceptions::{
    BatchUpdateException, DataTruncation, SqlClientInfoException, SqlDataException, SqlException,
    SqlExceptionKind, SqlFeatureNotSupportedException, SqlIntegrityConstraintViolationException,
    SqlInvalidAuthorizationSpecException, SqlNonTransientConnectionException,
    SqlNonTransientException, SqlRecoverableException, SqlSyntaxErrorException,
    SqlTimeoutException, SqlTransactionRollbackException, SqlTransientConnectionException,
    SqlTransientException, SqlWarning,
};
pub use exceptions::{SqlException as SQLException, SqlWarning as SQLWarning};
pub use n_clob::NClob;
pub use parameter_meta_data::{ParameterMetaData, ParameterMode, ParameterNullability};
pub use prepared_statement::PreparedStatement;
pub use pseudo_column_usage::PseudoColumnUsage;
pub use rdbc_type::RdbcType;
pub use rdbc_type::RdbcType as RDBCType;
pub use rdbc_url::RdbcUrl;
pub use ref_value::Ref;
pub use result_set::ResultSet;
pub use result_set_meta_data::ResultSetMetaData;
pub use row_id::RowId;
pub use row_id_lifetime::RowIdLifetime;
pub use savepoint::Savepoint;
pub use sql_data::SqlData;
pub use sql_data::SqlData as SQLData;
pub use sql_input::SqlInput;
pub use sql_input::SqlInput as SQLInput;
pub use sql_output::SqlOutput;
pub use sql_output::SqlOutput as SQLOutput;
pub use sql_permission::SqlPermission;
pub use sql_type::SqlType;
pub use sql_type::SqlType as SQLType;
pub use sql_xml::SqlXml;
pub use sql_xml::SqlXml as SQLXML;
pub use statement::Statement;
pub use struct_value::Struct;
pub use time::Time;
pub use timestamp::Timestamp;
pub use types::Types;
pub use wrapper::{Unwrapped, Wrapper, WrapperExt};

// RDBC resource adapters preserve Java SQL stream and UTF-16 value semantics while exposing
// Rust-native ownership and naming. They remain single physical resources; cloning a handle
// shares its cursor and closed state.
pub use crate::core::{
    RdbcCharacterLength, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcStreamLength,
    RdbcString, RdbcWriter,
};
