//! 结果集诊断：typed_getters 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

/// 完整保留 BASE 强类型重载参数与顺序观察。对应 Java: java.sql.ResultSet#getXxx。
#[tokio::test]
async fn task6_all_strong_getter_overloads_preserve_raw_spi_argument_identity() {
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let probe = Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut result_set = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(strong_getter_probe::StrongGetterProbe::new(Arc::clone(
            &probe,
        ))),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    let shanghai = RdbcCalendar::new("Asia/Shanghai").unwrap();

    result_set
        .big_decimal(ColumnIndex::new(1).unwrap())
        .unwrap();
    result_set.big_decimal_by_label("amount").unwrap();
    result_set
        .big_decimal_with_scale(ColumnIndex::new(2).unwrap(), 7)
        .unwrap();
    result_set
        .big_decimal_by_label_with_scale("scaled_amount", 8)
        .unwrap();

    result_set
        .date(
            ColumnIndex::new(3).unwrap(),
            &RdbcCalendarArgument::Unspecified,
        )
        .unwrap();
    result_set
        .date_by_label("event_date", &RdbcCalendarArgument::Unspecified)
        .unwrap();
    result_set
        .date(
            ColumnIndex::new(4).unwrap(),
            &RdbcCalendarArgument::Specified(Some(shanghai.clone())),
        )
        .unwrap();
    result_set
        .date_by_label("nullable_date", &RdbcCalendarArgument::Specified(None))
        .unwrap();

    result_set
        .time(
            ColumnIndex::new(5).unwrap(),
            &RdbcCalendarArgument::Unspecified,
        )
        .unwrap();
    result_set
        .time_by_label("event_time", &RdbcCalendarArgument::Unspecified)
        .unwrap();
    result_set
        .time(
            ColumnIndex::new(6).unwrap(),
            &RdbcCalendarArgument::Specified(None),
        )
        .unwrap();
    result_set
        .time_by_label(
            "zoned_time",
            &RdbcCalendarArgument::Specified(Some(shanghai.clone())),
        )
        .unwrap();

    result_set
        .timestamp(
            ColumnIndex::new(7).unwrap(),
            &RdbcCalendarArgument::Unspecified,
        )
        .unwrap();
    result_set
        .timestamp_by_label("event_at", &RdbcCalendarArgument::Unspecified)
        .unwrap();
    result_set
        .timestamp(
            ColumnIndex::new(8).unwrap(),
            &RdbcCalendarArgument::Specified(Some(shanghai.clone())),
        )
        .unwrap();
    result_set
        .timestamp_by_label("nullable_at", &RdbcCalendarArgument::Specified(None))
        .unwrap();

    assert_eq!(
        result_set.reference(ColumnIndex::new(9).unwrap()).unwrap(),
        None
    );
    assert_eq!(result_set.reference_by_label("reference").unwrap(), None);
    assert_eq!(
        result_set.blob(ColumnIndex::new(10).unwrap()).unwrap(),
        None
    );
    assert_eq!(result_set.blob_by_label("binary_lob").unwrap(), None);
    assert_eq!(
        result_set.clob(ColumnIndex::new(11).unwrap()).unwrap(),
        None
    );
    assert_eq!(result_set.clob_by_label("character_lob").unwrap(), None);
    assert_eq!(
        result_set.array(ColumnIndex::new(12).unwrap()).unwrap(),
        None
    );
    assert_eq!(result_set.array_by_label("array_value").unwrap(), None);
    assert_eq!(result_set.url(ColumnIndex::new(13).unwrap()).unwrap(), None);
    assert_eq!(result_set.url_by_label("url_value").unwrap(), None);
    assert_eq!(
        result_set.row_id(ColumnIndex::new(14).unwrap()).unwrap(),
        None
    );
    assert_eq!(result_set.row_id_by_label("row_identifier").unwrap(), None);
    assert_eq!(
        result_set.n_clob(ColumnIndex::new(15).unwrap()).unwrap(),
        None
    );
    assert_eq!(result_set.n_clob_by_label("national_lob").unwrap(), None);
    assert_eq!(
        result_set.sql_xml(ColumnIndex::new(16).unwrap()).unwrap(),
        None
    );
    assert_eq!(result_set.sql_xml_by_label("xml_value").unwrap(), None);
    result_set
        .update_reference(ColumnIndex::new(17).unwrap(), None)
        .unwrap();
    result_set
        .update_reference_by_label("reference_update", None)
        .unwrap();
    result_set
        .update_blob(ColumnIndex::new(18).unwrap(), None)
        .unwrap();
    result_set
        .update_blob_by_label("blob_update", None)
        .unwrap();
    result_set
        .update_clob(ColumnIndex::new(19).unwrap(), None)
        .unwrap();
    result_set
        .update_clob_by_label("clob_update", None)
        .unwrap();
    result_set
        .update_array(ColumnIndex::new(20).unwrap(), None)
        .unwrap();
    result_set
        .update_array_by_label("array_update", None)
        .unwrap();
    result_set
        .update_row_id(ColumnIndex::new(21).unwrap(), None)
        .unwrap();
    result_set
        .update_row_id_by_label("row_id_update", None)
        .unwrap();
    result_set
        .update_n_clob(ColumnIndex::new(22).unwrap(), None)
        .unwrap();
    result_set
        .update_n_clob_by_label("n_clob_update", None)
        .unwrap();
    result_set
        .update_sql_xml(ColumnIndex::new(23).unwrap(), None)
        .unwrap();
    result_set
        .update_sql_xml_by_label("sql_xml_update", None)
        .unwrap();
    result_set
        .update_blob_stream(
            ColumnIndex::new(24).unwrap(),
            None,
            RdbcStreamLength::Unspecified,
        )
        .unwrap();
    result_set
        .update_blob_stream_by_label("blob_stream", None, RdbcStreamLength::Unspecified)
        .unwrap();
    result_set
        .update_blob_stream(
            ColumnIndex::new(25).unwrap(),
            None,
            RdbcStreamLength::Long(101),
        )
        .unwrap();
    result_set
        .update_blob_stream_by_label("blob_stream_length", None, RdbcStreamLength::Long(102))
        .unwrap();
    result_set
        .update_clob_reader(
            ColumnIndex::new(26).unwrap(),
            None,
            RdbcCharacterLength::Unspecified,
        )
        .unwrap();
    result_set
        .update_clob_reader_by_label("clob_reader", None, RdbcCharacterLength::Unspecified)
        .unwrap();
    result_set
        .update_clob_reader(
            ColumnIndex::new(27).unwrap(),
            None,
            RdbcCharacterLength::Long(103),
        )
        .unwrap();
    result_set
        .update_clob_reader_by_label("clob_reader_length", None, RdbcCharacterLength::Long(104))
        .unwrap();
    result_set
        .update_n_clob_reader(
            ColumnIndex::new(28).unwrap(),
            None,
            RdbcCharacterLength::Unspecified,
        )
        .unwrap();
    result_set
        .update_n_clob_reader_by_label("n_clob_reader", None, RdbcCharacterLength::Unspecified)
        .unwrap();
    result_set
        .update_n_clob_reader(
            ColumnIndex::new(29).unwrap(),
            None,
            RdbcCharacterLength::Long(105),
        )
        .unwrap();
    result_set
        .update_n_clob_reader_by_label("n_clob_reader_length", None, RdbcCharacterLength::Long(106))
        .unwrap();

    assert_eq!(
        probe.lock().unwrap().clone(),
        vec![
            strong_getter_call::StrongGetterCall::BigDecimalIndex(1, None),
            strong_getter_call::StrongGetterCall::BigDecimalLabel("amount".to_string(), None),
            strong_getter_call::StrongGetterCall::BigDecimalIndex(2, Some(7)),
            strong_getter_call::StrongGetterCall::BigDecimalLabel(
                "scaled_amount".to_string(),
                Some(8)
            ),
            strong_getter_call::StrongGetterCall::DateIndex(3, RdbcCalendarArgument::Unspecified),
            strong_getter_call::StrongGetterCall::DateLabel(
                "event_date".to_string(),
                RdbcCalendarArgument::Unspecified
            ),
            strong_getter_call::StrongGetterCall::DateIndex(
                4,
                RdbcCalendarArgument::Specified(Some(shanghai.clone()))
            ),
            strong_getter_call::StrongGetterCall::DateLabel(
                "nullable_date".to_string(),
                RdbcCalendarArgument::Specified(None)
            ),
            strong_getter_call::StrongGetterCall::TimeIndex(5, RdbcCalendarArgument::Unspecified),
            strong_getter_call::StrongGetterCall::TimeLabel(
                "event_time".to_string(),
                RdbcCalendarArgument::Unspecified
            ),
            strong_getter_call::StrongGetterCall::TimeIndex(
                6,
                RdbcCalendarArgument::Specified(None)
            ),
            strong_getter_call::StrongGetterCall::TimeLabel(
                "zoned_time".to_string(),
                RdbcCalendarArgument::Specified(Some(shanghai.clone()))
            ),
            strong_getter_call::StrongGetterCall::TimestampIndex(
                7,
                RdbcCalendarArgument::Unspecified
            ),
            strong_getter_call::StrongGetterCall::TimestampLabel(
                "event_at".to_string(),
                RdbcCalendarArgument::Unspecified
            ),
            strong_getter_call::StrongGetterCall::TimestampIndex(
                8,
                RdbcCalendarArgument::Specified(Some(shanghai))
            ),
            strong_getter_call::StrongGetterCall::TimestampLabel(
                "nullable_at".to_string(),
                RdbcCalendarArgument::Specified(None)
            ),
            strong_getter_call::StrongGetterCall::RefIndex(9),
            strong_getter_call::StrongGetterCall::RefLabel("reference".to_string()),
            strong_getter_call::StrongGetterCall::BlobIndex(10),
            strong_getter_call::StrongGetterCall::BlobLabel("binary_lob".to_string()),
            strong_getter_call::StrongGetterCall::ClobIndex(11),
            strong_getter_call::StrongGetterCall::ClobLabel("character_lob".to_string()),
            strong_getter_call::StrongGetterCall::ArrayIndex(12),
            strong_getter_call::StrongGetterCall::ArrayLabel("array_value".to_string()),
            strong_getter_call::StrongGetterCall::UrlIndex(13),
            strong_getter_call::StrongGetterCall::UrlLabel("url_value".to_string()),
            strong_getter_call::StrongGetterCall::RowIdIndex(14),
            strong_getter_call::StrongGetterCall::RowIdLabel("row_identifier".to_string()),
            strong_getter_call::StrongGetterCall::NClobIndex(15),
            strong_getter_call::StrongGetterCall::NClobLabel("national_lob".to_string()),
            strong_getter_call::StrongGetterCall::SqlXmlIndex(16),
            strong_getter_call::StrongGetterCall::SqlXmlLabel("xml_value".to_string()),
            strong_getter_call::StrongGetterCall::UpdateRefIndex(17, false),
            strong_getter_call::StrongGetterCall::UpdateRefLabel(
                "reference_update".to_string(),
                false
            ),
            strong_getter_call::StrongGetterCall::UpdateBlobIndex(18, false),
            strong_getter_call::StrongGetterCall::UpdateBlobLabel("blob_update".to_string(), false),
            strong_getter_call::StrongGetterCall::UpdateClobIndex(19, false),
            strong_getter_call::StrongGetterCall::UpdateClobLabel("clob_update".to_string(), false),
            strong_getter_call::StrongGetterCall::UpdateArrayIndex(20, false),
            strong_getter_call::StrongGetterCall::UpdateArrayLabel(
                "array_update".to_string(),
                false
            ),
            strong_getter_call::StrongGetterCall::UpdateRowIdIndex(21, false),
            strong_getter_call::StrongGetterCall::UpdateRowIdLabel(
                "row_id_update".to_string(),
                false
            ),
            strong_getter_call::StrongGetterCall::UpdateNClobIndex(22, false),
            strong_getter_call::StrongGetterCall::UpdateNClobLabel(
                "n_clob_update".to_string(),
                false
            ),
            strong_getter_call::StrongGetterCall::UpdateSqlXmlIndex(23, false),
            strong_getter_call::StrongGetterCall::UpdateSqlXmlLabel(
                "sql_xml_update".to_string(),
                false
            ),
            strong_getter_call::StrongGetterCall::UpdateBlobStreamIndex(
                24,
                false,
                RdbcStreamLength::Unspecified
            ),
            strong_getter_call::StrongGetterCall::UpdateBlobStreamLabel(
                "blob_stream".to_string(),
                false,
                RdbcStreamLength::Unspecified
            ),
            strong_getter_call::StrongGetterCall::UpdateBlobStreamIndex(
                25,
                false,
                RdbcStreamLength::Long(101)
            ),
            strong_getter_call::StrongGetterCall::UpdateBlobStreamLabel(
                "blob_stream_length".to_string(),
                false,
                RdbcStreamLength::Long(102)
            ),
            strong_getter_call::StrongGetterCall::UpdateClobReaderIndex(
                26,
                false,
                RdbcCharacterLength::Unspecified
            ),
            strong_getter_call::StrongGetterCall::UpdateClobReaderLabel(
                "clob_reader".to_string(),
                false,
                RdbcCharacterLength::Unspecified
            ),
            strong_getter_call::StrongGetterCall::UpdateClobReaderIndex(
                27,
                false,
                RdbcCharacterLength::Long(103)
            ),
            strong_getter_call::StrongGetterCall::UpdateClobReaderLabel(
                "clob_reader_length".to_string(),
                false,
                RdbcCharacterLength::Long(104)
            ),
            strong_getter_call::StrongGetterCall::UpdateNClobReaderIndex(
                28,
                false,
                RdbcCharacterLength::Unspecified
            ),
            strong_getter_call::StrongGetterCall::UpdateNClobReaderLabel(
                "n_clob_reader".to_string(),
                false,
                RdbcCharacterLength::Unspecified
            ),
            strong_getter_call::StrongGetterCall::UpdateNClobReaderIndex(
                29,
                false,
                RdbcCharacterLength::Long(105)
            ),
            strong_getter_call::StrongGetterCall::UpdateNClobReaderLabel(
                "n_clob_reader_length".to_string(),
                false,
                RdbcCharacterLength::Long(106)
            ),
        ]
    );
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 通过动态标准句柄保留 Calendar/scale、Filter 顺序、映射和关闭优先级。
#[tokio::test]
async fn task6_strong_getters_preserve_filters_labels_and_governance() {
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut filters = crate::core::FilterChain::new();
    filters.add_result_set(Arc::new(strong_getter_filter::StrongGetterFilter::new(
        "a_before",
        "a_after",
        Arc::clone(&events),
        true,
    )));
    filters.add_result_set(Arc::new(strong_getter_filter::StrongGetterFilter::new(
        "b_before",
        "b_after",
        Arc::clone(&events),
        false,
    )));
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = statement
        .inner
        .resources
        .begin_admission(statement.inner.resources.current_generation())
        .unwrap();
    let mut rows = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(strong_getter_probe::StrongGetterProbe::new(Arc::clone(
            &calls,
        ))),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    let alias = DruidPooledResultSet::from_tracked_resource(
        Arc::clone(&statement.inner),
        Arc::clone(&rows.resource),
    )
    .unwrap();
    let mut standard: Box<dyn ResultSet> = Box::new(alias);
    assert_eq!(
        standard
            .big_decimal_by_label_with_scale("mapped", -7)
            .unwrap(),
        Some(druid_rdbc::types::RdbcBigDecimal::from(2))
    );
    assert_eq!(
        *events.lock().unwrap(),
        vec!["a_before", "b_before", "b_after", "a_after"]
    );
    assert_eq!(
        *calls.lock().unwrap(),
        vec![strong_getter_call::StrongGetterCall::BigDecimalLabel(
            "mapped".to_owned(),
            Some(-7)
        )]
    );
    events.lock().unwrap().clear();
    let calendar =
        RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("Asia/Shanghai").unwrap()));
    assert_eq!(
        standard
            .date(ColumnIndex::new(3).unwrap(), &calendar)
            .unwrap(),
        Some(druid_rdbc::types::RdbcNaiveDate::from_ymd_opt(2026, 7, 29).unwrap())
    );
    assert_eq!(
        calls.lock().unwrap().last(),
        Some(&strong_getter_call::StrongGetterCall::DateIndex(
            3,
            calendar.clone()
        ))
    );
    assert_eq!(
        *events.lock().unwrap(),
        vec!["a_before", "b_before", "b_after", "a_after"]
    );
    events.lock().unwrap().clear();
    // 资源读取不进入标量 Filter，保留既有对象路径与资源身份。
    assert!(standard.blob_by_label("native_blob").unwrap().is_none());
    assert_eq!(
        calls.lock().unwrap().last(),
        Some(&strong_getter_call::StrongGetterCall::BlobLabel(
            "native_blob".to_owned()
        ))
    );
    assert!(events.lock().unwrap().is_empty());
    let before = calls.lock().unwrap().len();
    rows.set_hidden_columns(Some(vec![2]));
    assert!(
        matches!(standard.date_by_label("mapped", &calendar), Err(druid_rdbc::error::RdbcError::InvalidArgument(message)) if message == "column is hidden")
    );
    assert_eq!(calls.lock().unwrap().len(), before);
    assert!(events.lock().unwrap().is_empty());
    rows.set_hidden_columns(None);
    rows.set_logic_column_map(Some(std::collections::HashMap::from([(1, 2)])));
    rows.set_physical_column_map(Some(std::collections::HashMap::from([(2, 1)])));
    assert_eq!(
        standard
            .time_by_label("mapped", &RdbcCalendarArgument::Specified(None))
            .unwrap(),
        Some(druid_rdbc::types::RdbcNaiveTime::from_hms_opt(13, 14, 15).unwrap())
    );
    assert_eq!(
        calls.lock().unwrap().last(),
        Some(&strong_getter_call::StrongGetterCall::TimeLabel(
            "mapped".to_owned(),
            RdbcCalendarArgument::Specified(None)
        ))
    );
    assert_eq!(
        standard.big_decimal(ColumnIndex::new(1).unwrap()).unwrap(),
        Some(druid_rdbc::types::RdbcBigDecimal::from(2))
    );
    assert_eq!(
        calls.lock().unwrap().last(),
        Some(&strong_getter_call::StrongGetterCall::BigDecimalIndex(
            2, None
        ))
    );
    rows.set_logic_column_map(None);
    rows.set_physical_column_map(None);
    rows.set_logic_column_map(Some(std::collections::HashMap::from([(3, 4)])));
    let before = calls.lock().unwrap().len();
    assert!(
        matches!(standard.timestamp_by_label("mapped", &calendar), Err(druid_rdbc::error::RdbcError::InvalidArgument(message)) if message == "column is hidden")
    );
    assert_eq!(calls.lock().unwrap().len(), before);
    rows.set_logic_column_map(None);
    standard.close().await.unwrap();
    assert!(matches!(
        standard.big_decimal_by_label_with_scale("mapped", 8),
        Err(druid_rdbc::error::RdbcError::ConnectionDiscarded)
    ));
    assert_eq!(calls.lock().unwrap().len(), before);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 保留真实 SQLite typed getObject 全部目标和创建语句异常计数。
