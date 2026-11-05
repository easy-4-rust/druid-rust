//! `java.sql.ParameterMetaData` 完整契约测试。
//!
//! 覆盖 Java `java.sql.ParameterMetaData` 中尚未评估的条目：
//! - 方法：`getPrecision(int)`、`getScale(int)`、`isSigned(int)`、`isWrapperFor(Class<?>)`、`unwrap(Class<T>)`
//! - 常量：`parameterModeIn`、`parameterModeInOut`、`parameterModeOut`、`parameterModeUnknown`、
//!   `parameterNoNulls`、`parameterNullable`、`parameterNullableUnknown`
//!
//! Java `ParameterMetaData#TYPE` 常量在 Rust 标准层没有独立入口（类型编号由 `RdbcType` 映射承担），
//! 因此本文件不为它写断言。

use std::any::TypeId;

use druid::sql::{ParameterMetaData, ParameterMode, ParameterNullability, RdbcType};
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::ParameterIndex;
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper, WrapperExt};

/// 非 `ParameterMetaData` 的解包目标，用于验证 `isWrapperFor` / `unwrap` 的负例。
struct UnsupportedUnwrapTarget;

/// `druid::sql::ParameterMetaData` 的 canonical 标准接口（Rust 侧为 trait object）。
type CanonicalParameterMetaData = dyn druid_rdbc::metadata::ParameterMetaData;

// ── getPrecision(int) ──────────────────────────────────────────

#[test]
fn parameter_meta_data_full_contract_get_precision_reports_unknown_as_zero() {
    let meta = ParameterMetaData::new(vec![
        RdbcType::Integer,
        RdbcType::Decimal,
        RdbcType::VarChar,
    ]);
    // RDBC 元数据没有驱动级精度信息时，precision 回落到 0（Java 侧同样是 unknown 语义）。
    assert_eq!(meta.get_precision(1), Some(0));
    assert_eq!(meta.get_precision(2), Some(0));
    assert_eq!(meta.get_precision(3), Some(0));
    // snake_case 别名与 get_ 形式必须一致。
    assert_eq!(meta.precision(1), meta.get_precision(1));
    assert_eq!(meta.precision(2), meta.get_precision(2));
    assert_eq!(meta.precision(3), meta.get_precision(3));
}

#[test]
fn parameter_meta_data_full_contract_get_precision_rejects_out_of_range_position() {
    let meta = ParameterMetaData::new(vec![RdbcType::Integer, RdbcType::VarChar]);
    // 1-based 越界保护：0 与超过参数个数的下标都返回 None。
    assert_eq!(meta.get_precision(0), None);
    assert_eq!(meta.get_precision(3), None);
    assert_eq!(meta.get_precision(99), None);
    assert_eq!(meta.precision(0), None);
    assert_eq!(meta.precision(99), None);
}

// ── getScale(int) ──────────────────────────────────────────────

#[test]
fn parameter_meta_data_full_contract_get_scale_reports_unknown_as_zero() {
    let meta = ParameterMetaData::new(vec![
        RdbcType::Integer,
        RdbcType::Decimal,
        RdbcType::VarChar,
    ]);
    // 驱动没有提供 scale 明细时，scale 回落到 0。
    assert_eq!(meta.get_scale(1), Some(0));
    assert_eq!(meta.get_scale(2), Some(0));
    assert_eq!(meta.get_scale(3), Some(0));
    // snake_case 别名与 get_ 形式必须一致。
    assert_eq!(meta.scale(1), meta.get_scale(1));
    assert_eq!(meta.scale(2), meta.get_scale(2));
    assert_eq!(meta.scale(3), meta.get_scale(3));
}

#[test]
fn parameter_meta_data_full_contract_get_scale_rejects_out_of_range_position() {
    let meta = ParameterMetaData::new(vec![RdbcType::Integer, RdbcType::VarChar]);
    assert_eq!(meta.get_scale(0), None);
    assert_eq!(meta.get_scale(3), None);
    assert_eq!(meta.get_scale(99), None);
    assert_eq!(meta.scale(0), None);
    assert_eq!(meta.scale(99), None);
}

// ── isSigned(int) ──────────────────────────────────────────────

#[test]
fn parameter_meta_data_full_contract_is_signed_classifies_numeric_parameters() {
    let meta = ParameterMetaData::new(vec![
        RdbcType::TinyInt,
        RdbcType::SmallInt,
        RdbcType::Integer,
        RdbcType::BigInt,
        RdbcType::Float,
        RdbcType::Real,
        RdbcType::Double,
        RdbcType::Numeric,
        RdbcType::Decimal,
    ]);
    for position in 1..=9 {
        assert_eq!(
            meta.is_signed(position),
            Some(true),
            "第 {position} 个参数是有符号数值类型"
        );
    }
}

#[test]
fn parameter_meta_data_full_contract_is_signed_reports_non_numeric_as_unsigned() {
    let meta = ParameterMetaData::new(vec![
        RdbcType::VarChar,
        RdbcType::Boolean,
        RdbcType::Date,
        RdbcType::Binary,
    ]);
    assert_eq!(meta.is_signed(1), Some(false));
    assert_eq!(meta.is_signed(2), Some(false));
    assert_eq!(meta.is_signed(3), Some(false));
    assert_eq!(meta.is_signed(4), Some(false));
}

#[test]
fn parameter_meta_data_full_contract_is_signed_rejects_out_of_range_position() {
    let meta = ParameterMetaData::new(vec![RdbcType::Integer, RdbcType::VarChar]);
    assert_eq!(meta.is_signed(0), None);
    assert_eq!(meta.is_signed(3), None);
    assert_eq!(meta.is_signed(99), None);
}

