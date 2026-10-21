//! 结果集诊断：navigation_values 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

/// 单行结果集在反复前后移动后仍只报告一个历史抓取行。
/// 对应 Java: DruidPooledResultSetTest#test_rowCount。
#[tokio::test]
async fn java_single_row_navigation_preserves_fetch_peak_one() {
    // 对应 Java 1.2.28 DruidPooledResultSetTest#test_rowCount：
    // 返回同一行两次只增加游标访问，不把历史抓取峰值误计为两行。
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let cursor = statement.execute_query("SELECT 1 AS id").await.unwrap();
    drop(cursor);
    let resource = statement.current_result_set_for_diagnostics();
    let mut result_set =
        DruidPooledResultSet::from_tracked_resource(Arc::clone(&statement.inner), resource)
            .unwrap();

    assert!(result_set.next().await.unwrap());
    assert!(!result_set.next().await.unwrap());
    assert_eq!(result_set.filter_context.fetch_row_count(), 1);
    assert!(result_set.previous().await.unwrap());
    assert!(!result_set.previous().await.unwrap());
    assert_eq!(result_set.filter_context.fetch_row_count(), 1);
    assert!(result_set.next().await.unwrap());
    assert!(!result_set.next().await.unwrap());
    assert_eq!(result_set.filter_context.fetch_row_count(), 1);

    result_set.close().await.unwrap();
    assert_eq!(statement.fetch_row_peak(), 1);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 完整保留原六列值、创建语句身份、42次资源拒绝、双向游标与抓取峰值。
/// 对应 Java: DruidPooledResultSet / java.sql.ResultSet。
#[tokio::test]
async fn task6_sqlite_result_set_preserves_values_cursor_peak_and_wrapper_identity() {
    use crate::core::RdbcError;
    use druid_rdbc::metadata::{ColumnNullability, ResultSetColumnType};
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::statement::{CallableStatement, GeneratedKeys, PreparedStatement};
    use druid_rdbc::types::{RdbcCharacterLength, RdbcObject, RdbcStreamLength, Value};
    use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper, WrapperExt};
    use std::any::{Any, TypeId};

    // 同一真实 Toasty SQLite 创建语句，保留原六列、两行与所有观察。
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    statement
        .execute_update(
            "CREATE TABLE result_event (
                id INTEGER PRIMARY KEY,
                enabled INTEGER NOT NULL,
                score REAL NOT NULL,
                label TEXT NOT NULL,
                payload BLOB NOT NULL,
                optional_label TEXT
            )",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    statement
        .execute_update(
            "INSERT INTO result_event VALUES
                (1, 1, 1.5, 'first', X'0102', NULL),
                (2, 0, 2.5, 'second', X'0304', 'present')",
            GeneratedKeys::None,
        )
        .await
        .unwrap();

    let result_set = statement
        .execute_query(
            "SELECT id, enabled, score, label, payload, optional_label
             FROM result_event ORDER BY id",
        )
        .await
        .unwrap();

    drop(result_set);
    let resource = statement.current_result_set_for_diagnostics();
    let mut result_set =
        DruidPooledResultSet::from_tracked_resource(Arc::clone(&statement.inner), resource)
            .unwrap();
    assert_eq!(statement.fetch_row_peak(), -1);
    assert!(Arc::ptr_eq(&statement.inner, &result_set.statement.inner));
    assert!(Arc::ptr_eq(&statement.inner, &result_set.statement().inner));
    let mut creating = result_set.get_statement().unwrap().unwrap();
    assert!(
        !creating
            .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
            .unwrap(),
        "普通 Statement 结果集不能伪装成 PreparedStatement"
    );
    assert!(
        !creating
            .is_wrapper_for(Some(TypeId::of::<dyn CallableStatement>()))
            .unwrap(),
        "普通 Statement 结果集不能伪装成 CallableStatement"
    );
    // 标准 getStatement 返回的真实句柄必须共享原创建语句状态。
    creating.set_fetch_size(11).unwrap();
    assert_eq!(statement.fetch_size().unwrap(), 11);
    creating.set_fetch_size(0).unwrap();
    assert_eq!(statement.fetch_size().unwrap(), 0);
    assert!(result_set.is_wrapper_for_type::<DruidPooledResultSet>());
    assert!(result_set
        .is_wrapper_for(Some(TypeId::of::<dyn ResultSet>()))
        .unwrap());
    assert!((&result_set as &dyn Any).is::<DruidPooledResultSet>());
    assert!(std::ptr::eq(
        result_set.unwrap_ref::<DruidPooledResultSet>().unwrap(),
        std::ptr::from_ref(&result_set)
    ));
    assert!(
        matches!(result_set.unwrap(Some(TypeId::of::<dyn ResultSet>())).unwrap(),
        StandardUnwrapped::ResultSet(view) if std::ptr::addr_eq(std::ptr::from_ref::<dyn ResultSet>(view), std::ptr::from_ref::<dyn ResultSet>(&result_set)))
    );
    assert!(
        matches!(result_set.is_wrapper_for(None), Err(RdbcError::InvalidArgument(message))
        if message == "wrapper target must not be null")
    );
    assert!(
        matches!(result_set.unwrap(None), Err(RdbcError::InvalidArgument(message))
        if message == "wrapper target must not be null")
    );
    assert!(format!("{result_set:?}").contains("DruidPooledResultSet"));

    assert!(result_set.is_before_first().unwrap());
    assert_eq!(result_set.row().unwrap(), 0);
    assert_eq!(result_set.warnings().unwrap(), None);
    result_set.clear_warnings().unwrap();
    assert_eq!(result_set.cursor_name().unwrap(), None);
    let meta_data = result_set.meta_data().unwrap();
    assert_eq!(meta_data.column_count(), 6);
    assert_eq!(
        meta_data.column_type(ColumnIndex::new(1).unwrap()).unwrap(),
        ResultSetColumnType::Integer
    );
    assert_eq!(
        meta_data.column_type(ColumnIndex::new(3).unwrap()).unwrap(),
        ResultSetColumnType::Float
    );
    assert_eq!(
        meta_data.column_type(ColumnIndex::new(4).unwrap()).unwrap(),
        ResultSetColumnType::Text
    );
    assert_eq!(
        meta_data.column_type(ColumnIndex::new(5).unwrap()).unwrap(),
        ResultSetColumnType::Binary
    );
    assert!(matches!(
        meta_data.nullability(ColumnIndex::new(6).unwrap()).unwrap(),
        ColumnNullability::Nullable
    ));
    assert!(result_set.next().await.unwrap());
    assert_eq!(result_set.row().unwrap(), 1);
    assert_eq!(result_set.long(ColumnIndex::new(1).unwrap()).unwrap(), 1);
    assert_eq!(result_set.int(ColumnIndex::new(1).unwrap()).unwrap(), 1);
    assert_eq!(result_set.short(ColumnIndex::new(1).unwrap()).unwrap(), 1);
    assert_eq!(result_set.byte(ColumnIndex::new(1).unwrap()).unwrap(), 1);
    assert!(result_set.boolean(ColumnIndex::new(2).unwrap()).unwrap());
    assert_eq!(
        result_set.double(ColumnIndex::new(3).unwrap()).unwrap(),
        1.5
    );
    assert_eq!(result_set.float(ColumnIndex::new(3).unwrap()).unwrap(), 1.5);
    assert_eq!(
        result_set.string(ColumnIndex::new(4).unwrap()).unwrap(),
        Some("first".to_string())
    );
    assert_eq!(
        result_set.bytes(ColumnIndex::new(5).unwrap()).unwrap(),
        Some(vec![1, 2])
    );
    macro_rules! assert_sqlite_resource_unsupported {
        ($expression:expr, $operation:literal) => {
            let observed = $expression;
            assert!(
                matches!(
                    &observed,
                    Err(RdbcError::UnsupportedOperation {
                        operation: $operation
                    })
                ),
                "expected {}, observed {:?}",
                $operation,
                observed
            );
        };
    }
    assert_sqlite_resource_unsupported!(
        result_set.reference(ColumnIndex::new(1).unwrap()),
        "result_set_ref"
    );
    assert_sqlite_resource_unsupported!(
        result_set.reference_by_label("id"),
        "result_set_ref_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.blob(ColumnIndex::new(5).unwrap()),
        "result_set_blob"
    );
    assert_sqlite_resource_unsupported!(
        result_set.blob_by_label("payload"),
        "result_set_blob_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.clob(ColumnIndex::new(4).unwrap()),
        "result_set_clob"
    );
    assert_sqlite_resource_unsupported!(
        result_set.clob_by_label("label"),
        "result_set_clob_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.array(ColumnIndex::new(1).unwrap()),
        "result_set_array"
    );
    assert_sqlite_resource_unsupported!(
        result_set.array_by_label("id"),
        "result_set_array_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.url(ColumnIndex::new(4).unwrap()),
        "result_set_url"
    );
    assert_sqlite_resource_unsupported!(
        result_set.url_by_label("label"),
        "result_set_url_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.row_id(ColumnIndex::new(1).unwrap()),
        "result_set_row_id"
    );
    assert_sqlite_resource_unsupported!(
        result_set.row_id_by_label("id"),
        "result_set_row_id_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.n_clob(ColumnIndex::new(4).unwrap()),
        "result_set_n_clob"
    );
    assert_sqlite_resource_unsupported!(
        result_set.n_clob_by_label("label"),
        "result_set_n_clob_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.sql_xml(ColumnIndex::new(4).unwrap()),
        "result_set_sql_xml"
    );
    assert_sqlite_resource_unsupported!(
        result_set.sql_xml_by_label("label"),
        "result_set_sql_xml_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_reference(ColumnIndex::new(1).unwrap(), None),
        "result_set_update_ref"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_reference_by_label("id", None),
        "result_set_update_ref_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_blob(ColumnIndex::new(5).unwrap(), None),
        "result_set_update_blob"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_blob_by_label("payload", None),
        "result_set_update_blob_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_clob(ColumnIndex::new(4).unwrap(), None),
        "result_set_update_clob"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_clob_by_label("label", None),
        "result_set_update_clob_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_array(ColumnIndex::new(1).unwrap(), None),
        "result_set_update_array"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_array_by_label("id", None),
        "result_set_update_array_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_row_id(ColumnIndex::new(1).unwrap(), None),
        "result_set_update_row_id"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_row_id_by_label("id", None),
        "result_set_update_row_id_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_n_clob(ColumnIndex::new(4).unwrap(), None),
        "result_set_update_n_clob"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_n_clob_by_label("label", None),
        "result_set_update_n_clob_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_sql_xml(ColumnIndex::new(4).unwrap(), None),
        "result_set_update_sql_xml"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_sql_xml_by_label("label", None),
        "result_set_update_sql_xml_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_blob_stream(
            ColumnIndex::new(5).unwrap(),
            None,
            RdbcStreamLength::Unspecified
        ),
        "result_set_update_blob_stream"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_blob_stream_by_label("payload", None, RdbcStreamLength::Unspecified),
        "result_set_update_blob_stream_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_blob_stream(
            ColumnIndex::new(5).unwrap(),
            None,
            RdbcStreamLength::Long(1)
        ),
        "result_set_update_blob_stream"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_blob_stream_by_label("payload", None, RdbcStreamLength::Long(1)),
        "result_set_update_blob_stream_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_clob_reader(
            ColumnIndex::new(4).unwrap(),
            None,
            RdbcCharacterLength::Unspecified
        ),
        "result_set_update_clob_reader"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_clob_reader_by_label("label", None, RdbcCharacterLength::Unspecified),
        "result_set_update_clob_reader_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_clob_reader(
            ColumnIndex::new(4).unwrap(),
            None,
            RdbcCharacterLength::Long(1)
        ),
        "result_set_update_clob_reader"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_clob_reader_by_label("label", None, RdbcCharacterLength::Long(1)),
        "result_set_update_clob_reader_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_n_clob_reader(
            ColumnIndex::new(4).unwrap(),
            None,
            RdbcCharacterLength::Unspecified
        ),
        "result_set_update_n_clob_reader"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_n_clob_reader_by_label("label", None, RdbcCharacterLength::Unspecified),
        "result_set_update_n_clob_reader_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_n_clob_reader(
            ColumnIndex::new(4).unwrap(),
            None,
            RdbcCharacterLength::Long(1)
        ),
        "result_set_update_n_clob_reader"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_n_clob_reader_by_label("label", None, RdbcCharacterLength::Long(1)),
        "result_set_update_n_clob_reader_by_label"
    );
    assert_eq!(statement.exception_count(), 42);
    assert_eq!(
        result_set
            .binary_stream(ColumnIndex::new(5).unwrap())
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        vec![1, 2]
    );
    assert_eq!(
        result_set
            .ascii_stream(ColumnIndex::new(4).unwrap())
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"first"
    );
    assert_eq!(
        result_set
            .unicode_stream(ColumnIndex::new(4).unwrap())
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"first"
    );
    assert_eq!(
        result_set
            .character_stream(ColumnIndex::new(4).unwrap())
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "first"
    );
    assert_eq!(
        result_set
            .n_character_stream(ColumnIndex::new(4).unwrap())
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "first"
    );
    assert_eq!(
        result_set.n_string(ColumnIndex::new(4).unwrap()).unwrap(),
        Some("first".to_string())
    );
    assert_eq!(
        result_set.object(ColumnIndex::new(6).unwrap()).unwrap(),
        RdbcObject::from(Value::Null)
    );
    assert!(result_set.was_null().unwrap());
    assert!(result_set.is_first().unwrap());
    assert!(!result_set.row_updated().unwrap());
    assert!(!result_set.row_inserted().unwrap());
    assert!(!result_set.row_deleted().unwrap());

    for operation in [
        result_set.insert_row(),
        result_set.update_row(),
        result_set.delete_row(),
        result_set.refresh_row(),
        result_set.cancel_row_updates(),
        result_set.move_to_insert_row(),
        result_set.move_to_current_row(),
    ] {
        assert!(matches!(
            operation,
            Err(RdbcError::UnsupportedOperation { .. })
        ));
    }

    assert!(result_set.next().await.unwrap());
    assert!(result_set.is_last().unwrap());
    assert_eq!(result_set.filter_context.fetch_row_count(), 2);
    assert!(result_set.previous().await.unwrap());
    assert_eq!(result_set.filter_context.fetch_row_count(), 2);
    result_set.close().await.unwrap();
    assert!(result_set.is_closed().unwrap());
    assert_eq!(statement.fetch_row_peak(), 2);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 创建语句级联关闭同一真实SQLite游标，保留抓取峰值和原关闭错误。