#[tokio::test]
async fn task6_sqlite_typed_get_object_converts_all_standard_scalar_targets() {
    use bigdecimal::BigDecimal;
    use chrono::{NaiveDate, NaiveTime};
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::statement::GeneratedKeys;
    use druid_rdbc::types::{RdbcObject, RdbcTargetType, Value};
    use std::str::FromStr;
    let (mut statement, _connection) = task6_sqlite_diagnostic_statement_with_filters(None).await;
    statement
        .execute_update(
            "CREATE TABLE typed_value (
                bool_text TEXT,
                byte_value INTEGER,
                short_value INTEGER,
                int_value INTEGER,
                long_value INTEGER,
                float_value REAL,
                double_value REAL,
                decimal_value DECIMAL,
                date_value DATE,
                time_value TIME,
                timestamp_value TIMESTAMP,
                text_value TEXT,
                bytes_value BLOB,
                null_value TEXT,
                overflow_byte INTEGER
            )",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    statement
        .execute_update(
            "INSERT INTO typed_value VALUES (
                '1', 127, 32000, 2147483647, 9223372036854775807,
                1.25, 2.5, '1234567890.123456789',
                '2026-07-29', '13:14:15.123456789',
                '2026-07-29 13:14:15.123456789',
                'druid', X'00FF', NULL, 256
            )",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    let mut result_set = statement
        .execute_query(
            "SELECT bool_text, byte_value, short_value, int_value, long_value,
                    float_value, double_value, decimal_value, date_value, time_value,
                    timestamp_value, text_value, bytes_value, null_value, overflow_byte
             FROM typed_value",
        )
        .await
        .unwrap();
    assert!(result_set.next().await.unwrap());

    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(1).unwrap(), &RdbcTargetType::Boolean)
            .unwrap(),
        RdbcObject::Boolean(true)
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(2).unwrap(), &RdbcTargetType::Byte)
            .unwrap(),
        RdbcObject::Byte(127)
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(3).unwrap(), &RdbcTargetType::Short)
            .unwrap(),
        RdbcObject::Short(32_000)
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(4).unwrap(), &RdbcTargetType::Integer)
            .unwrap(),
        RdbcObject::Integer(i32::MAX)
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(5).unwrap(), &RdbcTargetType::Long)
            .unwrap(),
        RdbcObject::Long(i64::MAX)
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(6).unwrap(), &RdbcTargetType::Float)
            .unwrap(),
        RdbcObject::Float(1.25)
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(7).unwrap(), &RdbcTargetType::Double)
            .unwrap(),
        RdbcObject::Double(2.5)
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(8).unwrap(), &RdbcTargetType::BigDecimal)
            .unwrap(),
        // SQLite NUMERIC affinity 以 IEEE-754 存储，保留原驱动返回的舍入值。
        RdbcObject::BigDecimal(BigDecimal::from_str("1234567890.1234567").unwrap())
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(9).unwrap(), &RdbcTargetType::Date)
            .unwrap(),
        RdbcObject::Date(NaiveDate::from_ymd_opt(2026, 7, 29).unwrap())
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(10).unwrap(), &RdbcTargetType::Time)
            .unwrap(),
        RdbcObject::Time(NaiveTime::from_hms_nano_opt(13, 14, 15, 123_456_789).unwrap())
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(11).unwrap(), &RdbcTargetType::Timestamp)
            .unwrap(),
        RdbcObject::Timestamp(
            NaiveDate::from_ymd_opt(2026, 7, 29)
                .unwrap()
                .and_hms_nano_opt(13, 14, 15, 123_456_789)
                .unwrap()
        )
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(4).unwrap(), &RdbcTargetType::String)
            .unwrap(),
        RdbcObject::String(i32::MAX.to_string())
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(12).unwrap(), &RdbcTargetType::Bytes)
            .unwrap(),
        RdbcObject::Bytes(b"druid".to_vec())
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(13).unwrap(), &RdbcTargetType::Bytes)
            .unwrap(),
        RdbcObject::Bytes(vec![0, 255])
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(14).unwrap(), &RdbcTargetType::String)
            .unwrap(),
        RdbcObject::Scalar(Value::Null)
    );
    assert!(result_set.was_null().unwrap());

    let error = result_set
        .object_as(ColumnIndex::new(15).unwrap(), &RdbcTargetType::Byte)
        .unwrap_err();
    assert_eq!(error.class_name(), "java.sql.SQLDataException");
    assert_eq!(error.sql_exception().unwrap().sql_state(), Some("22003"));
    assert!(matches!(
        result_set.object_as(
            ColumnIndex::new(12).unwrap(),
            &RdbcTargetType::Custom("vendor.Type".to_string()),
        ),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_get_object_typed_custom"
        })
    ));
    assert_eq!(statement.exception_count(), 2);
}

