//! `java.sql.DatabaseMetaData` 的 Java 8 整型常量合同。
//!
//! 对应 Java: `java.sql.DatabaseMetaData`（OpenJDK `jdk8u` 的
//! `jdk/src/share/classes/java/sql/DatabaseMetaData.java`）。
//!
//! 常量顺序、名称语义与取值逐项对齐 Java 8 源码；Java 8 中声明为 `short` 的
//! 常量（`attribute*` 与 `tableIndex*`）在 Rust 统一使用 `i32` 表达，数值本身
//! 保持 Java 8 的精确取值，便于与 `i32` 列值直接比较。

/// `getProcedures` 结果集 `PROCEDURE_TYPE` 列取值：过程是否返回结果未知。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureResultUnknown。
pub const PROCEDURE_RESULT_UNKNOWN: i32 = 0;

/// `getProcedures` 结果集 `PROCEDURE_TYPE` 列取值：过程不返回结果。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureNoResult。
pub const PROCEDURE_NO_RESULT: i32 = 1;

/// `getProcedures` 结果集 `PROCEDURE_TYPE` 列取值：过程返回结果。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureReturnsResult。
pub const PROCEDURE_RETURNS_RESULT: i32 = 2;

/// `getProcedureColumns` 结果集 `COLUMN_TYPE` 列取值：列类型未知。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureColumnUnknown。
pub const PROCEDURE_COLUMN_UNKNOWN: i32 = 0;

/// `getProcedureColumns` 结果集 `COLUMN_TYPE` 列取值：列存放 IN 参数。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureColumnIn。
pub const PROCEDURE_COLUMN_IN: i32 = 1;

/// `getProcedureColumns` 结果集 `COLUMN_TYPE` 列取值：列存放 INOUT 参数。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureColumnInOut。
pub const PROCEDURE_COLUMN_IN_OUT: i32 = 2;

/// `getProcedureColumns` 结果集 `COLUMN_TYPE` 列取值：列存放 OUT 参数。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureColumnOut。
pub const PROCEDURE_COLUMN_OUT: i32 = 4;

/// `getProcedureColumns` 结果集 `COLUMN_TYPE` 列取值：列存放返回值。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureColumnReturn。
pub const PROCEDURE_COLUMN_RETURN: i32 = 5;

/// `getProcedureColumns` 结果集 `COLUMN_TYPE` 列取值：列存放结果集列。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureColumnResult。
pub const PROCEDURE_COLUMN_RESULT: i32 = 3;

/// `getProcedureColumns` 结果集 `NULLABLE` 列取值：不允许 `NULL` 值。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureNoNulls。
pub const PROCEDURE_NO_NULLS: i32 = 0;

/// `getProcedureColumns` 结果集 `NULLABLE` 列取值：允许 `NULL` 值。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureNullable。
pub const PROCEDURE_NULLABLE: i32 = 1;

/// `getProcedureColumns` 结果集 `NULLABLE` 列取值：是否允许 `NULL` 值未知。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureNullableUnknown。
pub const PROCEDURE_NULLABLE_UNKNOWN: i32 = 2;

/// `getColumns` 结果集 `NULLABLE` 列取值：列可能不允许 `NULL` 值。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#columnNoNulls。
pub const COLUMN_NO_NULLS: i32 = 0;

/// `getColumns` 结果集 `NULLABLE` 列取值：列确定允许 `NULL` 值。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#columnNullable。
pub const COLUMN_NULLABLE: i32 = 1;

/// `getColumns` 结果集 `NULLABLE` 列取值：列的可空性未知。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#columnNullableUnknown。
pub const COLUMN_NULLABLE_UNKNOWN: i32 = 2;

/// `getBestRowIdentifier` 结果集 `SCOPE` 列取值：最佳行标识作用域非常临时，仅在行被使用期间有效。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#bestRowTemporary。
pub const BEST_ROW_TEMPORARY: i32 = 0;

