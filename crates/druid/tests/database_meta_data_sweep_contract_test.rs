//! `java.sql.DatabaseMetaData` 未评估方法的表驱动扫描契约测试。
//!
//! 数据来源：`docs/verification/rdbc-first/java8-method-evidence.tsv`，
//! 过滤条件为 `owner=java.sql.DatabaseMetaData` 且 `capability=UNASSESSED`。
//! 台账中该 owner 共有 178 个 UNASSESSED identity，其中 61 个是 `constant`、
//! 1 个是 `type`、116 个是方法（含 2 个 `java.sql.Wrapper` 继承方法）。
//! 常量与类型不是可调用方法，因此本测试只扫描这 116 个方法 identity。
//!
//! 每个方法只调用一次，把真实观察结果写入共享 `Vec<Observation>`：
//! - `Ok(value)` 记录为 `value`，并保留真实返回值；
//! - `Err(RdbcError::UnsupportedOperation { operation })` 记录为 `unsupported`，
//!   并保留精确的 operation 字符串；
//! - 其他错误记录为 `other-error`，保留 `Debug` 文本。
//!
//! 在 Rust 标准层没有可调用入口的方法记录为 `no-entry`，不做伪造调用。
//!
//! 对应 Java：`java.sql.DatabaseMetaData`（Java 8 公共方法台账）。

#![allow(clippy::too_many_lines)]

use druid::core::{
    Connection, ConnectionFactory, DruidConnectionHolder, DruidPooledConnection, RdbcError,
    SqlExceptionKind,
};
use druid_rdbc::metadata::{
    DatabaseMetaData, DatabaseMetaDataInfoExt, DatabaseMetaDataLimitsExt, DatabaseMetaDataQueryExt,
    DatabaseMetaDataSqlCapabilitiesExt,
};
use druid_rdbc::wrapper::StandardUnwrapped;
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::any::TypeId;
use std::time::Duration;

/// 建立一个连接真实 SQLite 内存库的 pooled connection。
async fn make_pooled() -> DruidPooledConnection {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory");
    let physical = factory.create().await.expect("connection");
    {
        let holder =
            DruidConnectionHolder::with_standard_connection(physical, 1, Duration::ZERO, 0)
                .await
                .expect("canonical holder");
        DruidPooledConnection::with_standard_holder(
            holder,
            "metadata-sweep".to_owned(),
            None,
            false,
            false,
            Box::new(|_, _| false),
        )
    }
}

/// 观察分类。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObservationKind {
    /// 调用成功，返回真实值。
    Value,
    /// 返回 `RdbcError::UnsupportedOperation`。
    Unsupported,
    /// 返回其他错误。
    OtherError,
    /// 该 identity 在 Rust 标准层没有可调用入口。
    NoEntry,
}

impl ObservationKind {
    /// 返回报告使用的稳定字符串。
    const fn as_str(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::Unsupported => "unsupported",
            Self::OtherError => "other-error",
            Self::NoEntry => "no-entry",
        }
    }
}

/// 单个 identity 的真实观察。
#[derive(Clone, Debug)]
struct Observation {
    /// 台账 identity，逐字一致。
    identity: &'static str,
    /// Rust 类型入口。
    rust_type_entry: &'static str,
    /// Rust 方法入口。
    rust_method_entry: &'static str,
    /// Rust 源文件。
    rust_source: &'static str,
    /// 观察备注。
    notes: &'static str,
    /// 观察分类。
    kind: ObservationKind,
    /// 观察细节：真实值、operation 字符串或错误 Debug。
    detail: String,
}

/// 负向观察的契约断言。
///
/// `RdbcError::UnsupportedOperation` 变体本身没有 SQLState，因此这里只断言它映射到
/// Java `UnsupportedOperationException` 类名；只有真正返回 `SqlException` 时才断言
/// `SQLFeatureNotSupported` 的 class 与 SQLState，不臆造不存在的 SQLState。
fn assert_negative_contract(error: &RdbcError) {
    match error {
        RdbcError::UnsupportedOperation { operation } => {
            assert!(
                !operation.trim().is_empty(),
                "unsupported observation must keep a non-empty operation name"
            );
            assert_eq!(
                error.class_name(),
                "java.lang.UnsupportedOperationException",
                "unsupported observation must map to the Java default-method exception class"
            );
        }
        RdbcError::SqlException(exception) => {
            if exception.kind() == SqlExceptionKind::FeatureNotSupported {
                assert_eq!(
                    exception.class_name(),
                    "java.sql.SQLFeatureNotSupportedException",
                    "SQLFeatureNotSupported observation must keep the Java exception class"
                );
                assert_eq!(
                    exception.sql_state(),
                    Some("0A000"),
                    "SQLFeatureNotSupported observation must keep SQLState 0A000"
                );
            }
        }
        _ => {}
    }
}

/// 归类错误并保留细节文本。
fn observe_error(error: RdbcError) -> (ObservationKind, String) {
    match &error {
        RdbcError::UnsupportedOperation { .. } | RdbcError::SqlException(_) => {
            assert_negative_contract(&error);
        }
        _ => {}
    }
    match error {
        RdbcError::UnsupportedOperation { operation } => (
            ObservationKind::Unsupported,
            format!("RdbcError::UnsupportedOperation {{ operation: \"{operation}\" }}"),
        ),
        other => (ObservationKind::OtherError, format!("{other:?}")),
    }
}