/// 保留原四列 BufferedRowSet 通过真实创建语句和池化代理的全部列名重载。
#[tokio::test]
async fn task6_labeled_result_set_preserves_all_scalar_and_stream_overloads() {
    use druid_rdbc::extensions::BufferedRowSet;
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::{Row, Value};
    let (statement, _connection) = task6_sqlite_diagnostic_statement_with_filters(None).await;
    let raw: Box<dyn ResultSet> = Box::new(BufferedRowSet::with_column_labels(
        vec![Row::new(vec![
            Value::Int(7),
            Value::Float(2.5),
            Value::Bytes(vec![9, 8]),
            Value::String("labelled".to_string()),
        ])],
        vec![
            "number".to_string(),
            "score".to_string(),
            "payload".to_string(),
            "label".to_string(),
        ],
    ));
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut result_set =
        DruidPooledResultSet::new_standard(Arc::clone(&statement.inner), raw, &admission)
            .await
            .unwrap();
    drop(admission);
    assert!(result_set.next().await.unwrap());
    assert_eq!(result_set.short_by_label("number").unwrap(), 7);
    assert_eq!(result_set.byte_by_label("number").unwrap(), 7);
    assert_eq!(result_set.float_by_label("score").unwrap(), 2.5);
    assert_eq!(
        result_set.bytes_by_label("payload").unwrap(),
        Some(vec![9, 8])
    );
    assert_eq!(
        result_set.n_string_by_label("label").unwrap(),
        Some("labelled".to_string())
    );
    assert_eq!(
        result_set
            .binary_stream_by_label("payload")
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        vec![9, 8]
    );
    assert_eq!(
        result_set
            .ascii_stream_by_label("label")
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"labelled"
    );
    assert_eq!(
        result_set
            .unicode_stream_by_label("label")
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"labelled"
    );
    assert_eq!(
        result_set
            .character_stream_by_label("label")
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "labelled"
    );
    assert_eq!(
        result_set
            .n_character_stream_by_label("label")
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "labelled"
    );
}