/// `getBestRowIdentifier` 结果集 `SCOPE` 列取值：最佳行标识作用域为当前事务的剩余部分。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#bestRowTransaction。
pub const BEST_ROW_TRANSACTION: i32 = 1;

/// `getBestRowIdentifier` 结果集 `SCOPE` 列取值：最佳行标识作用域为当前会话的剩余部分。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#bestRowSession。
pub const BEST_ROW_SESSION: i32 = 2;

/// `getBestRowIdentifier` 结果集 `PSEUDO_COLUMN` 列取值：最佳行标识可能是伪列，也可能是真实列。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#bestRowUnknown。
pub const BEST_ROW_UNKNOWN: i32 = 0;

/// `getBestRowIdentifier` 结果集 `PSEUDO_COLUMN` 列取值：最佳行标识不是伪列。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#bestRowNotPseudo。
pub const BEST_ROW_NOT_PSEUDO: i32 = 1;

/// `getBestRowIdentifier` 结果集 `PSEUDO_COLUMN` 列取值：最佳行标识是伪列。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#bestRowPseudo。
pub const BEST_ROW_PSEUDO: i32 = 2;

/// `getVersionColumns` 结果集 `PSEUDO_COLUMN` 列取值：版本列可能是伪列，也可能是真实列。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#versionColumnUnknown。
pub const VERSION_COLUMN_UNKNOWN: i32 = 0;

/// `getVersionColumns` 结果集 `PSEUDO_COLUMN` 列取值：版本列不是伪列。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#versionColumnNotPseudo。
pub const VERSION_COLUMN_NOT_PSEUDO: i32 = 1;

/// `getVersionColumns` 结果集 `PSEUDO_COLUMN` 列取值：版本列是伪列。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#versionColumnPseudo。
pub const VERSION_COLUMN_PSEUDO: i32 = 2;

/// `getImportedKeys` 等结果集 `UPDATE_RULE`/`DELETE_RULE` 列取值：主键更新或删除时级联修改外键。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeyCascade。
pub const IMPORTED_KEY_CASCADE: i32 = 0;

/// `getImportedKeys` 等结果集 `UPDATE_RULE`/`DELETE_RULE` 列取值：主键被外键引用时不允许更新或删除。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeyRestrict。
pub const IMPORTED_KEY_RESTRICT: i32 = 1;

/// `getImportedKeys` 等结果集 `UPDATE_RULE`/`DELETE_RULE` 列取值：主键更新或删除时外键置为 `NULL`。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeySetNull。
pub const IMPORTED_KEY_SET_NULL: i32 = 2;

/// `getImportedKeys` 等结果集 `UPDATE_RULE`/`DELETE_RULE` 列取值：主键被引用时不允许更新或删除，且不做级联动作。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeyNoAction。
pub const IMPORTED_KEY_NO_ACTION: i32 = 3;

/// `getImportedKeys` 等结果集 `UPDATE_RULE`/`DELETE_RULE` 列取值：主键更新或删除时外键置为默认值。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeySetDefault。
pub const IMPORTED_KEY_SET_DEFAULT: i32 = 4;

/// `getImportedKeys` 等结果集 `DEFERRABILITY` 列取值：初始延迟检查约束。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeyInitiallyDeferred。
pub const IMPORTED_KEY_INITIALLY_DEFERRED: i32 = 5;

/// `getImportedKeys` 等结果集 `DEFERRABILITY` 列取值：初始立即检查约束。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeyInitiallyImmediate。
pub const IMPORTED_KEY_INITIALLY_IMMEDIATE: i32 = 6;

/// `getImportedKeys` 等结果集 `DEFERRABILITY` 列取值：约束不可延迟。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeyNotDeferrable。
pub const IMPORTED_KEY_NOT_DEFERRABLE: i32 = 7;

/// `getTypeInfo` 结果集 `NULLABLE` 列取值：该数据类型不允许 `NULL` 值。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#typeNoNulls。
pub const TYPE_NO_NULLS: i32 = 0;