/// `Wrapper::unwrap` 返回的标准视图名称。
fn wrapper_view_name(view: &StandardUnwrapped<'_>) -> &'static str {
    match view {
        StandardUnwrapped::Connection(_) => "Connection",
        StandardUnwrapped::Statement(_) => "Statement",
        StandardUnwrapped::PreparedStatement(_) => "PreparedStatement",
        StandardUnwrapped::CallableStatement(_) => "CallableStatement",
        StandardUnwrapped::ResultSet(_) => "ResultSet",
        StandardUnwrapped::RowSet(_) => "RowSet",
        StandardUnwrapped::DatabaseMetaData(_) => "DatabaseMetaData",
        StandardUnwrapped::ResultSetMetaData(_) => "ResultSetMetaData",
        StandardUnwrapped::RowSetMetaData(_) => "RowSetMetaData",
        StandardUnwrapped::ParameterMetaData(_) => "ParameterMetaData",
        StandardUnwrapped::CommonDataSource(_) => "CommonDataSource",
        StandardUnwrapped::DataSource(_) => "DataSource",
        StandardUnwrapped::PooledConnection(_) => "PooledConnection",
        StandardUnwrapped::ConnectionPoolDataSource(_) => "ConnectionPoolDataSource",
        StandardUnwrapped::XaConnection(_) => "XaConnection",
        StandardUnwrapped::XaDataSource(_) => "XaDataSource",
        StandardUnwrapped::Driver(_) => "Driver",
        StandardUnwrapped::ConnectionFactory(_) => "ConnectionFactory",
        StandardUnwrapped::Wrapper(_) => "Wrapper",
        _ => "Unknown",
    }
}

/// 观察一个返回布尔值的元数据方法。
macro_rules! observe_bool {
    ($meta:ident, $method:ident) => {
        match $meta.$method().await {
            Ok(value) => (ObservationKind::Value, value.to_string()),
            Err(error) => observe_error(error),
        }
    };
    ($meta:ident, $method:ident ( $($arg:expr),+ $(,)? )) => {
        match $meta.$method($($arg),+).await {
            Ok(value) => (ObservationKind::Value, value.to_string()),
            Err(error) => observe_error(error),
        }
    };
}

/// 观察一个返回可空字符串的元数据方法。
macro_rules! observe_string {
    ($meta:ident, $method:ident) => {
        match $meta.$method().await {
            Ok(value) => (ObservationKind::Value, format!("{value:?}")),
            Err(error) => observe_error(error),
        }
    };
}

/// 观察一个返回整数的元数据方法。
macro_rules! observe_integer {
    ($meta:ident, $method:ident) => {
        match $meta.$method().await {
            Ok(value) => (ObservationKind::Value, value.to_string()),
            Err(error) => observe_error(error),
        }
    };
}

/// 观察一个返回行集的元数据方法。
macro_rules! observe_result_set {
    ($meta:ident, $method:ident) => {
        match $meta.$method().await {
            Ok(result_set) => {
                let columns = result_set
                    .meta_data()
                    .map(|meta_data| meta_data.column_count())
                    .unwrap_or_default();
                (ObservationKind::Value, format!("result_set(columns={columns})"))
            }
            Err(error) => observe_error(error),
        }
    };
    ($meta:ident, $method:ident ( $($arg:expr),+ $(,)? )) => {
        match $meta.$method($($arg),+).await {
            Ok(result_set) => {
                let columns = result_set
                    .meta_data()
                    .map(|meta_data| meta_data.column_count())
                    .unwrap_or_default();
                (ObservationKind::Value, format!("result_set(columns={columns})"))
            }
            Err(error) => observe_error(error),
        }
    };
}

/// 观察 `java.sql.Wrapper#isWrapperFor`。
macro_rules! observe_wrapper_flag {
    ($meta:ident) => {
        match $meta.is_wrapper_for(Some(TypeId::of::<dyn DatabaseMetaData>())) {
            Ok(value) => (ObservationKind::Value, value.to_string()),
            Err(error) => observe_error(error),
        }
    };
}

/// 观察 `java.sql.Wrapper#unwrap`。
macro_rules! observe_wrapper_unwrap {
    ($meta:ident) => {
        match $meta.unwrap(Some(TypeId::of::<dyn DatabaseMetaData>())) {
            Ok(view) => (
                ObservationKind::Value,
                format!("unwrap({})", wrapper_view_name(&view)),
            ),
            Err(error) => observe_error(error),
        }
    };
}

/// 一个被扫描的 `java.sql.DatabaseMetaData` identity 与其 Rust 标准入口。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SweepRow {
    /// 台账 identity，逐字取自 `java8-method-evidence.tsv`。
    identity: &'static str,
    /// Rust 类型入口；没有 Rust 入口时为空字符串。
    rust_type_entry: &'static str,
    /// Rust 方法入口；没有 Rust 入口时为空字符串。
    rust_method_entry: &'static str,
    /// Rust 源文件；没有 Rust 入口时为空字符串。
    rust_source: &'static str,
    /// 观察备注。
    notes: &'static str,
    /// 调用分派标识。
    id: SweepId,
}