/// 对应 Java: DruidPooledStatement#close 与 DruidPooledResultSet#next。
#[tokio::test]
async fn task6_statement_close_closes_traced_result_set_and_records_fetched_peak() {
    use crate::core::RdbcError;
    use druid_rdbc::result_set::ResultSet;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    /// 仅观察同一原生游标close后的实际状态，不合成关闭或替换执行后端。
    /// 对应 Java: ResultSet#close/isClosed。
    struct NativeCloseObserver {
        inner: Box<dyn ResultSet>,
        closed_observed: Arc<AtomicBool>,
        close_calls: Arc<AtomicUsize>,
        original_identity: usize,
    }
    druid_rdbc::impl_standard_wrapper!(NativeCloseObserver => ResultSet);
    #[async_trait::async_trait]
    impl ResultSet for NativeCloseObserver {
        fn known_row_count(&self) -> Option<u64> {
            self.inner.known_row_count()
        }
        async fn next(&mut self) -> Result<bool, RdbcError> {
            self.inner.next().await
        }
        async fn close(&mut self) -> Result<(), RdbcError> {
            assert_eq!(
                std::ptr::from_ref::<dyn ResultSet>(self.inner.as_ref()).cast::<()>() as usize,
                self.original_identity
            );
            self.close_calls.fetch_add(1, Ordering::AcqRel);
            self.inner.close().await?;
            self.closed_observed
                .store(self.inner.is_closed()?, Ordering::Release);
            Ok(())
        }
        fn is_closed(&self) -> Result<bool, RdbcError> {
            self.inner.is_closed()
        }
    }

    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let returned = statement
        .execute_query("SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3")
        .await
        .unwrap();
    drop(returned);
    let resource = statement.current_result_set_for_diagnostics();
    let closed_observed = Arc::new(AtomicBool::new(false));
    let close_calls = Arc::new(AtomicUsize::new(0));
    {
        let mut slot = resource.result_set.lock().await;
        let original = slot.take().expect("原查询创建的同一真实游标");
        let original_identity =
            std::ptr::from_ref::<dyn ResultSet>(original.as_ref()).cast::<()>() as usize;
        *slot = Some(Box::new(NativeCloseObserver {
            inner: original,
            closed_observed: Arc::clone(&closed_observed),
            close_calls: Arc::clone(&close_calls),
            original_identity,
        }));
    }
    let mut result_set =
        DruidPooledResultSet::from_tracked_resource(Arc::clone(&statement.inner), resource)
            .unwrap();
    assert!(result_set.next().await.unwrap());
    assert!(result_set.next().await.unwrap());
    statement.close().await.unwrap();
    assert!(statement.is_closed());
    assert!(result_set.is_closed().unwrap());
    assert!(closed_observed.load(Ordering::Acquire));
    assert_eq!(statement.fetch_row_peak(), 2);
    assert!(matches!(result_set.next().await,
        Err(RdbcError::Other(message)) if message == "result set is closed"));
    assert_eq!(close_calls.load(Ordering::Acquire), 1);
    statement.close().await.unwrap();
    result_set.close().await.unwrap();
    assert_eq!(close_calls.load(Ordering::Acquire), 1);
    connection.close().await.unwrap();
}

