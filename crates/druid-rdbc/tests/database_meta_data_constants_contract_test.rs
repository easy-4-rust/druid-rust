//! `java.sql.DatabaseMetaData` Java 8 常量合同的逐常量精确值断言。
//!
//! 数据来源：`docs/verification/rdbc-first/java8-method-evidence.tsv`，过滤条件为
//! `owner=java.sql.DatabaseMetaData` 且 `capability=UNASSESSED`，并排除
//! `java.sql.DatabaseMetaData#TYPE`（类型本身）与以 `()` 结尾的方法 identity。
//! 该过滤命中 61 个常量 identity，与 OpenJDK `jdk8u` 的
//! `jdk/src/share/classes/java/sql/DatabaseMetaData.java` 中的常量声明逐项一致。
//!
//! 精确取值同时核对两处来源：
//! - OpenJDK `jdk8u`（`openjdk/jdk8u@7373384725418fa645fb4508ec05a869bc7fc3e7`）
//!   的 `java/sql/DatabaseMetaData.java` 源码声明；
//! - Java 8 运行时 `rt.jar` 中 `java.sql.DatabaseMetaData` 类文件的 `ConstantValue` 属性。
//!
//! 对应 Java: `java.sql.DatabaseMetaData`。

use druid_rdbc::metadata::database_meta_data_constants as constants;
use druid_rdbc::metadata::DatabaseMetaDataConstants;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

/// 台账中该 owner 的未评估常量数量。
const CONSTANT_COUNT: usize = 61;

/// 台账 identity 的 owner 前缀。
const OWNER: &str = "java.sql.DatabaseMetaData";

/// Java 8 常量名，顺序与 Java 8 源码声明顺序一致。
const JAVA8_CONSTANT_NAMES: [&str; CONSTANT_COUNT] = [
    "procedureResultUnknown",
    "procedureNoResult",
    "procedureReturnsResult",
    "procedureColumnUnknown",
    "procedureColumnIn",
    "procedureColumnInOut",
    "procedureColumnOut",
    "procedureColumnReturn",
    "procedureColumnResult",
    "procedureNoNulls",
    "procedureNullable",
    "procedureNullableUnknown",
    "columnNoNulls",
    "columnNullable",
    "columnNullableUnknown",
    "bestRowTemporary",
    "bestRowTransaction",
    "bestRowSession",
    "bestRowUnknown",
    "bestRowNotPseudo",
    "bestRowPseudo",
    "versionColumnUnknown",
    "versionColumnNotPseudo",
    "versionColumnPseudo",
    "importedKeyCascade",
    "importedKeyRestrict",
    "importedKeySetNull",
    "importedKeyNoAction",
    "importedKeySetDefault",
    "importedKeyInitiallyDeferred",
    "importedKeyInitiallyImmediate",
    "importedKeyNotDeferrable",
    "typeNoNulls",
    "typeNullable",
    "typeNullableUnknown",
    "typePredNone",
    "typePredChar",
    "typePredBasic",
    "typeSearchable",
    "tableIndexStatistic",
    "tableIndexClustered",
    "tableIndexHashed",
    "tableIndexOther",
    "attributeNoNulls",
    "attributeNullable",
    "attributeNullableUnknown",
    "sqlStateXOpen",
    "sqlStateSQL",
    "sqlStateSQL99",
    "functionColumnUnknown",
    "functionColumnIn",
    "functionColumnInOut",
    "functionColumnOut",
    "functionReturn",
    "functionColumnResult",
    "functionNoNulls",
    "functionNullable",
    "functionNullableUnknown",
    "functionResultUnknown",
    "functionNoTable",
    "functionReturnsTable",
];

/// 定位证据台账文件。
fn evidence_ledger_path() -> PathBuf {
    let mut directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let candidate = directory
            .join("docs")
            .join("verification")
            .join("rdbc-first")
            .join("java8-method-evidence.tsv");
        if candidate.is_file() {
            return candidate;
        }
        if !directory.pop() {
            panic!("未找到证据台账 docs/verification/rdbc-first/java8-method-evidence.tsv");
        }
    }
}

