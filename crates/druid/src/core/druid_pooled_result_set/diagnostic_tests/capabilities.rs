//! 结果集诊断：capabilities 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

/// 完整保留原34个稀疏调用点及创建语句异常计数。
/// 对应 Java: DruidPooledResultSet 的 checkException 路径。
#[tokio::test]
async fn task6_pooled_result_set_classifies_every_sparse_capability_at_the_java_call_site() {
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::statement::FetchDirection;
    use std::sync::atomic::AtomicUsize;
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let closes = Arc::new(AtomicUsize::new(0));
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut result_set = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(sparse_result_set::SparseResultSet::new(
            Arc::clone(&calls),
            Arc::clone(&closes),
        )),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    // 原包装不得预读不支持的 metadata；能力错误应在对应调用点出现。
    assert!(calls.lock().unwrap().is_empty());
    macro_rules! unsupported {
        ($call:expr, $operation:literal) => {
            assert!(matches!($call, Err(RdbcError::UnsupportedOperation { operation }) if operation == $operation), "expected unsupported operation {}", $operation);
        };
    }
    unsupported!(result_set.next().await, "result_set_next");
    unsupported!(result_set.previous().await, "result_set_previous");
    unsupported!(result_set.is_before_first(), "result_set_is_before_first");
    unsupported!(result_set.is_after_last(), "result_set_is_after_last");
    unsupported!(result_set.is_first(), "result_set_is_first");
    unsupported!(result_set.is_last(), "result_set_is_last");
    unsupported!(result_set.before_first().await, "result_set_before_first");
    unsupported!(result_set.after_last().await, "result_set_after_last");
    unsupported!(result_set.first().await, "result_set_first");
    unsupported!(result_set.last().await, "result_set_last");
    unsupported!(result_set.row(), "result_set_row");
    unsupported!(result_set.absolute(-2).await, "result_set_absolute");
    unsupported!(result_set.relative(3).await, "result_set_relative");
    unsupported!(
        result_set.set_fetch_direction(FetchDirection::Forward),
        "result_set_set_fetch_direction"
    );
    unsupported!(result_set.fetch_direction(), "result_set_fetch_direction");
    unsupported!(result_set.set_fetch_size(10), "result_set_set_fetch_size");
    unsupported!(result_set.fetch_size(), "result_set_fetch_size");
    unsupported!(result_set.result_set_type(), "result_set_type");
    unsupported!(result_set.concurrency(), "result_set_concurrency");
    unsupported!(result_set.holdability(), "result_set_holdability");
    unsupported!(result_set.warnings(), "result_set_warnings");
    unsupported!(result_set.clear_warnings(), "result_set_clear_warnings");
    unsupported!(result_set.cursor_name(), "result_set_cursor_name");
    unsupported!(result_set.meta_data(), "result_set_meta_data");
    unsupported!(result_set.row_updated(), "result_set_row_updated");
    unsupported!(result_set.row_inserted(), "result_set_row_inserted");
    unsupported!(result_set.row_deleted(), "result_set_row_deleted");
    unsupported!(result_set.insert_row(), "result_set_insert_row");
    unsupported!(result_set.update_row(), "result_set_update_row");
    unsupported!(result_set.delete_row(), "result_set_delete_row");
    unsupported!(result_set.refresh_row(), "result_set_refresh_row");
    unsupported!(
        result_set.cancel_row_updates(),
        "result_set_cancel_row_updates"
    );
    unsupported!(
        result_set.move_to_insert_row(),
        "result_set_move_to_insert_row"
    );
    unsupported!(
        result_set.move_to_current_row(),
        "result_set_move_to_current_row"
    );
    assert_eq!(statement.exception_count(), 34);
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[
            "result_set_next",
            "result_set_previous",
            "result_set_is_before_first",
            "result_set_is_after_last",
            "result_set_is_first",
            "result_set_is_last",
            "result_set_before_first",
            "result_set_after_last",
            "result_set_first",
            "result_set_last",
            "result_set_row",
            "result_set_absolute",
            "result_set_relative",
            "result_set_set_fetch_direction",
            "result_set_fetch_direction",
            "result_set_set_fetch_size",
            "result_set_fetch_size",
            "result_set_type",
            "result_set_concurrency",
            "result_set_holdability",
            "result_set_warnings",
            "result_set_clear_warnings",
            "result_set_cursor_name",
            "result_set_meta_data",
            "result_set_row_updated",
            "result_set_row_inserted",
            "result_set_row_deleted",
            "result_set_insert_row",
            "result_set_update_row",
            "result_set_delete_row",
            "result_set_refresh_row",
            "result_set_cancel_row_updates",
            "result_set_move_to_insert_row",
            "result_set_move_to_current_row",
        ]
    );
    result_set.close().await.unwrap();
    assert_eq!(closes.load(Ordering::Acquire), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    statement.inner.resources.close_all().await.unwrap();
    assert_eq!(closes.load(Ordering::Acquire), 1);
    connection.close().await.unwrap();
}

