//! 对标 Java `java.sql.*` + `javax.sql.*` 的 RDBC 标准层，
//! 同时承载 Druid SQL 解析与 Wall 防火墙。
//!
//! 模块划分严格对齐 Java 平台：
//! - `connection / `array` / `blob` / `clob` / `n_clob` / `sql_xml` —— LOB 与数组
//! - `statement` / `prepared_statement` / `callable_statement` —— 语句对象
//! - `result_set` / `result_set_meta_data` —— 结果集与元数据
//! - `database_meta_data` / `parameter_meta_data` —— 数据库/参数元数据
//! - `savepoint` / `row_id` / `row_id_lifetime` —— 事务定位子系统
//! - `types` / `sql_type` / `rdbc_type` / `rdbc_sql_type` —— 类型系统
//! - `date` / `time` / `timestamp` —— JDBC 时间值类型
//! - `driver` / `driver_manager` / `driver_action` / `driver_property_info` —— Driver 子系统
//! - `data_source` / `common_data_source` —— DataSource 门面
//! - `exceptions` —— SQLException 谱系
//! - `wrapper` —— Wrapper/unwrap 语义
//! - `sql_data` / `sql_input` / `sql_output` / `ref_value` / `struct_value` —— UDT 自定义类型
//! - `sql_permission` / `pseudo_column_usage` / `client_info_status` —— 辅助契约
//!
//! 同时本模块还承载 Druid 原生 SQL 解析（Lexer / DialectFeature / Token）与
//! 及 Wall 防火墙（`wall_config / wall_provider / 多方言 visitor）。

// ─────────────────────────────── java.sql.* 公共类型（39 个子模块） ───────────────────────────────

pub mod array;
pub mod blob;
pub mod callable_statement;
pub mod client_info_status;
pub mod clob;
pub mod common_data_source;
pub mod connection;
pub mod data_source;
pub mod database_meta_data;
pub mod date;
pub mod driver;
pub mod driver_action;
pub mod driver_manager;
pub mod driver_property_info;
pub mod exceptions;
pub mod n_clob;
pub mod parameter_meta_data;
pub mod prepared_statement;
pub mod pseudo_column_usage;
pub mod rdbc_sql_type;
pub mod rdbc_type;
pub mod rdbc_url;
pub mod ref_value;
pub mod result_set;
pub mod result_set_meta_data;
pub mod row_id;
pub mod row_id_lifetime;
pub mod savepoint;
pub mod sql_data;
pub mod sql_input;
pub mod sql_output;
pub mod sql_permission;
pub mod sql_type;
pub mod sql_xml;
pub mod statement;
pub mod struct_value;
pub mod time;
pub mod timestamp;
pub mod types;
pub mod wrapper;

pub use array::{Array, RdbcArray};
pub use blob::{Blob, RdbcBlob};
pub use callable_statement::CallableStatement;
pub use client_info_status::ClientInfoStatus;
pub use clob::{Clob, RdbcClob};
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
pub use n_clob::{NClob, RdbcNClob};
pub use parameter_meta_data::{ParameterMetaData, ParameterMode, ParameterNullability};
pub use prepared_statement::PreparedStatement;
pub use pseudo_column_usage::PseudoColumnUsage;
pub use rdbc_sql_type::SqlType as SQLType;
pub use rdbc_type::RdbcType;
pub use rdbc_type::RdbcType as RDBCType;
pub use rdbc_url::RdbcUrl;
pub use ref_value::{RdbcRef, Ref};
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
pub use sql_xml::SqlXml as SQLXML;
pub use sql_xml::{RdbcSqlXml, SqlXml};
pub use statement::Statement;
pub use struct_value::Struct;
pub use time::Time;
pub use timestamp::Timestamp;
pub use types::Types;
pub use wrapper::{Unwrapped, Wrapper, WrapperExt};

// java.sql LOB/stream helpers live in core because pooled statements also consume them,
// but their canonical public path is `druid::sql::*`.
pub use crate::core::{
    RdbcCharacterLength, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcStreamLength,
    RdbcString, RdbcWriter,
};

// ─────────────────────────────── Druid SQL 解析 + Wall 防火墙 ───────────────────────────────