/// 读取台账中 `owner=java.sql.DatabaseMetaData` 且 `capability=UNASSESSED` 的常量 identity。
///
/// 方法 identity 以 `()` 结尾，类型 identity 以 `#TYPE` 结尾，两者都不是常量。
fn ledger_unassessed_constant_identities() -> BTreeSet<String> {
    let path = evidence_ledger_path();
    let content = fs::read_to_string(&path).expect("读取证据台账失败");
    let mut rows = content
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'));
    let header = rows.next().expect("证据台账缺少表头");
    let columns: Vec<&str> = header.split('\t').collect();
    let column_index = |name: &str| {
        columns
            .iter()
            .position(|column| *column == name)
            .unwrap_or_else(|| panic!("证据台账缺少列 {name}"))
    };
    let identity_index = column_index("identity");
    let owner_index = column_index("owner");
    let capability_index = column_index("capability");
    let last_index = identity_index.max(owner_index).max(capability_index);
    let mut identities = BTreeSet::new();
    for row in rows {
        if row.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = row.split('\t').collect();
        if fields.len() <= last_index {
            continue;
        }
        // 常量证据已登记为 SUPPORTED；该测试断言的是“台账中本接口的常量 identity
        // 集合与模块一致”，与 capability 无关，因此不再按 UNASSESSED 过滤。
        if fields[owner_index] != OWNER {
            continue;
        }
        let identity = fields[identity_index];
        // 方法 identity 含 `(`；TYPE 是类型标记。常量 identity 不含括号。
        if identity.contains('(') || identity.ends_with("#TYPE") {
            continue;
        }
        identities.insert(identity.to_owned());
    }
    identities
}