/// 原19调用和精确错误，不能以通用值getter代替。
/// 对应 Java: DruidPooledResultSet#getXxx。
#[tokio::test]
async fn task6_pooled_scalar_getters_delegate_exact_overloads_and_classify_errors() {
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::{RdbcObject, Value};
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let failed_calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut result_set = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(
            scalar_getter_delegation_probe::ScalarGetterDelegationProbe::new(
                Arc::clone(&failed_calls),
                Some("boolean_by_label"),
            ),
        ),
        &admission,
    )
    .await
    .unwrap();
    assert!(
        matches!(result_set.boolean_by_label("failing"), Err(RdbcError::DriverError(message)) if message == "boolean_by_label failed")
    );
    assert_eq!(statement.exception_count(), 1);
    assert_eq!(*failed_calls.lock().unwrap(), ["boolean_by_label"]);
    failed_calls.lock().unwrap().clear();
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut successful_result_set = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(
            scalar_getter_delegation_probe::ScalarGetterDelegationProbe::new(
                Arc::clone(&calls),
                None,
            ),
        ),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    assert_eq!(
        successful_result_set.object_by_label("object").unwrap(),
        RdbcObject::Scalar(Value::Int(90))
    );
    assert_eq!(
        successful_result_set
            .string(ColumnIndex::new(1).unwrap())
            .unwrap()
            .as_deref(),
        Some("index")
    );
    assert_eq!(
        successful_result_set
            .string_by_label("string")
            .unwrap()
            .as_deref(),
        Some("label")
    );
    assert!(successful_result_set
        .boolean(ColumnIndex::new(2).unwrap())
        .unwrap());
    assert!(!successful_result_set.boolean_by_label("boolean").unwrap());
    assert_eq!(
        successful_result_set
            .long(ColumnIndex::new(3).unwrap())
            .unwrap(),
        101
    );
    assert_eq!(successful_result_set.long_by_label("long").unwrap(), 102);
    assert_eq!(
        successful_result_set
            .int(ColumnIndex::new(4).unwrap())
            .unwrap(),
        103
    );
    assert_eq!(successful_result_set.int_by_label("int").unwrap(), 104);
    assert_eq!(
        successful_result_set
            .short(ColumnIndex::new(5).unwrap())
            .unwrap(),
        105
    );
    assert_eq!(successful_result_set.short_by_label("short").unwrap(), 106);
    assert_eq!(
        successful_result_set
            .byte(ColumnIndex::new(6).unwrap())
            .unwrap(),
        107
    );
    assert_eq!(successful_result_set.byte_by_label("byte").unwrap(), 108);
    assert_eq!(
        successful_result_set
            .double(ColumnIndex::new(7).unwrap())
            .unwrap(),
        109.5
    );
    assert_eq!(
        successful_result_set.double_by_label("double").unwrap(),
        110.5
    );
    assert_eq!(
        successful_result_set
            .float(ColumnIndex::new(8).unwrap())
            .unwrap(),
        111.5
    );
    assert_eq!(
        successful_result_set.float_by_label("float").unwrap(),
        112.5
    );
    assert_eq!(
        successful_result_set
            .bytes(ColumnIndex::new(9).unwrap())
            .unwrap(),
        Some(vec![113])
    );
    assert_eq!(
        successful_result_set.bytes_by_label("bytes").unwrap(),
        Some(vec![114])
    );
    assert_eq!(calls.lock().unwrap().len(), 19);
    assert_eq!(
        *calls.lock().unwrap(),
        [
            "value_by_label",
            "string",
            "string_by_label",
            "boolean",
            "boolean_by_label",
            "long",
            "long_by_label",
            "int",
            "int_by_label",
            "short",
            "short_by_label",
            "byte",
            "byte_by_label",
            "double",
            "double_by_label",
            "float",
            "float_by_label",
            "bytes",
            "bytes_by_label"
        ]
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    successful_result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 验证原自定义对象类型、引用身份、Debug、全部显示和 SQL NULL 合同。
/// 对应 Java: java.sql.ResultSet#getObject 的厂商对象及平台标量。
#[test]
fn task6_rdbc_object_and_opaque_object_preserve_type_identity_and_display_contract() {
    use super::vendor_object_probe::VendorObjectProbe;
    use druid_rdbc::types::{RdbcObject, RdbcOpaqueObject, Value};

    let physical = Arc::new(VendorObjectProbe { id: 99 });
    let opaque = RdbcOpaqueObject::new(physical.clone());
    let same = opaque.clone();
    let other = RdbcOpaqueObject::new(Arc::new(VendorObjectProbe { id: 99 }));

    assert_eq!(opaque.class_name(), "com.example.VendorObject");
    assert_eq!(
        opaque.downcast_ref::<VendorObjectProbe>(),
        Some(&VendorObjectProbe { id: 99 })
    );
    assert!(opaque.downcast_ref::<String>().is_none());
    assert_eq!(opaque.value().class_name(), "com.example.VendorObject");
    assert!(format!("{opaque:?}").contains("com.example.VendorObject"));
    assert_eq!(opaque, same);
    assert_ne!(opaque, other);

    let values = [
        (RdbcObject::String("text".to_string()), "text"),
        (RdbcObject::Boolean(true), "true"),
        (RdbcObject::Byte(-1), "-1"),
        (RdbcObject::Short(-2), "-2"),
        (RdbcObject::Integer(-3), "-3"),
        (RdbcObject::Long(-4), "-4"),
        (RdbcObject::Float(1.25), "1.25"),
        (RdbcObject::Double(2.5), "2.5"),
        (RdbcObject::Bytes(vec![0, 1, 2]), "<3 bytes>"),
        (RdbcObject::Custom(opaque), "<com.example.VendorObject>"),
    ];
    for (value, expected) in values {
        assert_eq!(value.to_string(), expected);
        assert!(!value.is_null());
    }
    assert!(RdbcObject::from(Value::Null).is_null());
}

/// 保留警告四字段与两列元数据的类型、可空性及越界错误。
/// 对应 Java: java.sql.SQLWarning 与 java.sql.ResultSetMetaData。
#[test]
fn task6_sql_warning_and_result_set_metadata_preserve_structured_fields() {
    use druid_rdbc::error::{RdbcError, SqlWarning};
    use druid_rdbc::extensions::BufferedRowSet;
    use druid_rdbc::metadata::{ColumnNullability, ResultSetColumnType};
    use druid_rdbc::result_set::{ColumnIndex, ResultSet};
    use druid_rdbc::types::{Row, Value};

    let mut warning = SqlWarning::new("first", Some("01000".to_string()), 7);
    warning.set_next_warning(SqlWarning::new("second", None, 8));
    assert_eq!(warning.message(), "first");
    assert_eq!(warning.sql_state(), Some("01000"));
    assert_eq!(warning.error_code(), 7);
    assert_eq!(warning.next_warning().unwrap().message(), "second");

    let result_set = BufferedRowSet::with_column_labels(
        vec![Row::new(vec![Value::Bool(true), Value::Null])],
        vec!["enabled".to_string(), "optional".to_string()],
    );
    let meta_data = result_set.meta_data().unwrap();
    assert_eq!(
        meta_data
            .column_label(ColumnIndex::new(1).unwrap())
            .unwrap(),
        "enabled"
    );
    assert_eq!(
        meta_data.column_type(ColumnIndex::new(1).unwrap()).unwrap(),
        ResultSetColumnType::Boolean
    );
    assert_eq!(
        meta_data.column_type(ColumnIndex::new(2).unwrap()).unwrap(),
        ResultSetColumnType::Unknown
    );
    assert!(!meta_data
        .nullability(ColumnIndex::new(1).unwrap())
        .map(|value| matches!(value, ColumnNullability::Nullable))
        .unwrap());
    assert!(meta_data
        .nullability(ColumnIndex::new(2).unwrap())
        .map(|value| matches!(value, ColumnNullability::Nullable))
        .unwrap());
    assert!(matches!(
        (|| -> Result<String, RdbcError> { meta_data.column_label(ColumnIndex::new(0)?) })(),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(matches!(
        meta_data.column_type(ColumnIndex::new(3).unwrap()),
        Err(RdbcError::InvalidArgument(_))
    ));
}

/// 完整保留BASE SQLite更新拒绝58次、实际语句计数、后续游标和调用者资源。
/// 对应 Java: java.sql.ResultSet#updateXxx/getObject。
#[tokio::test]
async fn task6_sqlite_rejects_every_scalar_and_stream_update_without_corrupting_cursor() {
    use crate::core::RdbcError;
    use druid_rdbc::result_set::{ResultSetExt, ResultSetUpdate};
    use druid_rdbc::types::{
        RdbcBigDecimal, RdbcCharacterLength, RdbcInputStream, RdbcNaiveDate, RdbcNaiveDateTime,
        RdbcNaiveTime, RdbcObject, RdbcReader, RdbcStreamLength, RdbcTargetType, RdbcTypeMap,
    };

    // 与 BASE 同一 Toasty SQLite；私有入口保留实际创建语句统计，不伪造计数。
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let mut result_set = statement
        .execute_query("SELECT 1 AS id, 'first' AS label UNION ALL SELECT 2, 'second'")
        .await
        .unwrap();
    assert!(result_set.next().await.unwrap());
    let input = RdbcInputStream::from_bytes([1, 2, 3]);
    let reader = RdbcReader::from_string("严格 SQLite");

    macro_rules! assert_update_unsupported {
        ($expression:expr, $operation:literal) => {
            let previous_exception_count = statement.exception_count();
            let observed = $expression;
            assert!(
                matches!(
                    observed,
                    Err(RdbcError::UnsupportedOperation {
                        operation: $operation
                    })
                ),
                "原重载必须准确拒绝：{} => {:?}",
                stringify!($expression),
                observed
            );
            assert_eq!(
                statement.exception_count(),
                previous_exception_count + 1,
                "每次真实失败必须恰好记账一次：{}",
                stringify!($expression)
            );
        };
    }
    // 每家族仍分别执行下标1和原列名id；descriptor不读流、不预验长度。
    macro_rules! assert_pair_unsupported {
        ($variant:ident, $value:expr) => {
            assert_update_unsupported!(
                result_set.update_value(
                    ColumnIndex::new(1).unwrap(),
                    ResultSetUpdate::$variant($value)
                ),
                "result_set_update_value"
            );
            assert_update_unsupported!(
                result_set.update_value_by_label("id", ResultSetUpdate::$variant($value)),
                "result_set_update_value_by_label"
            );
        };
    }
    assert_update_unsupported!(
        result_set.update_value(ColumnIndex::new(1).unwrap(), ResultSetUpdate::Null),
        "result_set_update_value"
    );
    assert_update_unsupported!(
        result_set.update_value_by_label("id", ResultSetUpdate::Null),
        "result_set_update_value_by_label"
    );
    assert_pair_unsupported!(Boolean, true);
    assert_pair_unsupported!(Byte, 1);
    assert_pair_unsupported!(Short, 2);
    assert_pair_unsupported!(Int, 3);
    assert_pair_unsupported!(Long, 4);
    assert_pair_unsupported!(Float, 5.0);
    assert_pair_unsupported!(Double, 6.0);
    assert_pair_unsupported!(BigDecimal, None::<RdbcBigDecimal>);
    assert_pair_unsupported!(String, None::<String>);
    assert_pair_unsupported!(Bytes, None::<Vec<u8>>);
    assert_pair_unsupported!(Date, None::<RdbcNaiveDate>);
    assert_pair_unsupported!(Time, None::<RdbcNaiveTime>);
    assert_pair_unsupported!(Timestamp, None::<RdbcNaiveDateTime>);
    assert_update_unsupported!(
        result_set.update_value(
            ColumnIndex::new(1).unwrap(),
            ResultSetUpdate::Object(RdbcObject::from(druid_rdbc::types::Value::Null))
        ),
        "result_set_update_value"
    );
    assert_update_unsupported!(
        result_set.update_value_by_label(
            "id",
            ResultSetUpdate::Object(RdbcObject::from(druid_rdbc::types::Value::Null))
        ),
        "result_set_update_value_by_label"
    );
    assert_update_unsupported!(
        result_set.update_value(
            ColumnIndex::new(1).unwrap(),
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: RdbcObject::from(druid_rdbc::types::Value::Null),
                scale_or_length: -1
            }
        ),
        "result_set_update_value"
    );
    assert_update_unsupported!(
        result_set.update_value_by_label(
            "id",
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: RdbcObject::from(druid_rdbc::types::Value::Null),
                scale_or_length: -1
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_pair_unsupported!(NString, None::<String>);

    macro_rules! assert_stream_family_unsupported {
        ($variant:ident) => {
            assert_update_unsupported!(
                result_set.update_value(
                    ColumnIndex::new(1).unwrap(),
                    ResultSetUpdate::$variant {
                        stream: Some(input.clone()),
                        length: RdbcStreamLength::Unspecified
                    }
                ),
                "result_set_update_value"
            );
            assert_update_unsupported!(
                result_set.update_value_by_label(
                    "id",
                    ResultSetUpdate::$variant {
                        stream: Some(input.clone()),
                        length: RdbcStreamLength::Unspecified
                    }
                ),
                "result_set_update_value_by_label"
            );
            assert_update_unsupported!(
                result_set.update_value(
                    ColumnIndex::new(1).unwrap(),
                    ResultSetUpdate::$variant {
                        stream: Some(input.clone()),
                        length: RdbcStreamLength::Int(-1)
                    }
                ),
                "result_set_update_value"
            );
            assert_update_unsupported!(
                result_set.update_value_by_label(
                    "id",
                    ResultSetUpdate::$variant {
                        stream: Some(input.clone()),
                        length: RdbcStreamLength::Int(-1)
                    }
                ),
                "result_set_update_value_by_label"
            );
            assert_update_unsupported!(
                result_set.update_value(
                    ColumnIndex::new(1).unwrap(),
                    ResultSetUpdate::$variant {
                        stream: Some(input.clone()),
                        length: RdbcStreamLength::Long(-1)
                    }
                ),
                "result_set_update_value"
            );
            assert_update_unsupported!(
                result_set.update_value_by_label(
                    "id",
                    ResultSetUpdate::$variant {
                        stream: Some(input.clone()),
                        length: RdbcStreamLength::Long(-1)
                    }
                ),
                "result_set_update_value_by_label"
            );
        };
    }
    assert_stream_family_unsupported!(AsciiStream);
    assert_stream_family_unsupported!(BinaryStream);
    macro_rules! assert_reader_family_unsupported {
        ($variant:ident) => {
            assert_update_unsupported!(
                result_set.update_value(
                    ColumnIndex::new(1).unwrap(),
                    ResultSetUpdate::$variant {
                        reader: Some(reader.clone()),
                        length: RdbcCharacterLength::Unspecified
                    }
                ),
                "result_set_update_value"
            );
            assert_update_unsupported!(
                result_set.update_value_by_label(
                    "id",
                    ResultSetUpdate::$variant {
                        reader: Some(reader.clone()),
                        length: RdbcCharacterLength::Unspecified
                    }
                ),
                "result_set_update_value_by_label"
            );
            assert_update_unsupported!(
                result_set.update_value(
                    ColumnIndex::new(1).unwrap(),
                    ResultSetUpdate::$variant {
                        reader: Some(reader.clone()),
                        length: RdbcCharacterLength::Int(-1)
                    }
                ),
                "result_set_update_value"
            );
            assert_update_unsupported!(
                result_set.update_value_by_label(
                    "id",
                    ResultSetUpdate::$variant {
                        reader: Some(reader.clone()),
                        length: RdbcCharacterLength::Int(-1)
                    }
                ),
                "result_set_update_value_by_label"
            );
            assert_update_unsupported!(
                result_set.update_value(
                    ColumnIndex::new(1).unwrap(),
                    ResultSetUpdate::$variant {
                        reader: Some(reader.clone()),
                        length: RdbcCharacterLength::Long(-1)
                    }
                ),
                "result_set_update_value"
            );
            assert_update_unsupported!(
                result_set.update_value_by_label(
                    "id",
                    ResultSetUpdate::$variant {
                        reader: Some(reader.clone()),
                        length: RdbcCharacterLength::Long(-1)
                    }
                ),
                "result_set_update_value_by_label"
            );
        };
    }
    assert_reader_family_unsupported!(CharacterStream);
    assert_update_unsupported!(
        result_set.update_value(
            ColumnIndex::new(1).unwrap(),
            ResultSetUpdate::NCharacterStream {
                reader: Some(reader.clone()),
                length: RdbcCharacterLength::Unspecified
            }
        ),
        "result_set_update_value"
    );
    assert_update_unsupported!(
        result_set.update_value_by_label(
            "id",
            ResultSetUpdate::NCharacterStream {
                reader: Some(reader.clone()),
                length: RdbcCharacterLength::Unspecified
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_update_unsupported!(
        result_set.update_value(
            ColumnIndex::new(1).unwrap(),
            ResultSetUpdate::NCharacterStream {
                reader: Some(reader.clone()),
                length: RdbcCharacterLength::Long(-1)
            }
        ),
        "result_set_update_value"
    );
    assert_update_unsupported!(
        result_set.update_value_by_label(
            "id",
            ResultSetUpdate::NCharacterStream {
                reader: Some(reader.clone()),
                length: RdbcCharacterLength::Long(-1)
            }
        ),
        "result_set_update_value_by_label"
    );
    let mut type_map = RdbcTypeMap::new();
    type_map.insert("APP.USER_TYPE", RdbcTargetType::Custom("User".to_string()));
    assert_update_unsupported!(
        result_set.object_with_type_map(ColumnIndex::new(1).unwrap(), Some(&type_map)),
        "result_set_get_object_with_type_map"
    );
    assert_update_unsupported!(
        result_set.object_with_type_map_by_label("id", None),
        "result_set_get_object_by_label_with_type_map"
    );
    assert_eq!(statement.exception_count(), 58);
    assert!(result_set.next().await.unwrap());
    assert_eq!(result_set.int(ColumnIndex::new(1).unwrap()).unwrap(), 2);
    assert_eq!(
        result_set.string(ColumnIndex::new(2).unwrap()).unwrap(),
        Some("second".to_string())
    );
    assert!(!input.is_closed());
    assert!(!reader.is_closed());
    // 额外清理不替代前面的原始58次观察。
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