/// `getTypeInfo` 结果集 `NULLABLE` 列取值：该数据类型允许 `NULL` 值。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#typeNullable。
pub const TYPE_NULLABLE: i32 = 1;

/// `getTypeInfo` 结果集 `NULLABLE` 列取值：是否允许 `NULL` 值未知。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#typeNullableUnknown。
pub const TYPE_NULLABLE_UNKNOWN: i32 = 2;

/// `getTypeInfo` 结果集 `SEARCHABLE` 列取值：不支持 `WHERE` 谓词搜索。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#typePredNone。
pub const TYPE_PRED_NONE: i32 = 0;

/// `getTypeInfo` 结果集 `SEARCHABLE` 列取值：只能使用 `LIKE` 谓词搜索。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#typePredChar。
pub const TYPE_PRED_CHAR: i32 = 1;

/// `getTypeInfo` 结果集 `SEARCHABLE` 列取值：只能使用不含 `LIKE` 的谓词搜索。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#typePredBasic。
pub const TYPE_PRED_BASIC: i32 = 2;

/// `getTypeInfo` 结果集 `SEARCHABLE` 列取值：可使用任意 `WHERE` 谓词搜索。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#typeSearchable。
pub const TYPE_SEARCHABLE: i32 = 3;

/// `getIndexInfo` 结果集 `TYPE` 列取值：该列是与表索引描述一起返回的表统计信息。
///
/// Java 8 声明类型 `short`；对应 Java: DatabaseMetaData#tableIndexStatistic。
pub const TABLE_INDEX_STATISTIC: i32 = 0;

/// `getIndexInfo` 结果集 `TYPE` 列取值：该表索引是聚簇索引。
///
/// Java 8 声明类型 `short`；对应 Java: DatabaseMetaData#tableIndexClustered。
pub const TABLE_INDEX_CLUSTERED: i32 = 1;

/// `getIndexInfo` 结果集 `TYPE` 列取值：该表索引是哈希索引。
///
/// Java 8 声明类型 `short`；对应 Java: DatabaseMetaData#tableIndexHashed。
pub const TABLE_INDEX_HASHED: i32 = 2;

/// `getIndexInfo` 结果集 `TYPE` 列取值：该表索引既不是聚簇索引，也不是哈希索引或表统计信息。
///
/// Java 8 声明类型 `short`；对应 Java: DatabaseMetaData#tableIndexOther。
pub const TABLE_INDEX_OTHER: i32 = 3;

/// `getAttributes` 结果集 `NULLABLE` 列取值：可能不允许 `NULL` 值。
///
/// Java 8 声明类型 `short`；对应 Java: DatabaseMetaData#attributeNoNulls。
pub const ATTRIBUTE_NO_NULLS: i32 = 0;

/// `getAttributes` 结果集 `NULLABLE` 列取值：确定允许 `NULL` 值。
///
/// Java 8 声明类型 `short`；对应 Java: DatabaseMetaData#attributeNullable。
pub const ATTRIBUTE_NULLABLE: i32 = 1;

/// `getAttributes` 结果集 `NULLABLE` 列取值：是否允许 `NULL` 值未知。
///
/// Java 8 声明类型 `short`；对应 Java: DatabaseMetaData#attributeNullableUnknown。
pub const ATTRIBUTE_NULLABLE_UNKNOWN: i32 = 2;

/// `getSQLStateType` 返回值：表示 `SQLException.getSQLState` 返回 X/Open SQL CLI SQLSTATE 值。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#sqlStateXOpen。
pub const SQL_STATE_X_OPEN: i32 = 1;

/// `getSQLStateType` 返回值：表示 `SQLException.getSQLState` 返回 SQLSTATE 值。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#sqlStateSQL。
pub const SQL_STATE_SQL: i32 = 2;

/// `getSQLStateType` 返回值：表示 `SQLException.getSQLState` 返回 SQL99 SQLSTATE 值，Java 8 中定义为 `sqlStateSQL`，仅为兼容保留。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#sqlStateSQL99。
pub const SQL_STATE_SQL_99: i32 = SQL_STATE_SQL;