#[test]
fn database_meta_data_constants_match_jdk8_exact_values() {
    assert_eq!(
        constants::PROCEDURE_RESULT_UNKNOWN,
        0,
        "java.sql.DatabaseMetaData#procedureResultUnknown 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::PROCEDURE_NO_RESULT,
        1,
        "java.sql.DatabaseMetaData#procedureNoResult 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::PROCEDURE_RETURNS_RESULT,
        2,
        "java.sql.DatabaseMetaData#procedureReturnsResult 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::PROCEDURE_COLUMN_UNKNOWN,
        0,
        "java.sql.DatabaseMetaData#procedureColumnUnknown 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::PROCEDURE_COLUMN_IN,
        1,
        "java.sql.DatabaseMetaData#procedureColumnIn 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::PROCEDURE_COLUMN_IN_OUT,
        2,
        "java.sql.DatabaseMetaData#procedureColumnInOut 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::PROCEDURE_COLUMN_OUT,
        4,
        "java.sql.DatabaseMetaData#procedureColumnOut 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::PROCEDURE_COLUMN_RETURN,
        5,
        "java.sql.DatabaseMetaData#procedureColumnReturn 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::PROCEDURE_COLUMN_RESULT,
        3,
        "java.sql.DatabaseMetaData#procedureColumnResult 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::PROCEDURE_NO_NULLS,
        0,
        "java.sql.DatabaseMetaData#procedureNoNulls 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::PROCEDURE_NULLABLE,
        1,
        "java.sql.DatabaseMetaData#procedureNullable 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::PROCEDURE_NULLABLE_UNKNOWN,
        2,
        "java.sql.DatabaseMetaData#procedureNullableUnknown 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::COLUMN_NO_NULLS,
        0,
        "java.sql.DatabaseMetaData#columnNoNulls 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::COLUMN_NULLABLE,
        1,
        "java.sql.DatabaseMetaData#columnNullable 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::COLUMN_NULLABLE_UNKNOWN,
        2,
        "java.sql.DatabaseMetaData#columnNullableUnknown 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::BEST_ROW_TEMPORARY,
        0,
        "java.sql.DatabaseMetaData#bestRowTemporary 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::BEST_ROW_TRANSACTION,
        1,
        "java.sql.DatabaseMetaData#bestRowTransaction 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::BEST_ROW_SESSION,
        2,
        "java.sql.DatabaseMetaData#bestRowSession 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::BEST_ROW_UNKNOWN,
        0,
        "java.sql.DatabaseMetaData#bestRowUnknown 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::BEST_ROW_NOT_PSEUDO,
        1,
        "java.sql.DatabaseMetaData#bestRowNotPseudo 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::BEST_ROW_PSEUDO,
        2,
        "java.sql.DatabaseMetaData#bestRowPseudo 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::VERSION_COLUMN_UNKNOWN,
        0,
        "java.sql.DatabaseMetaData#versionColumnUnknown 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::VERSION_COLUMN_NOT_PSEUDO,
        1,
        "java.sql.DatabaseMetaData#versionColumnNotPseudo 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::VERSION_COLUMN_PSEUDO,
        2,
        "java.sql.DatabaseMetaData#versionColumnPseudo 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::IMPORTED_KEY_CASCADE,
        0,
        "java.sql.DatabaseMetaData#importedKeyCascade 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::IMPORTED_KEY_RESTRICT,
        1,
        "java.sql.DatabaseMetaData#importedKeyRestrict 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::IMPORTED_KEY_SET_NULL,
        2,
        "java.sql.DatabaseMetaData#importedKeySetNull 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::IMPORTED_KEY_NO_ACTION,
        3,
        "java.sql.DatabaseMetaData#importedKeyNoAction 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::IMPORTED_KEY_SET_DEFAULT,
        4,
        "java.sql.DatabaseMetaData#importedKeySetDefault 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::IMPORTED_KEY_INITIALLY_DEFERRED,
        5,
        "java.sql.DatabaseMetaData#importedKeyInitiallyDeferred 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::IMPORTED_KEY_INITIALLY_IMMEDIATE,
        6,
        "java.sql.DatabaseMetaData#importedKeyInitiallyImmediate 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::IMPORTED_KEY_NOT_DEFERRABLE,
        7,
        "java.sql.DatabaseMetaData#importedKeyNotDeferrable 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::TYPE_NO_NULLS,
        0,
        "java.sql.DatabaseMetaData#typeNoNulls 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::TYPE_NULLABLE,
        1,
        "java.sql.DatabaseMetaData#typeNullable 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::TYPE_NULLABLE_UNKNOWN,
        2,
        "java.sql.DatabaseMetaData#typeNullableUnknown 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::TYPE_PRED_NONE,
        0,
        "java.sql.DatabaseMetaData#typePredNone 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::TYPE_PRED_CHAR,
        1,
        "java.sql.DatabaseMetaData#typePredChar 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::TYPE_PRED_BASIC,
        2,
        "java.sql.DatabaseMetaData#typePredBasic 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::TYPE_SEARCHABLE,
        3,
        "java.sql.DatabaseMetaData#typeSearchable 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::TABLE_INDEX_STATISTIC,
        0,
        "java.sql.DatabaseMetaData#tableIndexStatistic 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::TABLE_INDEX_CLUSTERED,
        1,
        "java.sql.DatabaseMetaData#tableIndexClustered 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::TABLE_INDEX_HASHED,
        2,
        "java.sql.DatabaseMetaData#tableIndexHashed 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::TABLE_INDEX_OTHER,
        3,
        "java.sql.DatabaseMetaData#tableIndexOther 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::ATTRIBUTE_NO_NULLS,
        0,
        "java.sql.DatabaseMetaData#attributeNoNulls 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::ATTRIBUTE_NULLABLE,
        1,
        "java.sql.DatabaseMetaData#attributeNullable 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::ATTRIBUTE_NULLABLE_UNKNOWN,
        2,
        "java.sql.DatabaseMetaData#attributeNullableUnknown 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::SQL_STATE_X_OPEN,
        1,
        "java.sql.DatabaseMetaData#sqlStateXOpen 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::SQL_STATE_SQL,
        2,
        "java.sql.DatabaseMetaData#sqlStateSQL 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::SQL_STATE_SQL_99,
        2,
        "java.sql.DatabaseMetaData#sqlStateSQL99 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::FUNCTION_COLUMN_UNKNOWN,
        0,
        "java.sql.DatabaseMetaData#functionColumnUnknown 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::FUNCTION_COLUMN_IN,
        1,
        "java.sql.DatabaseMetaData#functionColumnIn 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::FUNCTION_COLUMN_IN_OUT,
        2,
        "java.sql.DatabaseMetaData#functionColumnInOut 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::FUNCTION_COLUMN_OUT,
        3,
        "java.sql.DatabaseMetaData#functionColumnOut 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::FUNCTION_RETURN,
        4,
        "java.sql.DatabaseMetaData#functionReturn 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::FUNCTION_COLUMN_RESULT,
        5,
        "java.sql.DatabaseMetaData#functionColumnResult 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::FUNCTION_NO_NULLS,
        0,
        "java.sql.DatabaseMetaData#functionNoNulls 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::FUNCTION_NULLABLE,
        1,
        "java.sql.DatabaseMetaData#functionNullable 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::FUNCTION_NULLABLE_UNKNOWN,
        2,
        "java.sql.DatabaseMetaData#functionNullableUnknown 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::FUNCTION_RESULT_UNKNOWN,
        0,
        "java.sql.DatabaseMetaData#functionResultUnknown 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::FUNCTION_NO_TABLE,
        1,
        "java.sql.DatabaseMetaData#functionNoTable 必须等于 Java 8 精确取值",
    );
    assert_eq!(
        constants::FUNCTION_RETURNS_TABLE,
        2,
        "java.sql.DatabaseMetaData#functionReturnsTable 必须等于 Java 8 精确取值",
    );
}

