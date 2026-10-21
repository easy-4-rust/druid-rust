use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt, ResultSetUpdate};
use druid_rdbc::types::{
    RdbcCalendar, RdbcCalendarArgument, RdbcCharacterLength, RdbcInputStream, RdbcNaiveDate,
    RdbcNaiveTime, RdbcReader, RdbcStreamLength, Row, Value,
};

/// 默认 NString 保留 Unicode/NULL、严格类型、关闭与原定位错误的优先级。
/// 对应 Java: java.sql.ResultSet#getNString(int/String)。
#[tokio::test]
async fn buffered_n_string_defaults_preserve_unicode_null_strict_type_and_errors() {
    use druid_rdbc::result_set::ResultSetRead;
    use druid_rdbc::types::RdbcObject;
    let mut rows = BufferedRowSet::with_column_labels(
        vec![Row::new(vec![
            Value::String("你好，Druid😀".to_owned()),
            Value::Null,
            Value::Int(7),
        ])],
        vec!["unicode".to_owned(), "null".to_owned(), "int".to_owned()],
    );
    let column = ColumnIndex::new(1).unwrap();
    let before = rows.object(column).unwrap_err();
    assert_eq!(rows.n_string(column), Err(before.clone()));
    assert_eq!(rows.n_string_by_label("unicode"), Err(before));
    assert!(rows.next().await.unwrap());
    let standard: &dyn ResultSet = &rows;
    assert_eq!(
        standard.n_string(column).unwrap(),
        Some("你好，Druid😀".to_owned())
    );
    assert_eq!(
        standard.n_string_by_label("unicode").unwrap(),
        Some("你好，Druid😀".to_owned())
    );
    assert_eq!(
        standard
            .read_object(column, &ResultSetRead::NString)
            .unwrap(),
        RdbcObject::NString("你好，Druid😀".to_owned())
    );
    assert_eq!(standard.n_string_by_label("null").unwrap(), None);
    assert!(standard.was_null().unwrap());
    let wrong_type = standard.n_string_by_label("int").unwrap_err();
    let error = wrong_type.sql_exception().expect("SQLDataException");
    assert_eq!(error.class_name(), "java.sql.SQLDataException");
    assert_eq!(error.sql_state(), Some("22005"));
    assert!(!standard.was_null().unwrap());
    let missing = standard.find_column("missing").unwrap_err();
    assert_eq!(standard.n_string_by_label("missing"), Err(missing));
    rows.close().await.unwrap();
    let closed = rows.object(column).unwrap_err();
    assert_eq!(rows.n_string(column), Err(closed.clone()));
    assert_eq!(rows.n_string_by_label("unicode"), Err(closed));
}