/// 被扫描方法的调用分派标识；每个变体对应台账中的一个方法 identity。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SweepId {
    /// `java.sql.DatabaseMetaData#isWrapperFor(java.lang.Class)@java.sql.Wrapper`
    IsWrapperFor,
    /// `java.sql.DatabaseMetaData#unwrap(java.lang.Class)@java.sql.Wrapper`
    Unwrap,
    /// `java.sql.DatabaseMetaData#dataDefinitionCausesTransactionCommit()`
    DataDefinitionCausesTransactionCommit,
    /// `java.sql.DatabaseMetaData#dataDefinitionIgnoredInTransactions()`
    DataDefinitionIgnoredInTransactions,
    /// `java.sql.DatabaseMetaData#deletesAreDetected(int)`
    DeletesAreDetected,
    /// `java.sql.DatabaseMetaData#doesMaxRowSizeIncludeBlobs()`
    DoesMaxRowSizeIncludeBlobs,
    /// `java.sql.DatabaseMetaData#getAttributes(java.lang.String,java.lang.String,java.lang.String,java.lang.String)`
    GetAttributes,
    /// `java.sql.DatabaseMetaData#getBestRowIdentifier(java.lang.String,java.lang.String,java.lang.String,int,boolean)`
    GetBestRowIdentifier,
    /// `java.sql.DatabaseMetaData#getColumnPrivileges(java.lang.String,java.lang.String,java.lang.String,java.lang.String)`
    GetColumnPrivileges,
    /// `java.sql.DatabaseMetaData#getConnection()`
    GetConnection,
    /// `java.sql.DatabaseMetaData#getDatabaseMinorVersion()`
    GetDatabaseMinorVersion,
    /// `java.sql.DatabaseMetaData#getDriverMinorVersion()`
    GetDriverMinorVersion,
    /// `java.sql.DatabaseMetaData#getExtraNameCharacters()`
    GetExtraNameCharacters,
    /// `java.sql.DatabaseMetaData#getFunctionColumns(java.lang.String,java.lang.String,java.lang.String,java.lang.String)`
    GetFunctionColumns,
    /// `java.sql.DatabaseMetaData#getJDBCMajorVersion()`
    GetJDBCMajorVersion,
    /// `java.sql.DatabaseMetaData#getJDBCMinorVersion()`
    GetJDBCMinorVersion,
    /// `java.sql.DatabaseMetaData#getMaxBinaryLiteralLength()`
    GetMaxBinaryLiteralLength,
    /// `java.sql.DatabaseMetaData#getMaxCatalogNameLength()`
    GetMaxCatalogNameLength,
    /// `java.sql.DatabaseMetaData#getMaxCharLiteralLength()`
    GetMaxCharLiteralLength,
    /// `java.sql.DatabaseMetaData#getMaxColumnNameLength()`
    GetMaxColumnNameLength,
    /// `java.sql.DatabaseMetaData#getMaxColumnsInGroupBy()`
    GetMaxColumnsInGroupBy,
    /// `java.sql.DatabaseMetaData#getMaxColumnsInIndex()`
    GetMaxColumnsInIndex,
    /// `java.sql.DatabaseMetaData#getMaxColumnsInOrderBy()`
    GetMaxColumnsInOrderBy,
    /// `java.sql.DatabaseMetaData#getMaxCursorNameLength()`
    GetMaxCursorNameLength,
    /// `java.sql.DatabaseMetaData#getMaxIndexLength()`
    GetMaxIndexLength,
    /// `java.sql.DatabaseMetaData#getMaxLogicalLobSize()`
    GetMaxLogicalLobSize,
    /// `java.sql.DatabaseMetaData#getMaxProcedureNameLength()`
    GetMaxProcedureNameLength,
    /// `java.sql.DatabaseMetaData#getMaxRowSize()`
    GetMaxRowSize,
    /// `java.sql.DatabaseMetaData#getMaxSchemaNameLength()`
    GetMaxSchemaNameLength,
    /// `java.sql.DatabaseMetaData#getMaxStatements()`
    GetMaxStatements,
    /// `java.sql.DatabaseMetaData#getMaxTableNameLength()`
    GetMaxTableNameLength,
    /// `java.sql.DatabaseMetaData#getMaxTablesInSelect()`
    GetMaxTablesInSelect,
    /// `java.sql.DatabaseMetaData#getMaxUserNameLength()`
    GetMaxUserNameLength,
    /// `java.sql.DatabaseMetaData#getProcedureColumns(java.lang.String,java.lang.String,java.lang.String,java.lang.String)`
    GetProcedureColumns,
    /// `java.sql.DatabaseMetaData#getPseudoColumns(java.lang.String,java.lang.String,java.lang.String,java.lang.String)`
    GetPseudoColumns,
    /// `java.sql.DatabaseMetaData#getSchemas()`
    GetSchemas,
    /// `java.sql.DatabaseMetaData#getSchemas(java.lang.String,java.lang.String)`
    GetSchemasWithPattern,
    /// `java.sql.DatabaseMetaData#getStringFunctions()`
    GetStringFunctions,
    /// `java.sql.DatabaseMetaData#getSuperTables(java.lang.String,java.lang.String,java.lang.String)`
    GetSuperTables,
    /// `java.sql.DatabaseMetaData#getSuperTypes(java.lang.String,java.lang.String,java.lang.String)`
    GetSuperTypes,
    /// `java.sql.DatabaseMetaData#getSystemFunctions()`
    GetSystemFunctions,
    /// `java.sql.DatabaseMetaData#getTablePrivileges(java.lang.String,java.lang.String,java.lang.String)`
    GetTablePrivileges,
    /// `java.sql.DatabaseMetaData#getTimeDateFunctions()`
    GetTimeDateFunctions,
    /// `java.sql.DatabaseMetaData#getUDTs(java.lang.String,java.lang.String,java.lang.String,int[])`
    GetUDTs,
    /// `java.sql.DatabaseMetaData#getVersionColumns(java.lang.String,java.lang.String,java.lang.String)`
    GetVersionColumns,
    /// `java.sql.DatabaseMetaData#insertsAreDetected(int)`
    InsertsAreDetected,
    /// `java.sql.DatabaseMetaData#isCatalogAtStart()`
    IsCatalogAtStart,
    /// `java.sql.DatabaseMetaData#locatorsUpdateCopy()`
    LocatorsUpdateCopy,
    /// `java.sql.DatabaseMetaData#nullPlusNonNullIsNull()`
    NullPlusNonNullIsNull,
    /// `java.sql.DatabaseMetaData#nullsAreSortedAtEnd()`
    NullsAreSortedAtEnd,
    /// `java.sql.DatabaseMetaData#nullsAreSortedAtStart()`
    NullsAreSortedAtStart,
    /// `java.sql.DatabaseMetaData#nullsAreSortedLow()`
    NullsAreSortedLow,
    /// `java.sql.DatabaseMetaData#othersDeletesAreVisible(int)`
    OthersDeletesAreVisible,
    /// `java.sql.DatabaseMetaData#othersInsertsAreVisible(int)`
    OthersInsertsAreVisible,
    /// `java.sql.DatabaseMetaData#othersUpdatesAreVisible(int)`
    OthersUpdatesAreVisible,
    /// `java.sql.DatabaseMetaData#ownDeletesAreVisible(int)`
    OwnDeletesAreVisible,
    /// `java.sql.DatabaseMetaData#ownInsertsAreVisible(int)`
    OwnInsertsAreVisible,
    /// `java.sql.DatabaseMetaData#ownUpdatesAreVisible(int)`
    OwnUpdatesAreVisible,
    /// `java.sql.DatabaseMetaData#storesLowerCaseIdentifiers()`
    StoresLowerCaseIdentifiers,
    /// `java.sql.DatabaseMetaData#storesLowerCaseQuotedIdentifiers()`
    StoresLowerCaseQuotedIdentifiers,
    /// `java.sql.DatabaseMetaData#storesMixedCaseIdentifiers()`
    StoresMixedCaseIdentifiers,
    /// `java.sql.DatabaseMetaData#storesMixedCaseQuotedIdentifiers()`
    StoresMixedCaseQuotedIdentifiers,
    /// `java.sql.DatabaseMetaData#storesUpperCaseIdentifiers()`
    StoresUpperCaseIdentifiers,
    /// `java.sql.DatabaseMetaData#storesUpperCaseQuotedIdentifiers()`
    StoresUpperCaseQuotedIdentifiers,
    /// `java.sql.DatabaseMetaData#supportsANSI92EntryLevelSQL()`
    SupportsANSI92EntryLevelSQL,
    /// `java.sql.DatabaseMetaData#supportsANSI92FullSQL()`
    SupportsANSI92FullSQL,
    /// `java.sql.DatabaseMetaData#supportsANSI92IntermediateSQL()`
    SupportsANSI92IntermediateSQL,
    /// `java.sql.DatabaseMetaData#supportsAlterTableWithAddColumn()`
    SupportsAlterTableWithAddColumn,
    /// `java.sql.DatabaseMetaData#supportsAlterTableWithDropColumn()`
    SupportsAlterTableWithDropColumn,
    /// `java.sql.DatabaseMetaData#supportsCatalogsInIndexDefinitions()`
    SupportsCatalogsInIndexDefinitions,
    /// `java.sql.DatabaseMetaData#supportsCatalogsInPrivilegeDefinitions()`
    SupportsCatalogsInPrivilegeDefinitions,
    /// `java.sql.DatabaseMetaData#supportsCatalogsInProcedureCalls()`
    SupportsCatalogsInProcedureCalls,
    /// `java.sql.DatabaseMetaData#supportsCatalogsInTableDefinitions()`
    SupportsCatalogsInTableDefinitions,
    /// `java.sql.DatabaseMetaData#supportsConvert()`
    SupportsConvert,
    /// `java.sql.DatabaseMetaData#supportsConvert(int,int)`
    SupportsConvertBetween,
    /// `java.sql.DatabaseMetaData#supportsCoreSQLGrammar()`
    SupportsCoreSQLGrammar,
    /// `java.sql.DatabaseMetaData#supportsCorrelatedSubqueries()`
    SupportsCorrelatedSubqueries,
    /// `java.sql.DatabaseMetaData#supportsDataDefinitionAndDataManipulationTransactions()`
    SupportsDataDefinitionAndDataManipulationTransactions,
    /// `java.sql.DatabaseMetaData#supportsDataManipulationTransactionsOnly()`
    SupportsDataManipulationTransactionsOnly,
    /// `java.sql.DatabaseMetaData#supportsDifferentTableCorrelationNames()`
    SupportsDifferentTableCorrelationNames,
    /// `java.sql.DatabaseMetaData#supportsExtendedSQLGrammar()`
    SupportsExtendedSQLGrammar,
    /// `java.sql.DatabaseMetaData#supportsFullOuterJoins()`
    SupportsFullOuterJoins,
    /// `java.sql.DatabaseMetaData#supportsGetGeneratedKeys()`
    SupportsGetGeneratedKeys,
    /// `java.sql.DatabaseMetaData#supportsGroupByBeyondSelect()`
    SupportsGroupByBeyondSelect,
    /// `java.sql.DatabaseMetaData#supportsGroupByUnrelated()`
    SupportsGroupByUnrelated,
    /// `java.sql.DatabaseMetaData#supportsIntegrityEnhancementFacility()`
    SupportsIntegrityEnhancementFacility,
    /// `java.sql.DatabaseMetaData#supportsLimitedOuterJoins()`
    SupportsLimitedOuterJoins,
    /// `java.sql.DatabaseMetaData#supportsMinimumSQLGrammar()`
    SupportsMinimumSQLGrammar,
    /// `java.sql.DatabaseMetaData#supportsMixedCaseIdentifiers()`
    SupportsMixedCaseIdentifiers,
    /// `java.sql.DatabaseMetaData#supportsMixedCaseQuotedIdentifiers()`
    SupportsMixedCaseQuotedIdentifiers,
    /// `java.sql.DatabaseMetaData#supportsMultipleOpenResults()`
    SupportsMultipleOpenResults,
    /// `java.sql.DatabaseMetaData#supportsMultipleResultSets()`
    SupportsMultipleResultSets,
    /// `java.sql.DatabaseMetaData#supportsMultipleTransactions()`
    SupportsMultipleTransactions,
    /// `java.sql.DatabaseMetaData#supportsNonNullableColumns()`
    SupportsNonNullableColumns,
    /// `java.sql.DatabaseMetaData#supportsOpenCursorsAcrossCommit()`
    SupportsOpenCursorsAcrossCommit,
    /// `java.sql.DatabaseMetaData#supportsOpenCursorsAcrossRollback()`
    SupportsOpenCursorsAcrossRollback,
    /// `java.sql.DatabaseMetaData#supportsOpenStatementsAcrossCommit()`
    SupportsOpenStatementsAcrossCommit,
    /// `java.sql.DatabaseMetaData#supportsOpenStatementsAcrossRollback()`
    SupportsOpenStatementsAcrossRollback,
    /// `java.sql.DatabaseMetaData#supportsOrderByUnrelated()`
    SupportsOrderByUnrelated,
    /// `java.sql.DatabaseMetaData#supportsPositionedDelete()`
    SupportsPositionedDelete,
    /// `java.sql.DatabaseMetaData#supportsPositionedUpdate()`
    SupportsPositionedUpdate,
    /// `java.sql.DatabaseMetaData#supportsRefCursors()`
    SupportsRefCursors,
    /// `java.sql.DatabaseMetaData#supportsResultSetHoldability(int)`
    SupportsResultSetHoldability,
    /// `java.sql.DatabaseMetaData#supportsSchemasInIndexDefinitions()`
    SupportsSchemasInIndexDefinitions,
    /// `java.sql.DatabaseMetaData#supportsSchemasInPrivilegeDefinitions()`
    SupportsSchemasInPrivilegeDefinitions,
    /// `java.sql.DatabaseMetaData#supportsSchemasInProcedureCalls()`
    SupportsSchemasInProcedureCalls,
    /// `java.sql.DatabaseMetaData#supportsSchemasInTableDefinitions()`
    SupportsSchemasInTableDefinitions,
    /// `java.sql.DatabaseMetaData#supportsStoredProcedures()`
    SupportsStoredProcedures,
    /// `java.sql.DatabaseMetaData#supportsSubqueriesInExists()`
    SupportsSubqueriesInExists,
    /// `java.sql.DatabaseMetaData#supportsSubqueriesInIns()`
    SupportsSubqueriesInIns,
    /// `java.sql.DatabaseMetaData#supportsSubqueriesInQuantifieds()`
    SupportsSubqueriesInQuantifieds,
    /// `java.sql.DatabaseMetaData#supportsTableCorrelationNames()`
    SupportsTableCorrelationNames,
    /// `java.sql.DatabaseMetaData#supportsUnionAll()`
    SupportsUnionAll,
    /// `java.sql.DatabaseMetaData#updatesAreDetected(int)`
    UpdatesAreDetected,
    /// `java.sql.DatabaseMetaData#usesLocalFilePerTable()`
    UsesLocalFilePerTable,
    /// `java.sql.DatabaseMetaData#usesLocalFiles()`
    UsesLocalFiles,
}