/// `getFunctionColumns` 结果集 `COLUMN_TYPE` 列取值：参数或列类型未知。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionColumnUnknown。
pub const FUNCTION_COLUMN_UNKNOWN: i32 = 0;

/// `getFunctionColumns` 结果集 `COLUMN_TYPE` 列取值：参数或列是 IN 参数。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionColumnIn。
pub const FUNCTION_COLUMN_IN: i32 = 1;

/// `getFunctionColumns` 结果集 `COLUMN_TYPE` 列取值：参数或列是 INOUT 参数。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionColumnInOut。
pub const FUNCTION_COLUMN_IN_OUT: i32 = 2;

/// `getFunctionColumns` 结果集 `COLUMN_TYPE` 列取值：参数或列是 OUT 参数。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionColumnOut。
pub const FUNCTION_COLUMN_OUT: i32 = 3;

/// `getFunctionColumns` 结果集 `COLUMN_TYPE` 列取值：参数或列是返回值。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionReturn。
pub const FUNCTION_RETURN: i32 = 4;

/// `getFunctionColumns` 结果集 `COLUMN_TYPE` 列取值：参数或列是结果集中的列。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionColumnResult。
pub const FUNCTION_COLUMN_RESULT: i32 = 5;

/// `getFunctionColumns` 结果集 `NULLABLE` 列取值：不允许 `NULL` 值。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionNoNulls。
pub const FUNCTION_NO_NULLS: i32 = 0;

/// `getFunctionColumns` 结果集 `NULLABLE` 列取值：允许 `NULL` 值。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionNullable。
pub const FUNCTION_NULLABLE: i32 = 1;

/// `getFunctionColumns` 结果集 `NULLABLE` 列取值：是否允许 `NULL` 值未知。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionNullableUnknown。
pub const FUNCTION_NULLABLE_UNKNOWN: i32 = 2;

/// `getFunctions` 结果集 `FUNCTION_TYPE` 列取值：未知函数返回结果还是表。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionResultUnknown。
pub const FUNCTION_RESULT_UNKNOWN: i32 = 0;

/// `getFunctions` 结果集 `FUNCTION_TYPE` 列取值：函数不返回表。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionNoTable。
pub const FUNCTION_NO_TABLE: i32 = 1;

/// `getFunctions` 结果集 `FUNCTION_TYPE` 列取值：函数返回表。
///
/// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionReturnsTable。
pub const FUNCTION_RETURNS_TABLE: i32 = 2;

/// `java.sql.DatabaseMetaData` 常量的类型命名空间入口。
///
/// 对应 Java: `java.sql.DatabaseMetaData`。所有关联常量委托本模块的同名标准常量，
/// 保留 `DatabaseMetaDataConstants::常量` 调用形态。
pub struct DatabaseMetaDataConstants;