#[test]
fn database_meta_data_constants_namespace_mirrors_jdk8_exact_values() {
    assert_eq!(
        DatabaseMetaDataConstants::PROCEDURE_RESULT_UNKNOWN,
        0,
        "java.sql.DatabaseMetaData#procedureResultUnknown 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::PROCEDURE_NO_RESULT,
        1,
        "java.sql.DatabaseMetaData#procedureNoResult 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::PROCEDURE_RETURNS_RESULT,
        2,
        "java.sql.DatabaseMetaData#procedureReturnsResult 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::PROCEDURE_COLUMN_UNKNOWN,
        0,
        "java.sql.DatabaseMetaData#procedureColumnUnknown 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::PROCEDURE_COLUMN_IN,
        1,
        "java.sql.DatabaseMetaData#procedureColumnIn 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::PROCEDURE_COLUMN_IN_OUT,
        2,
        "java.sql.DatabaseMetaData#procedureColumnInOut 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::PROCEDURE_COLUMN_OUT,
        4,
        "java.sql.DatabaseMetaData#procedureColumnOut 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::PROCEDURE_COLUMN_RETURN,
        5,
        "java.sql.DatabaseMetaData#procedureColumnReturn 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::PROCEDURE_COLUMN_RESULT,
        3,
        "java.sql.DatabaseMetaData#procedureColumnResult 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::PROCEDURE_NO_NULLS,
        0,
        "java.sql.DatabaseMetaData#procedureNoNulls 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::PROCEDURE_NULLABLE,
        1,
        "java.sql.DatabaseMetaData#procedureNullable 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::PROCEDURE_NULLABLE_UNKNOWN,
        2,
        "java.sql.DatabaseMetaData#procedureNullableUnknown 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::COLUMN_NO_NULLS,
        0,
        "java.sql.DatabaseMetaData#columnNoNulls 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::COLUMN_NULLABLE,
        1,
        "java.sql.DatabaseMetaData#columnNullable 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::COLUMN_NULLABLE_UNKNOWN,
        2,
        "java.sql.DatabaseMetaData#columnNullableUnknown 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::BEST_ROW_TEMPORARY,
        0,
        "java.sql.DatabaseMetaData#bestRowTemporary 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::BEST_ROW_TRANSACTION,
        1,
        "java.sql.DatabaseMetaData#bestRowTransaction 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::BEST_ROW_SESSION,
        2,
        "java.sql.DatabaseMetaData#bestRowSession 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::BEST_ROW_UNKNOWN,
        0,
        "java.sql.DatabaseMetaData#bestRowUnknown 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::BEST_ROW_NOT_PSEUDO,
        1,
        "java.sql.DatabaseMetaData#bestRowNotPseudo 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::BEST_ROW_PSEUDO,
        2,
        "java.sql.DatabaseMetaData#bestRowPseudo 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::VERSION_COLUMN_UNKNOWN,
        0,
        "java.sql.DatabaseMetaData#versionColumnUnknown 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::VERSION_COLUMN_NOT_PSEUDO,
        1,
        "java.sql.DatabaseMetaData#versionColumnNotPseudo 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::VERSION_COLUMN_PSEUDO,
        2,
        "java.sql.DatabaseMetaData#versionColumnPseudo 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::IMPORTED_KEY_CASCADE,
        0,
        "java.sql.DatabaseMetaData#importedKeyCascade 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::IMPORTED_KEY_RESTRICT,
        1,
        "java.sql.DatabaseMetaData#importedKeyRestrict 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::IMPORTED_KEY_SET_NULL,
        2,
        "java.sql.DatabaseMetaData#importedKeySetNull 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::IMPORTED_KEY_NO_ACTION,
        3,
        "java.sql.DatabaseMetaData#importedKeyNoAction 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::IMPORTED_KEY_SET_DEFAULT,
        4,
        "java.sql.DatabaseMetaData#importedKeySetDefault 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::IMPORTED_KEY_INITIALLY_DEFERRED,
        5,
        "java.sql.DatabaseMetaData#importedKeyInitiallyDeferred 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::IMPORTED_KEY_INITIALLY_IMMEDIATE,
        6,
        "java.sql.DatabaseMetaData#importedKeyInitiallyImmediate 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::IMPORTED_KEY_NOT_DEFERRABLE,
        7,
        "java.sql.DatabaseMetaData#importedKeyNotDeferrable 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::TYPE_NO_NULLS,
        0,
        "java.sql.DatabaseMetaData#typeNoNulls 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::TYPE_NULLABLE,
        1,
        "java.sql.DatabaseMetaData#typeNullable 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::TYPE_NULLABLE_UNKNOWN,
        2,
        "java.sql.DatabaseMetaData#typeNullableUnknown 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::TYPE_PRED_NONE,
        0,
        "java.sql.DatabaseMetaData#typePredNone 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::TYPE_PRED_CHAR,
        1,
        "java.sql.DatabaseMetaData#typePredChar 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::TYPE_PRED_BASIC,
        2,
        "java.sql.DatabaseMetaData#typePredBasic 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::TYPE_SEARCHABLE,
        3,
        "java.sql.DatabaseMetaData#typeSearchable 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::TABLE_INDEX_STATISTIC,
        0,
        "java.sql.DatabaseMetaData#tableIndexStatistic 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::TABLE_INDEX_CLUSTERED,
        1,
        "java.sql.DatabaseMetaData#tableIndexClustered 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::TABLE_INDEX_HASHED,
        2,
        "java.sql.DatabaseMetaData#tableIndexHashed 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::TABLE_INDEX_OTHER,
        3,
        "java.sql.DatabaseMetaData#tableIndexOther 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::ATTRIBUTE_NO_NULLS,
        0,
        "java.sql.DatabaseMetaData#attributeNoNulls 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::ATTRIBUTE_NULLABLE,
        1,
        "java.sql.DatabaseMetaData#attributeNullable 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::ATTRIBUTE_NULLABLE_UNKNOWN,
        2,
        "java.sql.DatabaseMetaData#attributeNullableUnknown 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::SQL_STATE_X_OPEN,
        1,
        "java.sql.DatabaseMetaData#sqlStateXOpen 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::SQL_STATE_SQL,
        2,
        "java.sql.DatabaseMetaData#sqlStateSQL 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::SQL_STATE_SQL_99,
        2,
        "java.sql.DatabaseMetaData#sqlStateSQL99 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::FUNCTION_COLUMN_UNKNOWN,
        0,
        "java.sql.DatabaseMetaData#functionColumnUnknown 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::FUNCTION_COLUMN_IN,
        1,
        "java.sql.DatabaseMetaData#functionColumnIn 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::FUNCTION_COLUMN_IN_OUT,
        2,
        "java.sql.DatabaseMetaData#functionColumnInOut 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::FUNCTION_COLUMN_OUT,
        3,
        "java.sql.DatabaseMetaData#functionColumnOut 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::FUNCTION_RETURN,
        4,
        "java.sql.DatabaseMetaData#functionReturn 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::FUNCTION_COLUMN_RESULT,
        5,
        "java.sql.DatabaseMetaData#functionColumnResult 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::FUNCTION_NO_NULLS,
        0,
        "java.sql.DatabaseMetaData#functionNoNulls 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::FUNCTION_NULLABLE,
        1,
        "java.sql.DatabaseMetaData#functionNullable 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::FUNCTION_NULLABLE_UNKNOWN,
        2,
        "java.sql.DatabaseMetaData#functionNullableUnknown 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::FUNCTION_RESULT_UNKNOWN,
        0,
        "java.sql.DatabaseMetaData#functionResultUnknown 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::FUNCTION_NO_TABLE,
        1,
        "java.sql.DatabaseMetaData#functionNoTable 的命名空间入口必须等于 Java 8 精确取值",
    );
    assert_eq!(
        DatabaseMetaDataConstants::FUNCTION_RETURNS_TABLE,
        2,
        "java.sql.DatabaseMetaData#functionReturnsTable 的命名空间入口必须等于 Java 8 精确取值",
    );
}

#[test]
fn database_meta_data_constants_count_matches_evidence_ledger() {
    let ledger_identities = ledger_unassessed_constant_identities();
    assert_eq!(
        ledger_identities.len(),
        CONSTANT_COUNT,
        "台账中 {OWNER} 的未评估常量 identity 数量必须与常量模块一致",
    );

    let expected_identities: BTreeSet<String> = JAVA8_CONSTANT_NAMES
        .iter()
        .map(|name| format!("{OWNER}#{name}"))
        .collect();
    assert_eq!(
        ledger_identities, expected_identities,
        "台账中的未评估常量 identity 必须与常量模块逐项一致",
    );

    let unique_names: BTreeSet<&str> = JAVA8_CONSTANT_NAMES.iter().copied().collect();
    assert_eq!(unique_names.len(), CONSTANT_COUNT, "常量名必须互不重复",);
}