/// 默认驱动保留 BASE scale 转换和已解码日期时间的 Calendar 重载行为。
/// 对应 Java: java.sql.ResultSet 的 decimal 和 Calendar getter 重载。
#[tokio::test]
async fn buffered_defaults_preserve_plain_values_scale_and_calendar_overloads() {
    let date = RdbcNaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = RdbcNaiveTime::from_hms_opt(13, 14, 15).unwrap();
    let timestamp = date.and_time(time);
    let mut rows = BufferedRowSet::with_column_labels(
        vec![Row::new(vec![
            Value::String("12.50".to_owned()),
            Value::Date(date),
            Value::Time(time),
            Value::Timestamp(timestamp),
        ])],
        vec![
            "amount".to_owned(),
            "date".to_owned(),
            "time".to_owned(),
            "timestamp".to_owned(),
        ],
    );
    assert!(rows.next().await.unwrap());
    let standard: &dyn ResultSet = &rows;
    assert_eq!(
        standard.big_decimal_by_label("amount").unwrap(),
        Some("12.50".parse().unwrap())
    );
    assert_eq!(
        standard
            .date_by_label("date", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(date)
    );
    assert_eq!(
        standard
            .time_by_label("time", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(time)
    );
    assert_eq!(
        standard
            .timestamp_by_label("timestamp", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(timestamp)
    );
    assert_eq!(
        standard
            .big_decimal_by_label_with_scale("amount", 1)
            .unwrap(),
        Some("12.5".parse().unwrap())
    );
    assert_eq!(
        standard
            .big_decimal_with_scale(ColumnIndex::new(1).unwrap(), -3)
            .unwrap(),
        Some("0".parse().unwrap())
    );
    for calendar in [
        RdbcCalendarArgument::Specified(None),
        RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("Asia/Shanghai").unwrap())),
    ] {
        assert_eq!(
            standard.date_by_label("date", &calendar).unwrap(),
            Some(date)
        );
        assert_eq!(
            standard.time_by_label("time", &calendar).unwrap(),
            Some(time)
        );
        assert_eq!(
            standard.timestamp_by_label("timestamp", &calendar).unwrap(),
            Some(timestamp)
        );
    }
    assert_eq!(standard.row().unwrap(), 1);
    assert_eq!(
        standard.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::String("12.50".to_owned())
    );
    assert!(!rows.next().await.unwrap());
}

/// 类型化资源 NULL 仍能更新缓冲值；不支持的流更新不得消费或关闭调用者资源。
/// 对应 Java: java.sql.ResultSet#updateRef/updateBlob/updateClob/updateArray/updateRowId/updateNClob/updateSQLXML。
#[tokio::test]
async fn buffered_resource_null_updates_preserve_state_and_stream_refusal_is_non_consuming() {
    let column = ColumnIndex::new(1).unwrap();
    let mut rows = BufferedRowSet::with_column_labels(
        vec![Row::new(vec![Value::Int(1)])],
        vec!["resource".to_owned()],
    );
    assert!(rows.next().await.unwrap());
    for update in [
        ResultSetUpdate::Ref(None),
        ResultSetUpdate::Blob(None),
        ResultSetUpdate::Clob(None),
        ResultSetUpdate::Array(None),
        ResultSetUpdate::RowId(None),
        ResultSetUpdate::NClob(None),
        ResultSetUpdate::SqlXml(None),
    ] {
        rows.update_value(column, ResultSetUpdate::Int(1)).unwrap();
        rows.update_value_by_label("resource", update).unwrap();
        assert_eq!(rows.value(column).unwrap(), Value::Null);
        assert!(rows.was_null().unwrap());
        assert!(rows.row_updated().unwrap());
    }
    rows.update_value(column, ResultSetUpdate::Int(42)).unwrap();
    let stream = RdbcInputStream::from_bytes(vec![1, 2, 3]);
    let reader = RdbcReader::from_string("A😀");
    assert_eq!(
        rows.update_blob_stream_by_label("resource", Some(&stream), RdbcStreamLength::Long(-1))
            .unwrap_err()
            .sql_exception()
            .unwrap()
            .sql_state(),
        Some("0A000")
    );
    assert_eq!(
        rows.update_clob_reader(column, Some(&reader), RdbcCharacterLength::Unspecified)
            .unwrap_err()
            .sql_exception()
            .unwrap()
            .sql_state(),
        Some("0A000")
    );
    assert_eq!(
        rows.update_n_clob_reader_by_label("resource", Some(&reader), RdbcCharacterLength::Long(3))
            .unwrap_err()
            .sql_exception()
            .unwrap()
            .sql_state(),
        Some("0A000")
    );
    assert_eq!(rows.value(column).unwrap(), Value::Int(42));
    assert!(!stream.is_closed());
    assert!(!reader.is_closed());
    assert_eq!(stream.read_to_end().unwrap(), vec![1, 2, 3]);
    assert_eq!(reader.read_to_string().unwrap(), "A😀");
}

/// 原默认时间转换保留字符串、日期与时间戳之间的无损可观察转换。
/// 对应 Java: java.sql.ResultSet#getDate/getTime/getTimestamp 的驱动默认合同。
#[tokio::test]
async fn buffered_defaults_convert_alternate_temporal_values_and_preserve_errors() {
    let date = RdbcNaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = RdbcNaiveTime::from_hms_nano_opt(13, 14, 15, 123_000_000).unwrap();
    let timestamp = date.and_time(time);
    let mut rows = BufferedRowSet::new(vec![Row::new(vec![
        Value::String("2026-07-29".to_owned()),
        Value::String("13:14:15.123".to_owned()),
        Value::String("2026-07-29T13:14:15.123".to_owned()),
        Value::Date(date),
        Value::Timestamp(timestamp),
        Value::String("invalid".to_owned()),
        Value::Int(7),
        Value::Null,
    ])]);
    assert!(rows.next().await.unwrap());
    let calendar = RdbcCalendarArgument::Specified(None);
    assert_eq!(
        rows.date(ColumnIndex::new(1).unwrap(), &calendar).unwrap(),
        Some(date)
    );
    assert_eq!(
        rows.time(ColumnIndex::new(2).unwrap(), &calendar).unwrap(),
        Some(time)
    );
    assert_eq!(
        rows.timestamp(ColumnIndex::new(3).unwrap(), &calendar)
            .unwrap(),
        Some(timestamp)
    );
    assert_eq!(
        rows.timestamp(ColumnIndex::new(4).unwrap(), &calendar)
            .unwrap(),
        Some(date.and_time(RdbcNaiveTime::MIN))
    );
    assert_eq!(
        rows.date(ColumnIndex::new(5).unwrap(), &calendar).unwrap(),
        Some(date)
    );
    assert_eq!(
        rows.time(ColumnIndex::new(5).unwrap(), &calendar).unwrap(),
        Some(time)
    );
    for column in [6, 7] {
        assert!(matches!(
            rows.date(ColumnIndex::new(column).unwrap(), &calendar),
            Err(druid_rdbc::error::RdbcError::SqlException(error))
                if error.class_name() == "java.sql.SQLDataException"
                    && error.sql_state() == Some(if column == 6 { "22007" } else { "22005" })
        ));
        assert!(matches!(
            rows.time(ColumnIndex::new(column).unwrap(), &calendar),
            Err(druid_rdbc::error::RdbcError::SqlException(error))
                if error.class_name() == "java.sql.SQLDataException"
                    && error.sql_state() == Some(if column == 6 { "22007" } else { "22005" })
        ));
        assert!(matches!(
            rows.timestamp(ColumnIndex::new(column).unwrap(), &calendar),
            Err(druid_rdbc::error::RdbcError::SqlException(error))
                if error.class_name() == "java.sql.SQLDataException"
                    && error.sql_state() == Some(if column == 6 { "22007" } else { "22005" })
        ));
    }
    assert_eq!(
        rows.date(ColumnIndex::new(8).unwrap(), &calendar).unwrap(),
        None
    );
    assert_eq!(
        rows.time(ColumnIndex::new(8).unwrap(), &calendar).unwrap(),
        None
    );
    assert_eq!(
        rows.timestamp(ColumnIndex::new(8).unwrap(), &calendar)
            .unwrap(),
        None
    );
    assert_eq!(
        rows.big_decimal_with_scale(ColumnIndex::new(8).unwrap(), 7)
            .unwrap(),
        None
    );
    rows.close().await.unwrap();
    assert!(matches!(
        rows.date(ColumnIndex::new(1).unwrap(), &calendar),
        Err(druid_rdbc::error::RdbcError::Other(message)) if message == "result set is closed"
    ));
    assert!(matches!(
        rows.big_decimal_with_scale(ColumnIndex::new(1).unwrap(), 7),
        Err(druid_rdbc::error::RdbcError::Other(message)) if message == "result set is closed"
    ));
}