/// 扫描表；覆盖台账中 owner=java.sql.DatabaseMetaData 的全部 UNASSESSED 方法。
const SWEEP_TABLE: &[SweepRow] = &[
    SweepRow {
        identity: "java.sql.DatabaseMetaData#isWrapperFor(java.lang.Class)@java.sql.Wrapper",
        rust_type_entry: "druid_rdbc::wrapper::Wrapper",
        rust_method_entry: "druid_rdbc::wrapper::Wrapper::is_wrapper_for",
        rust_source: "crates/druid-rdbc/src/wrapper/wrapper.rs",
        notes: "继承自 java.sql.Wrapper；Rust 标准层用 Wrapper::is_wrapper_for(TypeId) 判定目标接口",
        id: SweepId::IsWrapperFor,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#unwrap(java.lang.Class)@java.sql.Wrapper",
        rust_type_entry: "druid_rdbc::wrapper::Wrapper",
        rust_method_entry: "druid_rdbc::wrapper::Wrapper::unwrap",
        rust_source: "crates/druid-rdbc/src/wrapper/wrapper.rs",
        notes: "继承自 java.sql.Wrapper；Rust 标准层用 Wrapper::unwrap(TypeId) 解包到同一 DatabaseMetaData 视图",
        id: SweepId::Unwrap,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#dataDefinitionCausesTransactionCommit()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::data_definition_causes_transaction_commit",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::DataDefinitionCausesTransactionCommit,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#dataDefinitionIgnoredInTransactions()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::data_definition_ignored_in_transactions",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::DataDefinitionIgnoredInTransactions,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#deletesAreDetected(int)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::deletes_are_detected",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::DeletesAreDetected,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#doesMaxRowSizeIncludeBlobs()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::does_max_row_size_include_blobs",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::DoesMaxRowSizeIncludeBlobs,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getAttributes(java.lang.String,java.lang.String,java.lang.String,java.lang.String)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_attributes",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::GetAttributes,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getBestRowIdentifier(java.lang.String,java.lang.String,java.lang.String,int,boolean)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_best_row_identifier",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::GetBestRowIdentifier,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getColumnPrivileges(java.lang.String,java.lang.String,java.lang.String,java.lang.String)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_column_privileges",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::GetColumnPrivileges,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getConnection()",
        rust_type_entry: "",
        rust_method_entry: "",
        rust_source: "",
        notes: "",
        id: SweepId::GetConnection,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getDatabaseMinorVersion()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_database_minor_version",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::GetDatabaseMinorVersion,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getDriverMinorVersion()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::get_driver_minor_version",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::GetDriverMinorVersion,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getExtraNameCharacters()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::get_extra_name_characters",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::GetExtraNameCharacters,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getFunctionColumns(java.lang.String,java.lang.String,java.lang.String,java.lang.String)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_function_columns",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::GetFunctionColumns,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getJDBCMajorVersion()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_rdbc_major_version",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "Java getJDBCMajorVersion 在 Rust 标准层按 RDBC 命名提供 get_rdbc_major_version",
        id: SweepId::GetJDBCMajorVersion,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getJDBCMinorVersion()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_rdbc_minor_version",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "Java getJDBCMinorVersion 在 Rust 标准层按 RDBC 命名提供 get_rdbc_minor_version",
        id: SweepId::GetJDBCMinorVersion,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxBinaryLiteralLength()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_binary_literal_length",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxBinaryLiteralLength,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxCatalogNameLength()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_catalog_name_length",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxCatalogNameLength,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxCharLiteralLength()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_char_literal_length",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxCharLiteralLength,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxColumnNameLength()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_column_name_length",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxColumnNameLength,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxColumnsInGroupBy()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_columns_in_group_by",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxColumnsInGroupBy,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxColumnsInIndex()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_columns_in_index",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxColumnsInIndex,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxColumnsInOrderBy()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_columns_in_order_by",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxColumnsInOrderBy,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxCursorNameLength()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_cursor_name_length",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxCursorNameLength,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxIndexLength()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_index_length",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxIndexLength,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxLogicalLobSize()",
        rust_type_entry: "",
        rust_method_entry: "",
        rust_source: "",
        notes: "",
        id: SweepId::GetMaxLogicalLobSize,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxProcedureNameLength()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_procedure_name_length",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxProcedureNameLength,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxRowSize()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_row_size",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxRowSize,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxSchemaNameLength()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_schema_name_length",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxSchemaNameLength,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxStatements()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_statements",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxStatements,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxTableNameLength()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_table_name_length",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxTableNameLength,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxTablesInSelect()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_tables_in_select",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxTablesInSelect,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getMaxUserNameLength()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_user_name_length",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetMaxUserNameLength,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getProcedureColumns(java.lang.String,java.lang.String,java.lang.String,java.lang.String)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_procedure_columns",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetProcedureColumns,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getPseudoColumns(java.lang.String,java.lang.String,java.lang.String,java.lang.String)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_pseudo_columns",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::GetPseudoColumns,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getSchemas()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_schemas",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::GetSchemas,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getSchemas(java.lang.String,java.lang.String)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_schemas_with_pattern",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::GetSchemasWithPattern,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getStringFunctions()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::get_string_functions",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::GetStringFunctions,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getSuperTables(java.lang.String,java.lang.String,java.lang.String)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_super_tables",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::GetSuperTables,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getSuperTypes(java.lang.String,java.lang.String,java.lang.String)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_super_types",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::GetSuperTypes,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getSystemFunctions()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::get_system_functions",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::GetSystemFunctions,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getTablePrivileges(java.lang.String,java.lang.String,java.lang.String)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_table_privileges",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::GetTablePrivileges,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getTimeDateFunctions()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::get_time_date_functions",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::GetTimeDateFunctions,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getUDTs(java.lang.String,java.lang.String,java.lang.String,int[])",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_ud_ts",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "Java getUDTs 在 Rust 标准层按 RDBC 命名提供 get_ud_ts",
        id: SweepId::GetUDTs,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#getVersionColumns(java.lang.String,java.lang.String,java.lang.String)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_version_columns",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::GetVersionColumns,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#insertsAreDetected(int)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::inserts_are_detected",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::InsertsAreDetected,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#isCatalogAtStart()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::is_catalog_at_start",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::IsCatalogAtStart,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#locatorsUpdateCopy()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::locators_update_copy",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::LocatorsUpdateCopy,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#nullPlusNonNullIsNull()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::null_plus_non_null_is_null",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::NullPlusNonNullIsNull,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#nullsAreSortedAtEnd()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::nulls_are_sorted_at_end",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::NullsAreSortedAtEnd,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#nullsAreSortedAtStart()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::nulls_are_sorted_at_start",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::NullsAreSortedAtStart,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#nullsAreSortedLow()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::nulls_are_sorted_low",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::NullsAreSortedLow,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#othersDeletesAreVisible(int)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::others_deletes_are_visible",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::OthersDeletesAreVisible,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#othersInsertsAreVisible(int)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::others_inserts_are_visible",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::OthersInsertsAreVisible,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#othersUpdatesAreVisible(int)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::others_updates_are_visible",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::OthersUpdatesAreVisible,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#ownDeletesAreVisible(int)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::own_deletes_are_visible",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::OwnDeletesAreVisible,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#ownInsertsAreVisible(int)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::own_inserts_are_visible",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::OwnInsertsAreVisible,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#ownUpdatesAreVisible(int)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::own_updates_are_visible",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::OwnUpdatesAreVisible,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#storesLowerCaseIdentifiers()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::stores_lower_case_identifiers",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::StoresLowerCaseIdentifiers,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#storesLowerCaseQuotedIdentifiers()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::stores_lower_case_quoted_identifiers",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::StoresLowerCaseQuotedIdentifiers,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#storesMixedCaseIdentifiers()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::stores_mixed_case_identifiers",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::StoresMixedCaseIdentifiers,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#storesMixedCaseQuotedIdentifiers()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::stores_mixed_case_quoted_identifiers",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::StoresMixedCaseQuotedIdentifiers,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#storesUpperCaseIdentifiers()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::stores_upper_case_identifiers",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::StoresUpperCaseIdentifiers,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#storesUpperCaseQuotedIdentifiers()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::stores_upper_case_quoted_identifiers",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::StoresUpperCaseQuotedIdentifiers,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsANSI92EntryLevelSQL()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_ansi92_entry_level_sql",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsANSI92EntryLevelSQL,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsANSI92FullSQL()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_ansi92_full_sql",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsANSI92FullSQL,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsANSI92IntermediateSQL()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_ansi92_intermediate_sql",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsANSI92IntermediateSQL,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsAlterTableWithAddColumn()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::supports_alter_table_with_add_column",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::SupportsAlterTableWithAddColumn,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsAlterTableWithDropColumn()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::supports_alter_table_with_drop_column",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::SupportsAlterTableWithDropColumn,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsCatalogsInIndexDefinitions()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_index_definitions",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsCatalogsInIndexDefinitions,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsCatalogsInPrivilegeDefinitions()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_privilege_definitions",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsCatalogsInPrivilegeDefinitions,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsCatalogsInProcedureCalls()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_procedure_calls",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsCatalogsInProcedureCalls,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsCatalogsInTableDefinitions()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_table_definitions",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsCatalogsInTableDefinitions,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsConvert()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::supports_convert",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::SupportsConvert,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsConvert(int,int)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::supports_convert_between",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "Java supportsConvert(int,int) 在 Rust 标准层命名为 supports_convert_between，两个 int 参数一致",
        id: SweepId::SupportsConvertBetween,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsCoreSQLGrammar()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_core_sql_grammar",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsCoreSQLGrammar,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsCorrelatedSubqueries()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_correlated_subqueries",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsCorrelatedSubqueries,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsDataDefinitionAndDataManipulationTransactions()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::supports_data_definition_and_data_manipulation_transactions",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::SupportsDataDefinitionAndDataManipulationTransactions,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsDataManipulationTransactionsOnly()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::supports_data_manipulation_transactions_only",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_limits_ext.rs",
        notes: "",
        id: SweepId::SupportsDataManipulationTransactionsOnly,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsDifferentTableCorrelationNames()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::supports_different_table_correlation_names",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::SupportsDifferentTableCorrelationNames,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsExtendedSQLGrammar()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_extended_sql_grammar",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsExtendedSQLGrammar,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsFullOuterJoins()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_full_outer_joins",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsFullOuterJoins,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsGetGeneratedKeys()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_get_generated_keys",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::SupportsGetGeneratedKeys,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsGroupByBeyondSelect()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_group_by_beyond_select",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsGroupByBeyondSelect,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsGroupByUnrelated()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::supports_group_by_unrelated",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::SupportsGroupByUnrelated,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsIntegrityEnhancementFacility()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_integrity_enhancement_facility",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsIntegrityEnhancementFacility,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsLimitedOuterJoins()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_limited_outer_joins",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsLimitedOuterJoins,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsMinimumSQLGrammar()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_minimum_sql_grammar",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsMinimumSQLGrammar,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsMixedCaseIdentifiers()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::supports_mixed_case_identifiers",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::SupportsMixedCaseIdentifiers,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsMixedCaseQuotedIdentifiers()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::supports_mixed_case_quoted_identifiers",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::SupportsMixedCaseQuotedIdentifiers,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsMultipleOpenResults()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_multiple_open_results",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::SupportsMultipleOpenResults,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsMultipleResultSets()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_multiple_result_sets",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsMultipleResultSets,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsMultipleTransactions()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_multiple_transactions",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsMultipleTransactions,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsNonNullableColumns()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_non_nullable_columns",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsNonNullableColumns,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsOpenCursorsAcrossCommit()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_open_cursors_across_commit",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsOpenCursorsAcrossCommit,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsOpenCursorsAcrossRollback()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_open_cursors_across_rollback",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsOpenCursorsAcrossRollback,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsOpenStatementsAcrossCommit()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_open_statements_across_commit",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsOpenStatementsAcrossCommit,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsOpenStatementsAcrossRollback()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_open_statements_across_rollback",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsOpenStatementsAcrossRollback,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsOrderByUnrelated()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::supports_order_by_unrelated",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::SupportsOrderByUnrelated,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsPositionedDelete()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_positioned_delete",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsPositionedDelete,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsPositionedUpdate()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_positioned_update",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsPositionedUpdate,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsRefCursors()",
        rust_type_entry: "",
        rust_method_entry: "",
        rust_source: "",
        notes: "",
        id: SweepId::SupportsRefCursors,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsResultSetHoldability(int)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_result_set_holdability",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::SupportsResultSetHoldability,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsSchemasInIndexDefinitions()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_index_definitions",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsSchemasInIndexDefinitions,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsSchemasInPrivilegeDefinitions()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_privilege_definitions",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsSchemasInPrivilegeDefinitions,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsSchemasInProcedureCalls()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_procedure_calls",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsSchemasInProcedureCalls,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsSchemasInTableDefinitions()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_table_definitions",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsSchemasInTableDefinitions,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsStoredProcedures()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_stored_procedures",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsStoredProcedures,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsSubqueriesInExists()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_exists",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsSubqueriesInExists,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsSubqueriesInIns()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_ins",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsSubqueriesInIns,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsSubqueriesInQuantifieds()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_quantifieds",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsSubqueriesInQuantifieds,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsTableCorrelationNames()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::supports_table_correlation_names",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::SupportsTableCorrelationNames,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#supportsUnionAll()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_union_all",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_sql_capabilities_ext.rs",
        notes: "",
        id: SweepId::SupportsUnionAll,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#updatesAreDetected(int)",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataQueryExt::updates_are_detected",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_query_ext.rs",
        notes: "",
        id: SweepId::UpdatesAreDetected,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#usesLocalFilePerTable()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::uses_local_file_per_table",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::UsesLocalFilePerTable,
    },
    SweepRow {
        identity: "java.sql.DatabaseMetaData#usesLocalFiles()",
        rust_type_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt",
        rust_method_entry: "druid_rdbc::metadata::DatabaseMetaDataInfoExt::uses_local_files",
        rust_source: "crates/druid-rdbc/src/metadata/database_meta_data_info_ext.rs",
        notes: "",
        id: SweepId::UsesLocalFiles,
    },
];