impl DatabaseMetaDataConstants {
    /// `getProcedures` 结果集 `PROCEDURE_TYPE` 列取值：过程是否返回结果未知。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureResultUnknown。
    pub const PROCEDURE_RESULT_UNKNOWN: i32 = self::PROCEDURE_RESULT_UNKNOWN;
    /// `getProcedures` 结果集 `PROCEDURE_TYPE` 列取值：过程不返回结果。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureNoResult。
    pub const PROCEDURE_NO_RESULT: i32 = self::PROCEDURE_NO_RESULT;
    /// `getProcedures` 结果集 `PROCEDURE_TYPE` 列取值：过程返回结果。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureReturnsResult。
    pub const PROCEDURE_RETURNS_RESULT: i32 = self::PROCEDURE_RETURNS_RESULT;
    /// `getProcedureColumns` 结果集 `COLUMN_TYPE` 列取值：列类型未知。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureColumnUnknown。
    pub const PROCEDURE_COLUMN_UNKNOWN: i32 = self::PROCEDURE_COLUMN_UNKNOWN;
    /// `getProcedureColumns` 结果集 `COLUMN_TYPE` 列取值：列存放 IN 参数。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureColumnIn。
    pub const PROCEDURE_COLUMN_IN: i32 = self::PROCEDURE_COLUMN_IN;
    /// `getProcedureColumns` 结果集 `COLUMN_TYPE` 列取值：列存放 INOUT 参数。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureColumnInOut。
    pub const PROCEDURE_COLUMN_IN_OUT: i32 = self::PROCEDURE_COLUMN_IN_OUT;
    /// `getProcedureColumns` 结果集 `COLUMN_TYPE` 列取值：列存放 OUT 参数。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureColumnOut。
    pub const PROCEDURE_COLUMN_OUT: i32 = self::PROCEDURE_COLUMN_OUT;
    /// `getProcedureColumns` 结果集 `COLUMN_TYPE` 列取值：列存放返回值。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureColumnReturn。
    pub const PROCEDURE_COLUMN_RETURN: i32 = self::PROCEDURE_COLUMN_RETURN;
    /// `getProcedureColumns` 结果集 `COLUMN_TYPE` 列取值：列存放结果集列。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureColumnResult。
    pub const PROCEDURE_COLUMN_RESULT: i32 = self::PROCEDURE_COLUMN_RESULT;
    /// `getProcedureColumns` 结果集 `NULLABLE` 列取值：不允许 `NULL` 值。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureNoNulls。
    pub const PROCEDURE_NO_NULLS: i32 = self::PROCEDURE_NO_NULLS;
    /// `getProcedureColumns` 结果集 `NULLABLE` 列取值：允许 `NULL` 值。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureNullable。
    pub const PROCEDURE_NULLABLE: i32 = self::PROCEDURE_NULLABLE;
    /// `getProcedureColumns` 结果集 `NULLABLE` 列取值：是否允许 `NULL` 值未知。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#procedureNullableUnknown。
    pub const PROCEDURE_NULLABLE_UNKNOWN: i32 = self::PROCEDURE_NULLABLE_UNKNOWN;
    /// `getColumns` 结果集 `NULLABLE` 列取值：列可能不允许 `NULL` 值。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#columnNoNulls。
    pub const COLUMN_NO_NULLS: i32 = self::COLUMN_NO_NULLS;
    /// `getColumns` 结果集 `NULLABLE` 列取值：列确定允许 `NULL` 值。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#columnNullable。
    pub const COLUMN_NULLABLE: i32 = self::COLUMN_NULLABLE;
    /// `getColumns` 结果集 `NULLABLE` 列取值：列的可空性未知。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#columnNullableUnknown。
    pub const COLUMN_NULLABLE_UNKNOWN: i32 = self::COLUMN_NULLABLE_UNKNOWN;
    /// `getBestRowIdentifier` 结果集 `SCOPE` 列取值：最佳行标识作用域非常临时，仅在行被使用期间有效。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#bestRowTemporary。
    pub const BEST_ROW_TEMPORARY: i32 = self::BEST_ROW_TEMPORARY;
    /// `getBestRowIdentifier` 结果集 `SCOPE` 列取值：最佳行标识作用域为当前事务的剩余部分。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#bestRowTransaction。
    pub const BEST_ROW_TRANSACTION: i32 = self::BEST_ROW_TRANSACTION;
    /// `getBestRowIdentifier` 结果集 `SCOPE` 列取值：最佳行标识作用域为当前会话的剩余部分。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#bestRowSession。
    pub const BEST_ROW_SESSION: i32 = self::BEST_ROW_SESSION;
    /// `getBestRowIdentifier` 结果集 `PSEUDO_COLUMN` 列取值：最佳行标识可能是伪列，也可能是真实列。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#bestRowUnknown。
    pub const BEST_ROW_UNKNOWN: i32 = self::BEST_ROW_UNKNOWN;
    /// `getBestRowIdentifier` 结果集 `PSEUDO_COLUMN` 列取值：最佳行标识不是伪列。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#bestRowNotPseudo。
    pub const BEST_ROW_NOT_PSEUDO: i32 = self::BEST_ROW_NOT_PSEUDO;
    /// `getBestRowIdentifier` 结果集 `PSEUDO_COLUMN` 列取值：最佳行标识是伪列。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#bestRowPseudo。
    pub const BEST_ROW_PSEUDO: i32 = self::BEST_ROW_PSEUDO;
    /// `getVersionColumns` 结果集 `PSEUDO_COLUMN` 列取值：版本列可能是伪列，也可能是真实列。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#versionColumnUnknown。
    pub const VERSION_COLUMN_UNKNOWN: i32 = self::VERSION_COLUMN_UNKNOWN;
    /// `getVersionColumns` 结果集 `PSEUDO_COLUMN` 列取值：版本列不是伪列。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#versionColumnNotPseudo。
    pub const VERSION_COLUMN_NOT_PSEUDO: i32 = self::VERSION_COLUMN_NOT_PSEUDO;
    /// `getVersionColumns` 结果集 `PSEUDO_COLUMN` 列取值：版本列是伪列。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#versionColumnPseudo。
    pub const VERSION_COLUMN_PSEUDO: i32 = self::VERSION_COLUMN_PSEUDO;
    /// `getImportedKeys` 等结果集 `UPDATE_RULE`/`DELETE_RULE` 列取值：主键更新或删除时级联修改外键。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeyCascade。
    pub const IMPORTED_KEY_CASCADE: i32 = self::IMPORTED_KEY_CASCADE;
    /// `getImportedKeys` 等结果集 `UPDATE_RULE`/`DELETE_RULE` 列取值：主键被外键引用时不允许更新或删除。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeyRestrict。
    pub const IMPORTED_KEY_RESTRICT: i32 = self::IMPORTED_KEY_RESTRICT;
    /// `getImportedKeys` 等结果集 `UPDATE_RULE`/`DELETE_RULE` 列取值：主键更新或删除时外键置为 `NULL`。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeySetNull。
    pub const IMPORTED_KEY_SET_NULL: i32 = self::IMPORTED_KEY_SET_NULL;
    /// `getImportedKeys` 等结果集 `UPDATE_RULE`/`DELETE_RULE` 列取值：主键被引用时不允许更新或删除，且不做级联动作。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeyNoAction。
    pub const IMPORTED_KEY_NO_ACTION: i32 = self::IMPORTED_KEY_NO_ACTION;
    /// `getImportedKeys` 等结果集 `UPDATE_RULE`/`DELETE_RULE` 列取值：主键更新或删除时外键置为默认值。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeySetDefault。
    pub const IMPORTED_KEY_SET_DEFAULT: i32 = self::IMPORTED_KEY_SET_DEFAULT;
    /// `getImportedKeys` 等结果集 `DEFERRABILITY` 列取值：初始延迟检查约束。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeyInitiallyDeferred。
    pub const IMPORTED_KEY_INITIALLY_DEFERRED: i32 = self::IMPORTED_KEY_INITIALLY_DEFERRED;
    /// `getImportedKeys` 等结果集 `DEFERRABILITY` 列取值：初始立即检查约束。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeyInitiallyImmediate。
    pub const IMPORTED_KEY_INITIALLY_IMMEDIATE: i32 = self::IMPORTED_KEY_INITIALLY_IMMEDIATE;
    /// `getImportedKeys` 等结果集 `DEFERRABILITY` 列取值：约束不可延迟。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#importedKeyNotDeferrable。
    pub const IMPORTED_KEY_NOT_DEFERRABLE: i32 = self::IMPORTED_KEY_NOT_DEFERRABLE;
    /// `getTypeInfo` 结果集 `NULLABLE` 列取值：该数据类型不允许 `NULL` 值。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#typeNoNulls。
    pub const TYPE_NO_NULLS: i32 = self::TYPE_NO_NULLS;
    /// `getTypeInfo` 结果集 `NULLABLE` 列取值：该数据类型允许 `NULL` 值。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#typeNullable。
    pub const TYPE_NULLABLE: i32 = self::TYPE_NULLABLE;
    /// `getTypeInfo` 结果集 `NULLABLE` 列取值：是否允许 `NULL` 值未知。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#typeNullableUnknown。
    pub const TYPE_NULLABLE_UNKNOWN: i32 = self::TYPE_NULLABLE_UNKNOWN;
    /// `getTypeInfo` 结果集 `SEARCHABLE` 列取值：不支持 `WHERE` 谓词搜索。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#typePredNone。
    pub const TYPE_PRED_NONE: i32 = self::TYPE_PRED_NONE;
    /// `getTypeInfo` 结果集 `SEARCHABLE` 列取值：只能使用 `LIKE` 谓词搜索。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#typePredChar。
    pub const TYPE_PRED_CHAR: i32 = self::TYPE_PRED_CHAR;
    /// `getTypeInfo` 结果集 `SEARCHABLE` 列取值：只能使用不含 `LIKE` 的谓词搜索。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#typePredBasic。
    pub const TYPE_PRED_BASIC: i32 = self::TYPE_PRED_BASIC;
    /// `getTypeInfo` 结果集 `SEARCHABLE` 列取值：可使用任意 `WHERE` 谓词搜索。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#typeSearchable。
    pub const TYPE_SEARCHABLE: i32 = self::TYPE_SEARCHABLE;
    /// `getIndexInfo` 结果集 `TYPE` 列取值：该列是与表索引描述一起返回的表统计信息。
    ///
    /// Java 8 声明类型 `short`；对应 Java: DatabaseMetaData#tableIndexStatistic。
    pub const TABLE_INDEX_STATISTIC: i32 = self::TABLE_INDEX_STATISTIC;
    /// `getIndexInfo` 结果集 `TYPE` 列取值：该表索引是聚簇索引。
    ///
    /// Java 8 声明类型 `short`；对应 Java: DatabaseMetaData#tableIndexClustered。
    pub const TABLE_INDEX_CLUSTERED: i32 = self::TABLE_INDEX_CLUSTERED;
    /// `getIndexInfo` 结果集 `TYPE` 列取值：该表索引是哈希索引。
    ///
    /// Java 8 声明类型 `short`；对应 Java: DatabaseMetaData#tableIndexHashed。
    pub const TABLE_INDEX_HASHED: i32 = self::TABLE_INDEX_HASHED;
    /// `getIndexInfo` 结果集 `TYPE` 列取值：该表索引既不是聚簇索引，也不是哈希索引或表统计信息。
    ///
    /// Java 8 声明类型 `short`；对应 Java: DatabaseMetaData#tableIndexOther。
    pub const TABLE_INDEX_OTHER: i32 = self::TABLE_INDEX_OTHER;
    /// `getAttributes` 结果集 `NULLABLE` 列取值：可能不允许 `NULL` 值。
    ///
    /// Java 8 声明类型 `short`；对应 Java: DatabaseMetaData#attributeNoNulls。
    pub const ATTRIBUTE_NO_NULLS: i32 = self::ATTRIBUTE_NO_NULLS;
    /// `getAttributes` 结果集 `NULLABLE` 列取值：确定允许 `NULL` 值。
    ///
    /// Java 8 声明类型 `short`；对应 Java: DatabaseMetaData#attributeNullable。
    pub const ATTRIBUTE_NULLABLE: i32 = self::ATTRIBUTE_NULLABLE;
    /// `getAttributes` 结果集 `NULLABLE` 列取值：是否允许 `NULL` 值未知。
    ///
    /// Java 8 声明类型 `short`；对应 Java: DatabaseMetaData#attributeNullableUnknown。
    pub const ATTRIBUTE_NULLABLE_UNKNOWN: i32 = self::ATTRIBUTE_NULLABLE_UNKNOWN;
    /// `getSQLStateType` 返回值：表示 `SQLException.getSQLState` 返回 X/Open SQL CLI SQLSTATE 值。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#sqlStateXOpen。
    pub const SQL_STATE_X_OPEN: i32 = self::SQL_STATE_X_OPEN;
    /// `getSQLStateType` 返回值：表示 `SQLException.getSQLState` 返回 SQLSTATE 值。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#sqlStateSQL。
    pub const SQL_STATE_SQL: i32 = self::SQL_STATE_SQL;
    /// `getSQLStateType` 返回值：表示 `SQLException.getSQLState` 返回 SQL99 SQLSTATE 值，Java 8 中定义为 `sqlStateSQL`，仅为兼容保留。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#sqlStateSQL99。
    pub const SQL_STATE_SQL_99: i32 = self::SQL_STATE_SQL_99;
    /// `getFunctionColumns` 结果集 `COLUMN_TYPE` 列取值：参数或列类型未知。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionColumnUnknown。
    pub const FUNCTION_COLUMN_UNKNOWN: i32 = self::FUNCTION_COLUMN_UNKNOWN;
    /// `getFunctionColumns` 结果集 `COLUMN_TYPE` 列取值：参数或列是 IN 参数。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionColumnIn。
    pub const FUNCTION_COLUMN_IN: i32 = self::FUNCTION_COLUMN_IN;
    /// `getFunctionColumns` 结果集 `COLUMN_TYPE` 列取值：参数或列是 INOUT 参数。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionColumnInOut。
    pub const FUNCTION_COLUMN_IN_OUT: i32 = self::FUNCTION_COLUMN_IN_OUT;
    /// `getFunctionColumns` 结果集 `COLUMN_TYPE` 列取值：参数或列是 OUT 参数。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionColumnOut。
    pub const FUNCTION_COLUMN_OUT: i32 = self::FUNCTION_COLUMN_OUT;
    /// `getFunctionColumns` 结果集 `COLUMN_TYPE` 列取值：参数或列是返回值。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionReturn。
    pub const FUNCTION_RETURN: i32 = self::FUNCTION_RETURN;
    /// `getFunctionColumns` 结果集 `COLUMN_TYPE` 列取值：参数或列是结果集中的列。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionColumnResult。
    pub const FUNCTION_COLUMN_RESULT: i32 = self::FUNCTION_COLUMN_RESULT;
    /// `getFunctionColumns` 结果集 `NULLABLE` 列取值：不允许 `NULL` 值。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionNoNulls。
    pub const FUNCTION_NO_NULLS: i32 = self::FUNCTION_NO_NULLS;
    /// `getFunctionColumns` 结果集 `NULLABLE` 列取值：允许 `NULL` 值。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionNullable。
    pub const FUNCTION_NULLABLE: i32 = self::FUNCTION_NULLABLE;
    /// `getFunctionColumns` 结果集 `NULLABLE` 列取值：是否允许 `NULL` 值未知。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionNullableUnknown。
    pub const FUNCTION_NULLABLE_UNKNOWN: i32 = self::FUNCTION_NULLABLE_UNKNOWN;
    /// `getFunctions` 结果集 `FUNCTION_TYPE` 列取值：未知函数返回结果还是表。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionResultUnknown。
    pub const FUNCTION_RESULT_UNKNOWN: i32 = self::FUNCTION_RESULT_UNKNOWN;
    /// `getFunctions` 结果集 `FUNCTION_TYPE` 列取值：函数不返回表。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionNoTable。
    pub const FUNCTION_NO_TABLE: i32 = self::FUNCTION_NO_TABLE;
    /// `getFunctions` 结果集 `FUNCTION_TYPE` 列取值：函数返回表。
    ///
    /// Java 8 声明类型 `int`；对应 Java: DatabaseMetaData#functionReturnsTable。
    pub const FUNCTION_RETURNS_TABLE: i32 = self::FUNCTION_RETURNS_TABLE;
}
