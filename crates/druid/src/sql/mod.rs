//! 基于 sqlparser-rs 的 SQL 解析兼容层与 Wall 规则。
//!
//! 对应 Druid Java 的 `com.alibaba.druid.wall` 和 `com.alibaba.druid.sql` 包。
//! SQL 解析替换为 sqlparser-rs（ADR-002），Wall 规则基于 AST 检查。

pub mod char_types;
pub mod clickhouse;
pub mod db2;
pub mod db_type;
pub mod dialect_feature;
pub mod eof_parser_exception;
pub mod jdbc_utils;
pub mod keywords;
pub mod layout_characters;
pub mod lexer;
pub mod mysql;
pub mod not_allow_comment_exception;
pub mod oracle;
pub mod parser_exception;
pub mod postgresql;
pub mod sql_insert_value_handler;
pub mod sql_parse_exception;
pub mod sql_parser_feature;
pub mod sql_type;
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
pub use jdbc_utils::JdbcUtils;
pub use keywords::{Keywords, DEFAULT_KEYWORDS, DM_KEYWORDS, SQLITE_KEYWORDS};
pub use layout_characters::LayoutCharacters;
pub use lexer::{CommentHandler, Lexer, LexerError, LexerSavePoint};
pub use mysql::{MySqlWallProvider, MySqlWallVisitor};
pub use not_allow_comment_exception::NotAllowCommentException;
pub use oracle::{OracleWallProvider, OracleWallVisitor};
pub use parser_exception::ParserException;
pub use postgresql::{PgWallProvider, PgWallVisitor};
pub use sql_insert_value_handler::{
    SqlInsertFunctionValue, SqlInsertNumber, SqlInsertValueHandler,
};
#[allow(deprecated)]
pub use sql_parse_exception::SqlParseException;
pub use sql_parser_feature::SqlParserFeature;
pub use sql_type::SqlType;
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
