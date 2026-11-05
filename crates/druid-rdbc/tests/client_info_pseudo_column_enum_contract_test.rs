//! `java.sql.ClientInfoStatus` 与 `java.sql.PseudoColumnUsage` 的枚举常量合同。
//!
//! JDK 8 的两个枚举各声明 4 个常量，并公开 `valueOf(String)` 与 `values()` 静态入口。
//! Rust 侧对应 `druid_rdbc::connection::ClientInfoStatus` 与
//! `druid_rdbc::metadata::PseudoColumnUsage` 两个等价枚举，变体与 Java 常量一一对应。
//!
//! 形态说明：`druid-rdbc` 没有 `values()`/`valueOf` 入口，因此 `values()` 的等价遍历由本
//! 测试文件内的全变体表承担；`valueOf` 同样没有独立入口，表内的名称解析是测试内的等价
//! 映射，不代表生产代码提供了该 API。常量名到 Rust 变体的绑定以本文件的对应表为准，
//! 并通过规范源文件的声明顺序与标准失败负载文本交叉验证。

use druid_rdbc::connection::ClientInfoStatus;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::PseudoColumnUsage;
use std::collections::{HashMap, HashSet};

/// Java 8 `java.sql.ClientInfoStatus` 常量名与 Rust 变体的对应表。
///
/// 顺序与 JDK 8 `ClientInfoStatus` 的声明顺序一致，即 Java `ordinal()` 与
/// `values()` 的顺序：REASON_UNKNOWN、REASON_UNKNOWN_PROPERTY、REASON_VALUE_INVALID、
/// REASON_VALUE_TRUNCATED。
const CLIENT_INFO_STATUS_CONSTANTS: [(&str, ClientInfoStatus); 4] = [
    ("REASON_UNKNOWN", ClientInfoStatus::ReasonUnknown),
    (
        "REASON_UNKNOWN_PROPERTY",
        ClientInfoStatus::ReasonUnknownProperty,
    ),
    ("REASON_VALUE_INVALID", ClientInfoStatus::ReasonValueInvalid),
    (
        "REASON_VALUE_TRUNCATED",
        ClientInfoStatus::ReasonValueTruncated,
    ),
];

/// Java 8 `java.sql.PseudoColumnUsage` 常量名与 Rust 变体的对应表。
///
/// 顺序与 JDK 8 `PseudoColumnUsage` 的声明顺序一致，即 Java `ordinal()` 与
/// `values()` 的顺序：SELECT_LIST_ONLY、WHERE_CLAUSE_ONLY、NO_USAGE_RESTRICTIONS、
/// USAGE_UNKNOWN。
const PSEUDO_COLUMN_USAGE_CONSTANTS: [(&str, PseudoColumnUsage); 4] = [
    ("SELECT_LIST_ONLY", PseudoColumnUsage::SelectListOnly),
    ("WHERE_CLAUSE_ONLY", PseudoColumnUsage::WhereClauseOnly),
    (
        "NO_USAGE_RESTRICTIONS",
        PseudoColumnUsage::NoUsageRestrictions,
    ),
    ("USAGE_UNKNOWN", PseudoColumnUsage::UsageUnknown),
];

/// `ClientInfoStatus` 规范源文件文本，用于核对变体声明顺序。
const CLIENT_INFO_STATUS_SOURCE: &str = include_str!("../src/connection/client_info_status.rs");

/// `PseudoColumnUsage` 规范源文件文本，用于核对变体声明顺序与常量名注释。
const PSEUDO_COLUMN_USAGE_SOURCE: &str = include_str!("../src/metadata/pseudo_column_usage.rs");