/// 保留完整 Decimal、日期、时间、时间戳及 Calendar 重载与 SQL NULL 合同。
/// 对应 Java: DruidPooledResultSet#getBigDecimal/getDate/getTime/getTimestamp。
#[tokio::test]
async fn task6_strong_typed_getters_preserve_all_decimal_temporal_and_calendar_overloads() {
    use bigdecimal::BigDecimal;
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    use druid_rdbc::extensions::BufferedRowSet;
    use druid_rdbc::metadata::ResultSetColumnType;
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::{RdbcCalendar, RdbcCalendarArgument, Row, Value};
    use std::str::FromStr;
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let decimal = BigDecimal::from_str("1234567890.45").unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_nano_opt(13, 14, 15, 123_456_789).unwrap();
    let timestamp = NaiveDateTime::new(date, time);
    let physical: Box<dyn ResultSet> = Box::new(BufferedRowSet::with_column_labels(
        vec![Row::new(vec![
            Value::Decimal(decimal.clone()),
            Value::Date(date),
            Value::Time(time),
            Value::Timestamp(timestamp),
            Value::Null,
        ])],
        vec![
            "amount".to_string(),
            "event_date".to_string(),
            "event_time".to_string(),
            "event_at".to_string(),
            "optional_amount".to_string(),
        ],
    ));
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut result_set =
        DruidPooledResultSet::new_standard(Arc::clone(&statement.inner), physical, &admission)
            .await
            .unwrap();
    drop(admission);
    assert!(result_set.next().await.unwrap());

    let meta_data = result_set.meta_data().unwrap();
    assert_eq!(
        meta_data.column_type(ColumnIndex::new(1).unwrap()).unwrap(),
        ResultSetColumnType::Decimal
    );
    assert_eq!(
        meta_data.column_type(ColumnIndex::new(2).unwrap()).unwrap(),
        ResultSetColumnType::Date
    );
    assert_eq!(
        meta_data.column_type(ColumnIndex::new(3).unwrap()).unwrap(),
        ResultSetColumnType::Time
    );
    assert_eq!(
        meta_data.column_type(ColumnIndex::new(4).unwrap()).unwrap(),
        ResultSetColumnType::Timestamp
    );

    assert_eq!(
        result_set
            .big_decimal(ColumnIndex::new(1).unwrap())
            .unwrap(),
        Some(decimal.clone())
    );
    assert_eq!(
        result_set.big_decimal_by_label("amount").unwrap(),
        Some(decimal.clone())
    );
    assert_eq!(
        result_set
            .big_decimal_with_scale(ColumnIndex::new(1).unwrap(), 2)
            .unwrap(),
        Some(decimal.clone())
    );
    assert_eq!(
        result_set
            .big_decimal_by_label_with_scale("amount", 2)
            .unwrap(),
        Some(decimal)
    );

    let shanghai = RdbcCalendar::new("Asia/Shanghai").unwrap();
    assert_eq!(
        result_set
            .date(
                ColumnIndex::new(2).unwrap(),
                &RdbcCalendarArgument::Unspecified
            )
            .unwrap(),
        Some(date)
    );
    assert_eq!(
        result_set
            .date_by_label("event_date", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(date)
    );
    assert_eq!(
        result_set
            .date(
                ColumnIndex::new(2).unwrap(),
                &RdbcCalendarArgument::Specified(Some(shanghai.clone()))
            )
            .unwrap(),
        Some(date)
    );
    assert_eq!(
        result_set
            .date_by_label("event_date", &RdbcCalendarArgument::Specified(None))
            .unwrap(),
        Some(date)
    );

    assert_eq!(
        result_set
            .time(
                ColumnIndex::new(3).unwrap(),
                &RdbcCalendarArgument::Unspecified
            )
            .unwrap(),
        Some(time)
    );
    assert_eq!(
        result_set
            .time_by_label("event_time", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(time)
    );
    assert_eq!(
        result_set
            .time(
                ColumnIndex::new(3).unwrap(),
                &RdbcCalendarArgument::Specified(None)
            )
            .unwrap(),
        Some(time)
    );
    assert_eq!(
        result_set
            .time_by_label(
                "event_time",
                &RdbcCalendarArgument::Specified(Some(shanghai.clone()))
            )
            .unwrap(),
        Some(time)
    );

    assert_eq!(
        result_set
            .timestamp(
                ColumnIndex::new(4).unwrap(),
                &RdbcCalendarArgument::Unspecified
            )
            .unwrap(),
        Some(timestamp)
    );
    assert_eq!(
        result_set
            .timestamp_by_label("event_at", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(timestamp)
    );
    assert_eq!(
        result_set
            .timestamp(
                ColumnIndex::new(4).unwrap(),
                &RdbcCalendarArgument::Specified(Some(shanghai))
            )
            .unwrap(),
        Some(timestamp)
    );
    assert_eq!(
        result_set
            .timestamp_by_label("event_at", &RdbcCalendarArgument::Specified(None))
            .unwrap(),
        Some(timestamp)
    );

    assert_eq!(
        result_set.big_decimal_by_label("optional_amount").unwrap(),
        None
    );
    assert!(result_set.was_null().unwrap());
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