// ── isWrapperFor(Class<?>) / unwrap(Class<T>) ──────────────────

#[test]
fn parameter_meta_data_full_contract_is_wrapper_for_matches_canonical_parameter_meta_data() {
    let meta = ParameterMetaData::new(vec![RdbcType::Integer, RdbcType::VarChar]);
    let canonical = TypeId::of::<CanonicalParameterMetaData>();
    assert!(
        Wrapper::is_wrapper_for(&meta, Some(canonical)).expect("canonical 目标必须可判定"),
        "ParameterMetaData 必须声明自己是 canonical ParameterMetaData"
    );
    assert!(
        Wrapper::is_wrapper_for(&meta, Some(TypeId::of::<dyn Wrapper>()))
            .expect("Wrapper 目标必须可判定"),
        "任何 Wrapper 实现都必须声明自己是 Wrapper"
    );
    assert!(
        !Wrapper::is_wrapper_for(&meta, Some(TypeId::of::<UnsupportedUnwrapTarget>()))
            .expect("无关目标必须可判定"),
        "ParameterMetaData 不是无关目标类型"
    );
    assert!(
        !Wrapper::is_wrapper_for(
            &meta,
            Some(TypeId::of::<dyn druid_rdbc::metadata::ResultSetMetaData>())
        )
        .expect("ResultSetMetaData 目标必须可判定"),
        "ParameterMetaData 不是 ResultSetMetaData"
    );
}

#[test]
fn parameter_meta_data_full_contract_is_wrapper_for_rejects_missing_target() {
    let meta = ParameterMetaData::new(vec![RdbcType::Integer]);
    assert!(matches!(
        Wrapper::is_wrapper_for(&meta, None),
        Err(RdbcError::InvalidArgument(_))
    ));
}

#[test]
fn parameter_meta_data_full_contract_unwrap_preserves_object_identity() {
    let meta = ParameterMetaData::new(vec![RdbcType::Integer, RdbcType::VarChar]);
    let canonical = TypeId::of::<CanonicalParameterMetaData>();
    let StandardUnwrapped::ParameterMetaData(leaf) =
        Wrapper::unwrap(&meta, Some(canonical)).expect("unwrap 必须返回 canonical 视图")
    else {
        panic!("unwrap 必须返回 canonical ParameterMetaData 叶接口");
    };
    // 解包结果与原对象是同一实例（Java unwrap 返回同一个底层对象）。
    let original: &CanonicalParameterMetaData = &meta;
    assert!(std::ptr::addr_eq(
        std::ptr::from_ref::<CanonicalParameterMetaData>(original),
        std::ptr::from_ref::<CanonicalParameterMetaData>(leaf)
    ));
    // 解包视图与原对象暴露同一份元数据。
    assert_eq!(leaf.parameter_count(), meta.parameter_count());
    let descriptor = leaf
        .parameter(ParameterIndex::new(1).expect("1 是合法的 1-based parameter 下标"))
        .expect("合法下标必须返回 descriptor");
    assert_eq!(
        descriptor.type_name(),
        meta.parameter_type_name(1).expect("Integer 必须有类型名")
    );
}

#[test]
fn parameter_meta_data_full_contract_unwrap_rejects_missing_and_unsupported_targets() {
    let meta = ParameterMetaData::new(vec![RdbcType::Integer]);
    assert!(matches!(
        Wrapper::unwrap(&meta, None),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(Wrapper::unwrap(&meta, Some(TypeId::of::<UnsupportedUnwrapTarget>())).is_err());
}

#[test]
fn parameter_meta_data_full_contract_concrete_wrapper_identity_is_preserved() {
    let meta = ParameterMetaData::new(vec![RdbcType::Integer, RdbcType::VarChar]);
    assert!(
        meta.is_wrapper_for_type::<ParameterMetaData>(),
        "concrete 类型必须能识别自身"
    );
    let concrete = meta
        .unwrap_ref::<ParameterMetaData>()
        .expect("unwrap_ref 必须返回同一 concrete 实例");
    assert_eq!(concrete, &meta);
}

// ── 常量：parameterMode* / parameter*Nulls* ────────────────────

#[test]
fn parameter_meta_data_full_contract_parameter_mode_constants_match_jdk_values() {
    // Java ParameterMetaData#parameterModeIn = 1
    assert_eq!(ParameterMode::In as i32, 1);
    // Java ParameterMetaData#parameterModeInOut = 2
    assert_eq!(ParameterMode::InOut as i32, 2);
    // Java ParameterMetaData#parameterModeOut = 4
    assert_eq!(ParameterMode::Out as i32, 4);
    // Java ParameterMetaData#parameterModeUnknown = 0
    assert_eq!(ParameterMode::Unknown as i32, 0);
    // mode 常量互不相同。
    assert_ne!(ParameterMode::In as i32, ParameterMode::InOut as i32);
    assert_ne!(ParameterMode::Out as i32, ParameterMode::Unknown as i32);
}

#[test]
fn parameter_meta_data_full_contract_parameter_nullability_constants_match_jdk_values() {
    // Java ParameterMetaData#parameterNoNulls = 0
    assert_eq!(ParameterNullability::NoNulls as i32, 0);
    // Java ParameterMetaData#parameterNullable = 1
    assert_eq!(ParameterNullability::Nullable as i32, 1);
    // Java ParameterMetaData#parameterNullableUnknown = 2
    assert_eq!(ParameterNullability::Unknown as i32, 2);
    // nullability 常量互不相同。
    assert_ne!(
        ParameterNullability::NoNulls as i32,
        ParameterNullability::Nullable as i32
    );
    assert_ne!(
        ParameterNullability::Nullable as i32,
        ParameterNullability::Unknown as i32
    );
}