/// `values()` 等价遍历：返回 Java `ClientInfoStatus.values()` 覆盖的全部常量。
///
/// 对应 Java: `ClientInfoStatus#values()`；生产代码未提供该入口，等价遍历在测试内完成。
fn client_info_status_values() -> [(&'static str, ClientInfoStatus); 4] {
    CLIENT_INFO_STATUS_CONSTANTS
}

/// `values()` 等价遍历：返回 Java `PseudoColumnUsage.values()` 覆盖的全部常量。
///
/// 对应 Java: `PseudoColumnUsage#values()`；生产代码未提供该入口，等价遍历在测试内完成。
fn pseudo_column_usage_values() -> [(&'static str, PseudoColumnUsage); 4] {
    PSEUDO_COLUMN_USAGE_CONSTANTS
}

/// `valueOf(String)` 等价解析：按 Java 常量名解析 `ClientInfoStatus` 变体。
///
/// 对应 Java: `ClientInfoStatus#valueOf(java.lang.String)`；生产代码未提供该入口，
/// 解析由本文件的对应表承担；未知名称与大小写不符的名称都不解析。
fn client_info_status_value_of(name: &str) -> Option<ClientInfoStatus> {
    CLIENT_INFO_STATUS_CONSTANTS
        .iter()
        .find(|(constant, _)| *constant == name)
        .map(|(_, status)| *status)
}

/// `valueOf(String)` 等价解析：按 Java 常量名解析 `PseudoColumnUsage` 变体。
///
/// 对应 Java: `PseudoColumnUsage#valueOf(java.lang.String)`；生产代码未提供该入口，
/// 解析由本文件的对应表承担；未知名称与大小写不符的名称都不解析。
fn pseudo_column_usage_value_of(name: &str) -> Option<PseudoColumnUsage> {
    PSEUDO_COLUMN_USAGE_CONSTANTS
        .iter()
        .find(|(constant, _)| *constant == name)
        .map(|(_, usage)| *usage)
}

/// 在规范源文件中定位枚举变体的声明位置。
///
/// 只匹配缩进后的声明行，避免命中文档注释或其他标识符中的同名片段。
fn variant_declaration_position(source: &str, identifier: &str) -> usize {
    let declaration = format!("\n    {identifier},");
    source
        .find(&declaration)
        .unwrap_or_else(|| panic!("variant {identifier} is not declared in the canonical source"))
}

#[test]
fn client_info_status_constants_expose_one_distinct_rust_variant_each() {
    // 对应 Java: ClientInfoStatus#REASON_UNKNOWN、#REASON_UNKNOWN_PROPERTY、
    // #REASON_VALUE_INVALID、#REASON_VALUE_TRUNCATED
    let values = client_info_status_values();
    assert_eq!(values.len(), 4);
    for (index, (name, status)) in values.iter().enumerate() {
        // 常量名与 Rust 变体的绑定是双向唯一的。
        assert_eq!(client_info_status_value_of(name), Some(*status));
        for (other_index, (_, other)) in values.iter().enumerate() {
            assert_eq!(status == other, index == other_index);
        }
        // Rust 侧的 `as` 系列值即变体标识符文本，必须与 Java 常量名区分。
        assert_ne!(format!("{status:?}"), *name);
    }
    let names: Vec<&str> = values.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        names,
        vec![
            "REASON_UNKNOWN",
            "REASON_UNKNOWN_PROPERTY",
            "REASON_VALUE_INVALID",
            "REASON_VALUE_TRUNCATED",
        ]
    );
    let identifiers: Vec<String> = values
        .iter()
        .map(|(_, status)| format!("{status:?}"))
        .collect();
    assert_eq!(
        identifiers,
        vec![
            "ReasonUnknown".to_owned(),
            "ReasonUnknownProperty".to_owned(),
            "ReasonValueInvalid".to_owned(),
            "ReasonValueTruncated".to_owned(),
        ]
    );
}

#[test]
fn client_info_status_values_equivalent_traverses_exactly_four_constants() {
    // 对应 Java: ClientInfoStatus#values()
    let values = client_info_status_values();
    assert_eq!(values.len(), 4);
    let distinct: HashSet<ClientInfoStatus> = values.iter().map(|(_, status)| *status).collect();
    assert_eq!(distinct.len(), 4);
    // 遍历顺序即 Java `values()` 的声明顺序。
    let mut previous = 0usize;
    for (_, status) in client_info_status_values() {
        let position =
            variant_declaration_position(CLIENT_INFO_STATUS_SOURCE, &format!("{status:?}"));
        assert!(
            position > previous,
            "variant {status:?} is not declared in JDK 8 values() order"
        );
        previous = position;
    }
}

#[test]
fn client_info_status_value_of_equivalent_resolves_every_jdk_constant_name() {
    // 对应 Java: ClientInfoStatus#valueOf(java.lang.String)
    for (name, status) in client_info_status_values() {
        assert_eq!(client_info_status_value_of(name), Some(status));
    }
    // Java `valueOf` 对未知名称抛出 IllegalArgumentException，且大小写敏感。
    assert_eq!(
        client_info_status_value_of("REASON_NOT_A_JDK_CONSTANT"),
        None
    );
    assert_eq!(client_info_status_value_of("reason_unknown"), None);
    assert_eq!(client_info_status_value_of(""), None);
}

#[test]
fn client_info_status_java_constant_names_are_the_standard_failure_payload_text() {
    // Java 的常量名就是 SQLClientInfoException#getFailedProperties 中的状态文本，
    // Rust 侧同一契约用同一批字符串承载。
    for (name, _) in client_info_status_values() {
        let error = RdbcError::feature_not_supported("connection_set_client_info")
            .with_client_info_failure(HashMap::from([("ClientUser".to_owned(), name.to_owned())]));
        let failed = error
            .failed_client_info_properties()
            .expect("client info failure keeps the failed properties payload");
        assert_eq!(failed.get("ClientUser").map(String::as_str), Some(name));
    }
}

#[test]
fn pseudo_column_usage_constants_expose_one_distinct_rust_variant_each() {
    // 对应 Java: PseudoColumnUsage#NO_USAGE_RESTRICTIONS、#SELECT_LIST_ONLY、
    // #USAGE_UNKNOWN、#WHERE_CLAUSE_ONLY
    let values = pseudo_column_usage_values();
    assert_eq!(values.len(), 4);
    for (index, (name, usage)) in values.iter().enumerate() {
        // 常量名与 Rust 变体的绑定是双向唯一的。
        assert_eq!(pseudo_column_usage_value_of(name), Some(*usage));
        for (other_index, (_, other)) in values.iter().enumerate() {
            assert_eq!(usage == other, index == other_index);
        }
        assert_ne!(format!("{usage:?}"), *name);
    }
    let names: Vec<&str> = values.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        names,
        vec![
            "SELECT_LIST_ONLY",
            "WHERE_CLAUSE_ONLY",
            "NO_USAGE_RESTRICTIONS",
            "USAGE_UNKNOWN",
        ]
    );
    let identifiers: Vec<String> = values
        .iter()
        .map(|(_, usage)| format!("{usage:?}"))
        .collect();
    assert_eq!(
        identifiers,
        vec![
            "SelectListOnly".to_owned(),
            "WhereClauseOnly".to_owned(),
            "NoUsageRestrictions".to_owned(),
            "UsageUnknown".to_owned(),
        ]
    );
}