/// 调用一个被扫描方法并返回真实观察结果。
///
/// `no-entry` 的方法在 Rust 标准层没有入口，直接返回 `no-entry`，不做任何伪造调用。
async fn observe(meta: &mut dyn DatabaseMetaData, id: SweepId) -> (ObservationKind, String) {
    match id {
        SweepId::IsWrapperFor => observe_wrapper_flag!(meta),
        SweepId::Unwrap => observe_wrapper_unwrap!(meta),
        SweepId::DataDefinitionCausesTransactionCommit => {
            observe_bool!(meta, data_definition_causes_transaction_commit)
        }
        SweepId::DataDefinitionIgnoredInTransactions => {
            observe_bool!(meta, data_definition_ignored_in_transactions)
        }
        SweepId::DeletesAreDetected => observe_bool!(meta, deletes_are_detected(0)),
        SweepId::DoesMaxRowSizeIncludeBlobs => observe_bool!(meta, does_max_row_size_include_blobs),
        SweepId::GetAttributes => observe_result_set!(meta, get_attributes(None, None, None, None)),
        SweepId::GetBestRowIdentifier => {
            observe_result_set!(meta, get_best_row_identifier(None, None, None, 0, false))
        }
        SweepId::GetColumnPrivileges => {
            observe_result_set!(meta, get_column_privileges(None, None, None, None))
        }
        SweepId::GetConnection => (ObservationKind::NoEntry, "no rust entry".to_owned()),
        SweepId::GetDatabaseMinorVersion => observe_integer!(meta, get_database_minor_version),
        SweepId::GetDriverMinorVersion => observe_integer!(meta, get_driver_minor_version),
        SweepId::GetExtraNameCharacters => observe_string!(meta, get_extra_name_characters),
        SweepId::GetFunctionColumns => {
            observe_result_set!(meta, get_function_columns(None, None, None, None))
        }
        SweepId::GetJDBCMajorVersion => observe_integer!(meta, get_rdbc_major_version),
        SweepId::GetJDBCMinorVersion => observe_integer!(meta, get_rdbc_minor_version),
        SweepId::GetMaxBinaryLiteralLength => observe_integer!(meta, get_max_binary_literal_length),
        SweepId::GetMaxCatalogNameLength => observe_integer!(meta, get_max_catalog_name_length),
        SweepId::GetMaxCharLiteralLength => observe_integer!(meta, get_max_char_literal_length),
        SweepId::GetMaxColumnNameLength => observe_integer!(meta, get_max_column_name_length),
        SweepId::GetMaxColumnsInGroupBy => observe_integer!(meta, get_max_columns_in_group_by),
        SweepId::GetMaxColumnsInIndex => observe_integer!(meta, get_max_columns_in_index),
        SweepId::GetMaxColumnsInOrderBy => observe_integer!(meta, get_max_columns_in_order_by),
        SweepId::GetMaxCursorNameLength => observe_integer!(meta, get_max_cursor_name_length),
        SweepId::GetMaxIndexLength => observe_integer!(meta, get_max_index_length),
        SweepId::GetMaxLogicalLobSize => (ObservationKind::NoEntry, "no rust entry".to_owned()),
        SweepId::GetMaxProcedureNameLength => observe_integer!(meta, get_max_procedure_name_length),
        SweepId::GetMaxRowSize => observe_integer!(meta, get_max_row_size),
        SweepId::GetMaxSchemaNameLength => observe_integer!(meta, get_max_schema_name_length),
        SweepId::GetMaxStatements => observe_integer!(meta, get_max_statements),
        SweepId::GetMaxTableNameLength => observe_integer!(meta, get_max_table_name_length),
        SweepId::GetMaxTablesInSelect => observe_integer!(meta, get_max_tables_in_select),
        SweepId::GetMaxUserNameLength => observe_integer!(meta, get_max_user_name_length),
        SweepId::GetProcedureColumns => {
            observe_result_set!(meta, get_procedure_columns(None, None, None, None))
        }
        SweepId::GetPseudoColumns => {
            observe_result_set!(meta, get_pseudo_columns(None, None, None, None))
        }
        SweepId::GetSchemas => observe_result_set!(meta, get_schemas),
        SweepId::GetSchemasWithPattern => {
            observe_result_set!(meta, get_schemas_with_pattern(None, None))
        }
        SweepId::GetStringFunctions => observe_string!(meta, get_string_functions),
        SweepId::GetSuperTables => observe_result_set!(meta, get_super_tables(None, None, None)),
        SweepId::GetSuperTypes => observe_result_set!(meta, get_super_types(None, None, None)),
        SweepId::GetSystemFunctions => observe_string!(meta, get_system_functions),
        SweepId::GetTablePrivileges => {
            observe_result_set!(meta, get_table_privileges(None, None, None))
        }
        SweepId::GetTimeDateFunctions => observe_string!(meta, get_time_date_functions),
        SweepId::GetUDTs => observe_result_set!(meta, get_ud_ts(None, None, None, None)),
        SweepId::GetVersionColumns => {
            observe_result_set!(meta, get_version_columns(None, None, None))
        }
        SweepId::InsertsAreDetected => observe_bool!(meta, inserts_are_detected(0)),
        SweepId::IsCatalogAtStart => observe_bool!(meta, is_catalog_at_start),
        SweepId::LocatorsUpdateCopy => observe_bool!(meta, locators_update_copy),
        SweepId::NullPlusNonNullIsNull => observe_bool!(meta, null_plus_non_null_is_null),
        SweepId::NullsAreSortedAtEnd => observe_bool!(meta, nulls_are_sorted_at_end),
        SweepId::NullsAreSortedAtStart => observe_bool!(meta, nulls_are_sorted_at_start),
        SweepId::NullsAreSortedLow => observe_bool!(meta, nulls_are_sorted_low),
        SweepId::OthersDeletesAreVisible => observe_bool!(meta, others_deletes_are_visible(0)),
        SweepId::OthersInsertsAreVisible => observe_bool!(meta, others_inserts_are_visible(0)),
        SweepId::OthersUpdatesAreVisible => observe_bool!(meta, others_updates_are_visible(0)),
        SweepId::OwnDeletesAreVisible => observe_bool!(meta, own_deletes_are_visible(0)),
        SweepId::OwnInsertsAreVisible => observe_bool!(meta, own_inserts_are_visible(0)),
        SweepId::OwnUpdatesAreVisible => observe_bool!(meta, own_updates_are_visible(0)),
        SweepId::StoresLowerCaseIdentifiers => observe_bool!(meta, stores_lower_case_identifiers),
        SweepId::StoresLowerCaseQuotedIdentifiers => {
            observe_bool!(meta, stores_lower_case_quoted_identifiers)
        }
        SweepId::StoresMixedCaseIdentifiers => observe_bool!(meta, stores_mixed_case_identifiers),
        SweepId::StoresMixedCaseQuotedIdentifiers => {
            observe_bool!(meta, stores_mixed_case_quoted_identifiers)
        }
        SweepId::StoresUpperCaseIdentifiers => observe_bool!(meta, stores_upper_case_identifiers),
        SweepId::StoresUpperCaseQuotedIdentifiers => {
            observe_bool!(meta, stores_upper_case_quoted_identifiers)
        }
        SweepId::SupportsANSI92EntryLevelSQL => {
            observe_bool!(meta, supports_ansi92_entry_level_sql)
        }
        SweepId::SupportsANSI92FullSQL => observe_bool!(meta, supports_ansi92_full_sql),
        SweepId::SupportsANSI92IntermediateSQL => {
            observe_bool!(meta, supports_ansi92_intermediate_sql)
        }
        SweepId::SupportsAlterTableWithAddColumn => {
            observe_bool!(meta, supports_alter_table_with_add_column)
        }
        SweepId::SupportsAlterTableWithDropColumn => {
            observe_bool!(meta, supports_alter_table_with_drop_column)
        }
        SweepId::SupportsCatalogsInIndexDefinitions => {
            observe_bool!(meta, supports_catalogs_in_index_definitions)
        }
        SweepId::SupportsCatalogsInPrivilegeDefinitions => {
            observe_bool!(meta, supports_catalogs_in_privilege_definitions)
        }
        SweepId::SupportsCatalogsInProcedureCalls => {
            observe_bool!(meta, supports_catalogs_in_procedure_calls)
        }
        SweepId::SupportsCatalogsInTableDefinitions => {
            observe_bool!(meta, supports_catalogs_in_table_definitions)
        }
        SweepId::SupportsConvert => observe_bool!(meta, supports_convert),
        SweepId::SupportsConvertBetween => observe_bool!(meta, supports_convert_between(0, 0)),
        SweepId::SupportsCoreSQLGrammar => observe_bool!(meta, supports_core_sql_grammar),
        SweepId::SupportsCorrelatedSubqueries => {
            observe_bool!(meta, supports_correlated_subqueries)
        }
        SweepId::SupportsDataDefinitionAndDataManipulationTransactions => observe_bool!(
            meta,
            supports_data_definition_and_data_manipulation_transactions
        ),
        SweepId::SupportsDataManipulationTransactionsOnly => {
            observe_bool!(meta, supports_data_manipulation_transactions_only)
        }
        SweepId::SupportsDifferentTableCorrelationNames => {
            observe_bool!(meta, supports_different_table_correlation_names)
        }
        SweepId::SupportsExtendedSQLGrammar => observe_bool!(meta, supports_extended_sql_grammar),
        SweepId::SupportsFullOuterJoins => observe_bool!(meta, supports_full_outer_joins),
        SweepId::SupportsGetGeneratedKeys => observe_bool!(meta, supports_get_generated_keys),
        SweepId::SupportsGroupByBeyondSelect => {
            observe_bool!(meta, supports_group_by_beyond_select)
        }
        SweepId::SupportsGroupByUnrelated => observe_bool!(meta, supports_group_by_unrelated),
        SweepId::SupportsIntegrityEnhancementFacility => {
            observe_bool!(meta, supports_integrity_enhancement_facility)
        }
        SweepId::SupportsLimitedOuterJoins => observe_bool!(meta, supports_limited_outer_joins),
        SweepId::SupportsMinimumSQLGrammar => observe_bool!(meta, supports_minimum_sql_grammar),
        SweepId::SupportsMixedCaseIdentifiers => {
            observe_bool!(meta, supports_mixed_case_identifiers)
        }
        SweepId::SupportsMixedCaseQuotedIdentifiers => {
            observe_bool!(meta, supports_mixed_case_quoted_identifiers)
        }
        SweepId::SupportsMultipleOpenResults => observe_bool!(meta, supports_multiple_open_results),
        SweepId::SupportsMultipleResultSets => observe_bool!(meta, supports_multiple_result_sets),
        SweepId::SupportsMultipleTransactions => {
            observe_bool!(meta, supports_multiple_transactions)
        }
        SweepId::SupportsNonNullableColumns => observe_bool!(meta, supports_non_nullable_columns),
        SweepId::SupportsOpenCursorsAcrossCommit => {
            observe_bool!(meta, supports_open_cursors_across_commit)
        }
        SweepId::SupportsOpenCursorsAcrossRollback => {
            observe_bool!(meta, supports_open_cursors_across_rollback)
        }
        SweepId::SupportsOpenStatementsAcrossCommit => {
            observe_bool!(meta, supports_open_statements_across_commit)
        }
        SweepId::SupportsOpenStatementsAcrossRollback => {
            observe_bool!(meta, supports_open_statements_across_rollback)
        }
        SweepId::SupportsOrderByUnrelated => observe_bool!(meta, supports_order_by_unrelated),
        SweepId::SupportsPositionedDelete => observe_bool!(meta, supports_positioned_delete),
        SweepId::SupportsPositionedUpdate => observe_bool!(meta, supports_positioned_update),
        SweepId::SupportsRefCursors => (ObservationKind::NoEntry, "no rust entry".to_owned()),
        SweepId::SupportsResultSetHoldability => {
            observe_bool!(meta, supports_result_set_holdability(0))
        }
        SweepId::SupportsSchemasInIndexDefinitions => {
            observe_bool!(meta, supports_schemas_in_index_definitions)
        }
        SweepId::SupportsSchemasInPrivilegeDefinitions => {
            observe_bool!(meta, supports_schemas_in_privilege_definitions)
        }
        SweepId::SupportsSchemasInProcedureCalls => {
            observe_bool!(meta, supports_schemas_in_procedure_calls)
        }
        SweepId::SupportsSchemasInTableDefinitions => {
            observe_bool!(meta, supports_schemas_in_table_definitions)
        }
        SweepId::SupportsStoredProcedures => observe_bool!(meta, supports_stored_procedures),
        SweepId::SupportsSubqueriesInExists => observe_bool!(meta, supports_subqueries_in_exists),
        SweepId::SupportsSubqueriesInIns => observe_bool!(meta, supports_subqueries_in_ins),
        SweepId::SupportsSubqueriesInQuantifieds => {
            observe_bool!(meta, supports_subqueries_in_quantifieds)
        }
        SweepId::SupportsTableCorrelationNames => {
            observe_bool!(meta, supports_table_correlation_names)
        }
        SweepId::SupportsUnionAll => observe_bool!(meta, supports_union_all),
        SweepId::UpdatesAreDetected => observe_bool!(meta, updates_are_detected(0)),
        SweepId::UsesLocalFilePerTable => observe_bool!(meta, uses_local_file_per_table),
        SweepId::UsesLocalFiles => observe_bool!(meta, uses_local_files),
    }
}