pub mod char_types;
pub mod clickhouse;
pub mod db2;
pub mod db_type;
pub mod dialect_feature;
pub mod eof_parser_exception;
pub mod keywords;
pub mod layout_characters;
pub mod lexer;
pub mod mysql;
pub mod not_allow_comment_exception;
pub mod oracle;
pub mod parser_exception;
pub mod postgresql;
pub mod rdbc_utils;
pub mod sql_insert_value_handler;
pub mod sql_parse_exception;
pub mod sql_parser_feature;
pub mod sql_utils;
pub mod sqlite;
pub mod sqlserver;
pub mod symbol_table;
pub mod tenant_call_back;
pub mod token;
pub mod wall;
pub mod wall_check_result;
pub mod wall_config;
pub mod wall_context;
pub mod wall_deny_stat;
pub mod wall_filter;
pub mod wall_function_stat;
pub mod wall_function_stat_value;
pub mod wall_provider;
pub mod wall_provider_creator;
pub mod wall_provider_stat_value;
pub mod wall_sql_function_stat;
pub mod wall_sql_stat;
pub mod wall_sql_stat_value;
pub mod wall_sql_table_stat;
pub mod wall_table_stat;
pub mod wall_table_stat_value;
pub mod wall_update_check_handler;
pub mod wall_update_check_item;
pub mod wall_utils;
pub mod wall_violation;
pub mod wall_visitor;
pub mod wall_visitor_base;
pub mod wall_visitor_utils;

pub use char_types::CharTypes;
pub use clickhouse::{CkWallProvider, ClickhouseWallVisitor};
pub use db2::{Db2WallProvider, Db2WallVisitor};
pub use db_type::DbType;
pub use dialect_feature::{DialectFeature, DialectFeatureValue, LexerFeature, ParserFeature};
pub use eof_parser_exception::EofParserException;
pub use keywords::{Keywords, DEFAULT_KEYWORDS, DM_KEYWORDS, SQLITE_KEYWORDS};
pub use layout_characters::LayoutCharacters;
pub use lexer::{CommentHandler, Lexer, LexerError, LexerSavePoint};
pub use mysql::{MySqlWallProvider, MySqlWallVisitor};
pub use not_allow_comment_exception::NotAllowCommentException;
pub use oracle::{OracleWallProvider, OracleWallVisitor};
pub use parser_exception::ParserException;
pub use postgresql::{PgWallProvider, PgWallVisitor};
pub use rdbc_utils::RdbcUtils;
pub use sql_insert_value_handler::{
    SqlInsertFunctionValue, SqlInsertNumber, SqlInsertValueHandler,
};
#[allow(deprecated)]
pub use sql_parse_exception::SqlParseException;
pub use sql_parser_feature::SqlParserFeature;
pub use sql_utils::{SqlFormatOption, SqlUtils};
pub use sqlite::{SQLiteWallProvider, SQLiteWallVisitor};
pub use sqlserver::{SqlServerWallProvider, SqlServerWallVisitor};
pub use symbol_table::{SymbolTable, GLOBAL_SYMBOL_TABLE};
pub use tenant_call_back::{TenantCallBack, TenantStatementType};
pub use token::Token;
pub use wall::Wall;
pub use wall_check_result::WallCheckResult;
pub use wall_config::{WallConfig, WallConfigBuilder};
pub use wall_context::WallContext;
pub use wall_deny_stat::WallDenyStat;
pub use wall_filter::WallFilter;
pub use wall_function_stat::WallFunctionStat;
pub use wall_function_stat_value::WallFunctionStatValue;
pub use wall_provider::WallProvider;
pub use wall_provider_creator::{
    registered_wall_provider_creators, WallProviderCreator, WallProviderCreatorRegistration,
};
pub use wall_provider_stat_value::WallProviderStatValue;
pub use wall_sql_function_stat::WallSqlFunctionStat;
pub use wall_sql_stat::WallSqlStat;
pub use wall_sql_stat_value::WallSqlStatValue;
pub use wall_sql_table_stat::WallSqlTableStat;
pub use wall_table_stat::WallTableStat;
pub use wall_table_stat_value::WallTableStatValue;
pub use wall_update_check_handler::WallUpdateCheckHandler;
pub use wall_update_check_item::WallUpdateCheckItem;
pub use wall_utils::WallUtils;
pub use wall_violation::WallViolation;
pub use wall_visitor::WallVisitor;
pub use wall_visitor_base::WallVisitorBase;
pub use wall_visitor_utils::WallVisitorUtils;