#[test]
fn pseudo_column_usage_values_equivalent_traverses_exactly_four_constants() {
    // 对应 Java: PseudoColumnUsage#values()
    let values = pseudo_column_usage_values();
    assert_eq!(values.len(), 4);
    let distinct: HashSet<PseudoColumnUsage> = values.iter().map(|(_, usage)| *usage).collect();
    assert_eq!(distinct.len(), 4);
    // 遍历顺序即 Java `values()` 的声明顺序。
    let mut previous = 0usize;
    for (_, usage) in pseudo_column_usage_values() {
        let position =
            variant_declaration_position(PSEUDO_COLUMN_USAGE_SOURCE, &format!("{usage:?}"));
        assert!(
            position > previous,
            "variant {usage:?} is not declared in JDK 8 values() order"
        );
        previous = position;
    }
}

#[test]
fn pseudo_column_usage_value_of_equivalent_resolves_every_jdk_constant_name() {
    // 对应 Java: PseudoColumnUsage#valueOf(java.lang.String)
    for (name, usage) in pseudo_column_usage_values() {
        assert_eq!(pseudo_column_usage_value_of(name), Some(usage));
    }
    // Java `valueOf` 对未知名称抛出 IllegalArgumentException，且大小写敏感。
    assert_eq!(
        pseudo_column_usage_value_of("USAGE_NOT_A_JDK_CONSTANT"),
        None
    );
    assert_eq!(pseudo_column_usage_value_of("select_list_only"), None);
    assert_eq!(pseudo_column_usage_value_of(""), None);
}

#[test]
fn pseudo_column_usage_source_documents_every_java_constant_name() {
    // 规范源文件在每个变体的文档注释中标注对应的 Java 常量名。
    for (name, usage) in pseudo_column_usage_values() {
        let documented = format!("`{name}`");
        assert!(
            PSEUDO_COLUMN_USAGE_SOURCE.contains(&documented),
            "constant {name} is not documented next to variant {usage:?}"
        );
    }
}

#[test]
fn canonical_enum_types_are_distinct_stable_and_value_like() {
    // 对应 Java: ClientInfoStatus#TYPE、PseudoColumnUsage#TYPE（Class 对象）
    // Rust 侧没有 Class 对象，类型级入口就是这两个枚举声明本身。
    let client_info = std::any::TypeId::of::<ClientInfoStatus>();
    let pseudo_column = std::any::TypeId::of::<PseudoColumnUsage>();
    assert_ne!(client_info, pseudo_column);
    assert_eq!(
        std::any::type_name::<ClientInfoStatus>(),
        "druid_rdbc::connection::client_info_status::ClientInfoStatus"
    );
    assert_eq!(
        std::any::type_name::<PseudoColumnUsage>(),
        "druid_rdbc::metadata::pseudo_column_usage::PseudoColumnUsage"
    );
    // 两个枚举都是无负载的可复制值类型：只占判别标签的 1 字节，没有附加数据。
    let status = ClientInfoStatus::ReasonValueTruncated;
    let copied_status = status;
    assert_eq!(status, copied_status);
    let usage = PseudoColumnUsage::NoUsageRestrictions;
    let copied_usage = usage;
    assert_eq!(usage, copied_usage);
    assert_eq!(std::mem::size_of::<ClientInfoStatus>(), 1);
    assert_eq!(std::mem::size_of::<PseudoColumnUsage>(), 1);
}