/// 逐个扫描台账中 `java.sql.DatabaseMetaData` 的全部 UNASSESSED 方法并记录真实观察。
#[tokio::test]
async fn database_meta_data_sweep_records_real_observation_for_every_unassessed_method() {
    let mut pooled = make_pooled().await;
    let mut meta = pooled
        .database_meta_data()
        .await
        .expect("database metadata must be available");

    // 共享观察向量：每个被扫描方法追加一条真实观察。
    let mut observations: Vec<Observation> = Vec::with_capacity(SWEEP_TABLE.len());
    for row in SWEEP_TABLE {
        let (kind, detail) = observe(&mut *meta, row.id).await;
        observations.push(Observation {
            identity: row.identity,
            rust_type_entry: row.rust_type_entry,
            rust_method_entry: row.rust_method_entry,
            rust_source: row.rust_source,
            notes: row.notes,
            kind,
            detail,
        });
    }

    assert_eq!(
        observations.len(),
        SWEEP_TABLE.len(),
        "every scanned method must produce exactly one observation"
    );

    // 每个观察必须落在 value / unsupported / other-error / no-entry 之一。
    for observation in &observations {
        assert!(
            matches!(
                observation.kind,
                ObservationKind::Value
                    | ObservationKind::Unsupported
                    | ObservationKind::OtherError
                    | ObservationKind::NoEntry
            ),
            "observation of {} must fall into a known kind",
            observation.identity
        );
        assert!(
            !observation.detail.trim().is_empty(),
            "observation of {} must keep a non-empty detail",
            observation.identity
        );
    }

    // 有 Rust 入口的方法必须真的被调用，不能退化成 no-entry。
    for row in SWEEP_TABLE {
        if row.rust_method_entry.is_empty() {
            continue;
        }
        let observation = observations
            .iter()
            .find(|observation| observation.identity == row.identity)
            .expect("scanned identity must be observed");
        assert_ne!(
            observation.kind,
            ObservationKind::NoEntry,
            "{} has a rust entry and must be called",
            row.identity
        );
    }

    // 没有 Rust 入口的方法只能记录为 no-entry，并且必须是台账中已知的三个方法。
    for observation in &observations {
        if observation.kind != ObservationKind::NoEntry {
            continue;
        }
        assert!(
            observation.rust_method_entry.is_empty(),
            "{} is no-entry and must not declare a rust entry",
            observation.identity
        );
        assert!(
            matches!(
                observation.identity,
                "java.sql.DatabaseMetaData#getConnection()"
                    | "java.sql.DatabaseMetaData#getMaxLogicalLobSize()"
                    | "java.sql.DatabaseMetaData#supportsRefCursors()"
            ),
            "unexpected no-entry identity {}",
            observation.identity
        );
    }

    // identity 不得重复。
    for (index, observation) in observations.iter().enumerate() {
        assert!(
            !observations[..index]
                .iter()
                .any(|previous| previous.identity == observation.identity),
            "identity {} must be scanned only once",
            observation.identity
        );
    }

    // 打印观察结果，供台账报告离线消费。
    for observation in &observations {
        println!(
            "SWEEP\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            observation.identity,
            observation.kind.as_str(),
            observation.detail,
            observation.rust_type_entry,
            observation.rust_method_entry,
            observation.rust_source,
            observation.notes,
        );
    }
}

/// 负向观察的契约断言：只有真正返回 `SqlException` 时才断言 SQLFeatureNotSupported 的
/// class 与 SQLState；`UnsupportedOperation` 变体没有 SQLState，不做臆造断言。
#[test]
fn database_meta_data_sweep_negative_contract_only_asserts_sql_state_for_sql_exception() {
    let unsupported = RdbcError::UnsupportedOperation {
        operation: "metadata_sweep_probe",
    };
    assert_negative_contract(&unsupported);
    assert_eq!(
        unsupported.class_name(),
        "java.lang.UnsupportedOperationException"
    );

    let feature_not_supported = RdbcError::feature_not_supported("metadata_sweep_probe");
    assert_negative_contract(&feature_not_supported);
    let exception = feature_not_supported
        .sql_exception()
        .expect("feature not supported must carry a sql exception");
    assert_eq!(exception.kind(), SqlExceptionKind::FeatureNotSupported);
    assert_eq!(
        exception.class_name(),
        "java.sql.SQLFeatureNotSupportedException"
    );
    assert_eq!(exception.sql_state(), Some("0A000"));
}
