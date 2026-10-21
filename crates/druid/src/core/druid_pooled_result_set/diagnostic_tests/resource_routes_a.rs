//! 结果集诊断：resource_routes_a 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

/// 完整固定BASE资源getter、共享流游标、精确错误与真实SQLite读取合同。
#[tokio::test]
async fn task6_pooled_result_set_routes_resource_getters_and_real_sqlite_streams() {
    use super::resource_short_circuit_filter::ResourceShortCircuitFilter;
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::resource::{RdbcInputStream, RdbcReader};
    use druid_rdbc::result_set::ResultSetExt;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let shared_stream = RdbcInputStream::from_bytes([1, 2, 3]);
    let shared_reader = RdbcReader::from_string("迁移");
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(ResourceShortCircuitFilter {
        calls: Arc::clone(&call_log),
        stream: shared_stream,
        reader: shared_reader,
    }));

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        76,
        "sqlite-resource-filter",
        Arc::new(filter_chain),
    )
    .await;
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();

    macro_rules! assert_none_resource_pair {
        ($index:ident, $label:ident, $position:expr, $name:literal) => {
            assert!(result_set
                .$index(ColumnIndex::new($position).unwrap())
                .unwrap()
                .is_none());
            assert!(result_set.$label($name).unwrap().is_none());
        };
    }

    assert_none_resource_pair!(reference, reference_by_label, 1, "ref");
    assert_none_resource_pair!(blob, blob_by_label, 2, "blob");
    assert_none_resource_pair!(clob, clob_by_label, 3, "clob");
    assert_none_resource_pair!(array, array_by_label, 4, "array");
    assert_none_resource_pair!(url, url_by_label, 5, "url");
    assert_none_resource_pair!(row_id, row_id_by_label, 6, "row_id");
    assert_none_resource_pair!(n_clob, n_clob_by_label, 7, "n_clob");
    assert_none_resource_pair!(sql_xml, sql_xml_by_label, 8, "sql_xml");

    let stream_by_index = result_set
        .ascii_stream(ColumnIndex::new(9).unwrap())
        .unwrap()
        .unwrap();
    let stream_by_label = result_set.ascii_stream_by_label("ascii").unwrap().unwrap();
    let mut first_byte = [0_u8; 1];
    assert_eq!(stream_by_index.read(&mut first_byte).unwrap(), 1);
    assert_eq!(first_byte, [1]);
    assert_eq!(stream_by_label.read_to_end().unwrap(), [2, 3]);

    assert_none_resource_pair!(unicode_stream, unicode_stream_by_label, 10, "unicode");
    assert_none_resource_pair!(binary_stream, binary_stream_by_label, 11, "binary");

    let reader_by_index = result_set
        .character_stream(ColumnIndex::new(12).unwrap())
        .unwrap()
        .unwrap();
    let reader_by_label = result_set
        .character_stream_by_label("character")
        .unwrap()
        .unwrap();
    let mut first_unit = [0_u16; 1];
    assert_eq!(reader_by_index.read_utf16(&mut first_unit).unwrap(), 1);
    assert_eq!(first_unit[0], '迁' as u16);
    assert_eq!(reader_by_label.read_to_string().unwrap(), "移");

    assert_none_resource_pair!(
        n_character_stream,
        n_character_stream_by_label,
        13,
        "n_character"
    );

    assert_eq!(
        result_set.sql_xml_by_label("fail"),
        Err(RdbcError::DriverError(
            "filtered SQLXML failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    assert_eq!(calls(&call_log).len(), 27);
    assert_eq!(
        calls(&call_log),
        [
            "result_set_get_ref:1",
            "result_set_get_ref_by_label:ref",
            "result_set_get_blob:2",
            "result_set_get_blob_by_label:blob",
            "result_set_get_clob:3",
            "result_set_get_clob_by_label:clob",
            "result_set_get_array:4",
            "result_set_get_array_by_label:array",
            "result_set_get_url:5",
            "result_set_get_url_by_label:url",
            "result_set_get_row_id:6",
            "result_set_get_row_id_by_label:row_id",
            "result_set_get_n_clob:7",
            "result_set_get_n_clob_by_label:n_clob",
            "result_set_get_sql_xml:8",
            "result_set_get_sql_xml_by_label:sql_xml",
            "result_set_get_ascii_stream:9",
            "result_set_get_ascii_stream_by_label:ascii",
            "result_set_get_unicode_stream:10",
            "result_set_get_unicode_stream_by_label:unicode",
            "result_set_get_binary_stream:11",
            "result_set_get_binary_stream_by_label:binary",
            "result_set_get_character_stream:12",
            "result_set_get_character_stream_by_label:character",
            "result_set_get_n_character_stream:13",
            "result_set_get_n_character_stream_by_label:n_character",
            "result_set_get_sql_xml_by_label:fail",
        ]
    );

    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    // VALUE_ADD / V5_HOST：真实 Toasty SQLite 行集经过默认 Filter，
    // 验证流资源的实际读取。Toasty 当前行集未暴露 SQL alias，故主机证据使用
    // RDBC 1-based 索引；标签重载身份由上方精确物理探针独立证明。
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        77,
        "sqlite-resource-pass-through",
        Arc::new(pass_chain),
    )
    .await;
    let mut result_set = statement
        .execute_query("SELECT X'010203' AS payload, '你好' AS text_value")
        .await
        .unwrap();
    assert!(result_set.next().await.unwrap());
    assert_eq!(
        result_set
            .binary_stream(ColumnIndex::new(1).unwrap())
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        [1, 2, 3]
    );
    assert_eq!(
        result_set
            .n_character_stream(ColumnIndex::new(2).unwrap())
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "你好"
    );
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 十三资源家族的原生重载身份、ABC顺序、B短路/错误及共享游标。
#[tokio::test]
async fn task6_resource_filters_preserve_native_overloads_order_and_stream_identity() {
    use super::ordered_resource_filter::OrderedResourceFilter;
    use super::resource_read_probe::ResourceReadProbe;
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::resource::{RdbcInputStream, RdbcReader};
    use druid_rdbc::result_set::{ResultSetExt, ResultSetRead, ResultSetStreamKind};
    use std::sync::Mutex;
    let requests = [
        ResultSetRead::Ref,
        ResultSetRead::Blob,
        ResultSetRead::Clob,
        ResultSetRead::Array,
        ResultSetRead::Url,
        ResultSetRead::RowId,
        ResultSetRead::NClob,
        ResultSetRead::SqlXml,
    ];
    for mode in [0, 1, 2] {
        let log = Arc::new(Mutex::new(Vec::new()));
        let stream = RdbcInputStream::from_bytes([1, 2, 3]);
        let reader = RdbcReader::from_string("迁移");
        let mut filters = FilterChain::new();
        for name in ["A", "B", "C"] {
            filters.add_result_set(Arc::new(OrderedResourceFilter {
                name,
                mode: if name == "B" { mode } else { 0 },
                calls: Arc::clone(&log),
                stream: stream.clone(),
                reader: reader.clone(),
            }));
        }
        let (mut statement, mut connection) =
            task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
        let admission = statement.inner.begin_standard_admission().unwrap();
        let mut rows = DruidPooledResultSet::new_standard(
            Arc::clone(&statement.inner),
            Box::new(ResourceReadProbe {
                calls: Arc::clone(&log),
                stream: stream.clone(),
                reader: reader.clone(),
                closed: false,
            }),
            &admission,
        )
        .await
        .unwrap();
        drop(admission);
        macro_rules! check_read {
            ($family:expr, $target:expr, $read:expr) => {{
                log.lock().unwrap().clear();
                let result: Result<bool, RdbcError> = $read;
                let expected = if mode == 2 {
                    Err(RdbcError::DriverError(
                        "ordered resource failure".to_owned(),
                    ))
                } else {
                    Ok(true)
                };
                assert_eq!(result, expected);
                let expected_log = if mode == 0 {
                    vec![
                        "A:before".to_owned(),
                        "B:before".to_owned(),
                        "C:before".to_owned(),
                        format!("raw:{}:{}", $family, $target),
                        "C:ok".to_owned(),
                        "B:ok".to_owned(),
                        "A:ok".to_owned(),
                    ]
                } else {
                    let status = if mode == 2 { "error" } else { "ok" };
                    vec![
                        "A:before".to_owned(),
                        "B:before".to_owned(),
                        format!("B:{status}"),
                        format!("A:{status}"),
                    ]
                };
                assert_eq!(*log.lock().unwrap(), expected_log);
            }};
        }
        for request in &requests {
            let family = format!("resource:{request:?}");
            check_read!(
                &family,
                "index:7",
                rows.read_object(ColumnIndex::new(7).unwrap(), request)
                    .map(|value| value.is_null())
            );
            check_read!(
                &family,
                "label:virtual",
                rows.read_object_by_label("virtual", request)
                    .map(|value| value.is_null())
            );
        }
        for kind in [
            ResultSetStreamKind::Ascii,
            ResultSetStreamKind::Unicode,
            ResultSetStreamKind::Binary,
        ] {
            let family = format!("stream:{kind:?}");
            check_read!(
                &family,
                "index:7",
                rows.input_stream(ColumnIndex::new(7).unwrap(), kind)
                    .map(|value| value.is_some())
            );
            check_read!(
                &family,
                "label:virtual",
                rows.input_stream_by_label("virtual", kind)
                    .map(|value| value.is_some())
            );
        }
        for national in [false, true] {
            let family = format!("reader:{national}");
            check_read!(
                &family,
                "index:7",
                rows.reader(ColumnIndex::new(7).unwrap(), national)
                    .map(|value| value.is_some())
            );
            check_read!(
                &family,
                "label:virtual",
                rows.reader_by_label("virtual", national)
                    .map(|value| value.is_some())
            );
        }
        assert_eq!(statement.exception_count(), if mode == 2 { 26 } else { 0 });
        if mode == 0 {
            // 默认委托返回同一原生游标；取得句柄不消费，两个重载可接续读取。
            let indexed = rows
                .ascii_stream(ColumnIndex::new(7).unwrap())
                .unwrap()
                .unwrap();
            let labeled = rows.ascii_stream_by_label("virtual").unwrap().unwrap();
            let mut first = [0_u8; 1];
            assert_eq!(indexed.read(&mut first).unwrap(), 1);
            assert_eq!(first, [1]);
            assert_eq!(labeled.read_to_end().unwrap(), [2, 3]);
            let indexed = rows
                .character_stream(ColumnIndex::new(7).unwrap())
                .unwrap()
                .unwrap();
            let labeled = rows.character_stream_by_label("virtual").unwrap().unwrap();
            let mut first = [0_u16; 1];
            assert_eq!(indexed.read_utf16(&mut first).unwrap(), 1);
            assert_eq!(first[0], '迁' as u16);
            assert_eq!(labeled.read_to_string().unwrap(), "移");
            macro_rules! native_error {
                ($family:expr, $read:expr) => {{
                    log.lock().unwrap().clear();
                    let result: Result<bool, RdbcError> = $read;
                    assert_eq!(
                        result,
                        Err(RdbcError::DriverError("native resource failure".to_owned()))
                    );
                    assert_eq!(
                        *log.lock().unwrap(),
                        vec![
                            "A:before".to_owned(),
                            "B:before".to_owned(),
                            "C:before".to_owned(),
                            format!("raw:{}:label:fail", $family),
                            "C:error".to_owned(),
                            "B:error".to_owned(),
                            "A:error".to_owned()
                        ]
                    );
                }};
            }
            for request in &requests {
                native_error!(
                    format!("resource:{request:?}"),
                    rows.read_object_by_label("fail", request)
                        .map(|value| value.is_null())
                );
            }
            for kind in [
                ResultSetStreamKind::Ascii,
                ResultSetStreamKind::Unicode,
                ResultSetStreamKind::Binary,
            ] {
                native_error!(
                    format!("stream:{kind:?}"),
                    rows.input_stream_by_label("fail", kind)
                        .map(|value| value.is_some())
                );
            }
            for national in [false, true] {
                native_error!(
                    format!("reader:{national}"),
                    rows.reader_by_label("fail", national)
                        .map(|value| value.is_some())
                );
            }
            assert_eq!(statement.exception_count(), 13);
        }
        rows.close().await.unwrap();
        log.lock().unwrap().clear();
        assert_eq!(
            rows.binary_stream_by_label("virtual"),
            Err(RdbcError::ConnectionDiscarded)
        );
        assert_eq!(
            rows.character_stream_by_label("virtual"),
            Err(RdbcError::ConnectionDiscarded)
        );
        assert!(log.lock().unwrap().is_empty());
        statement.close().await.unwrap();
        connection.close().await.unwrap();
    }
}

/// 资源短路仍服从隐藏列、关闭与旧租约治理，失败只向实际创建语句记录一次。
#[tokio::test]
async fn task6_resource_filters_obey_column_visibility_close_and_expired_lease() {
    use super::resource_short_circuit_filter::ResourceShortCircuitFilter;
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::resource::{RdbcInputStream, RdbcReader};
    use druid_rdbc::result_set::{ResultSetRead, ResultSetStreamKind};
    use std::sync::Mutex;
    let requests = [
        ResultSetRead::Ref,
        ResultSetRead::Blob,
        ResultSetRead::Clob,
        ResultSetRead::Array,
        ResultSetRead::Url,
        ResultSetRead::RowId,
        ResultSetRead::NClob,
        ResultSetRead::SqlXml,
    ];
    for expired in [false, true] {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut filters = FilterChain::new();
        filters.add_result_set(Arc::new(ResourceShortCircuitFilter {
            calls: Arc::clone(&log),
            stream: RdbcInputStream::from_bytes([1, 2, 3]),
            reader: RdbcReader::from_string("迁移"),
        }));
        let (mut rows, mut statement, mut connection) =
            task6_sqlite_diagnostic_cursor_with_sql_and_filters(
                "SELECT 1 AS value",
                Some(Arc::new(filters)),
            )
            .await;
        let error = if expired {
            statement.inner.lease_active.store(false, Ordering::Release);
            RdbcError::ConnectionDiscarded
        } else {
            rows.set_hidden_columns(Some(vec![1]));
            RdbcError::InvalidArgument("column is hidden".to_owned())
        };
        for request in &requests {
            assert_eq!(
                rows.read_object(ColumnIndex::new(1).unwrap(), request),
                Err(error.clone())
            );
            assert_eq!(
                rows.read_object_by_label("value", request),
                Err(error.clone())
            );
        }
        for kind in [
            ResultSetStreamKind::Ascii,
            ResultSetStreamKind::Unicode,
            ResultSetStreamKind::Binary,
        ] {
            assert_eq!(
                rows.input_stream(ColumnIndex::new(1).unwrap(), kind),
                Err(error.clone())
            );
            assert_eq!(
                rows.input_stream_by_label("value", kind),
                Err(error.clone())
            );
        }
        for national in [false, true] {
            assert_eq!(
                rows.reader(ColumnIndex::new(1).unwrap(), national),
                Err(error.clone())
            );
            assert_eq!(rows.reader_by_label("value", national), Err(error.clone()));
        }
        assert!(log.lock().unwrap().is_empty());
        assert_eq!(statement.exception_count(), if expired { 0 } else { 26 });
        rows.close().await.unwrap();
        assert_eq!(
            rows.input_stream_by_label("value", ResultSetStreamKind::Ascii),
            Err(RdbcError::ConnectionDiscarded)
        );
        assert_eq!(
            rows.reader_by_label("value", false),
            Err(RdbcError::ConnectionDiscarded)
        );
        assert!(log.lock().unwrap().is_empty());
        assert_eq!(statement.exception_count(), if expired { 0 } else { 26 });
        statement.inner.lease_active.store(true, Ordering::Release);
        statement.close().await.unwrap();
        connection.close().await.unwrap();
    }
}
