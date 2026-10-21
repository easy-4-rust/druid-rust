//! 结果集诊断：resource_routes_b 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

/// 恢复固定BASE十四种资源更新重载、精确错误和实际SQLite能力合同。
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn task6_pooled_result_set_routes_resource_updates_and_preserves_sqlite_capability_errors() {
    use super::resource_update_short_circuit_filter::ResourceUpdateShortCircuitFilter;
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::RdbcRowId;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(ResourceUpdateShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        91,
        "sqlite-resource-update-short-circuit",
        Arc::new(filter_chain),
    )
    .await;
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();

    let row_id = RdbcRowId::new([4, 5, 6]);

    result_set
        .update_reference(ColumnIndex::new(1).unwrap(), None)
        .unwrap();
    result_set.update_reference_by_label("ref", None).unwrap();
    result_set
        .update_blob(ColumnIndex::new(2).unwrap(), None)
        .unwrap();
    result_set.update_blob_by_label("blob", None).unwrap();
    result_set
        .update_clob(ColumnIndex::new(3).unwrap(), None)
        .unwrap();
    result_set.update_clob_by_label("clob", None).unwrap();
    result_set
        .update_array(ColumnIndex::new(4).unwrap(), None)
        .unwrap();
    result_set.update_array_by_label("array", None).unwrap();
    result_set
        .update_row_id(ColumnIndex::new(5).unwrap(), Some(row_id.clone()))
        .unwrap();
    result_set
        .update_row_id_by_label("row_id", Some(row_id.clone()))
        .unwrap();
    result_set
        .update_n_clob(ColumnIndex::new(6).unwrap(), None)
        .unwrap();
    result_set.update_n_clob_by_label("n_clob", None).unwrap();
    result_set
        .update_sql_xml(ColumnIndex::new(7).unwrap(), None)
        .unwrap();
    result_set.update_sql_xml_by_label("sql_xml", None).unwrap();
    assert_eq!(
        calls(&call_log),
        [
            "result_set_update_reference:1:None",
            "result_set_update_reference_by_label:ref:None",
            "result_set_update_blob:2:None",
            "result_set_update_blob_by_label:blob:None",
            "result_set_update_clob:3:None",
            "result_set_update_clob_by_label:clob:None",
            "result_set_update_array:4:None",
            "result_set_update_array_by_label:array:None",
            "result_set_update_row_id:5:Some(RdbcRowId { bytes: [4, 5, 6] })",
            "result_set_update_row_id_by_label:row_id:Some(RdbcRowId { bytes: [4, 5, 6] })",
            "result_set_update_n_clob:6:None",
            "result_set_update_n_clob_by_label:n_clob:None",
            "result_set_update_sql_xml:7:None",
            "result_set_update_sql_xml_by_label:sql_xml:None",
        ]
    );

    // RUST_OBLIGATION / V1_RUST_LOCAL：Filter 错误只在原 Statement 分类一次，
    // 且不会触达只读 RowSet。
    assert_eq!(
        result_set.update_sql_xml_by_label("fail", None),
        Err(RdbcError::DriverError(
            "filtered resource update failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        92,
        "sqlite-resource-update-pass-through",
        Arc::new(pass_chain),
    )
    .await;
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();

    macro_rules! assert_resource_unsupported {
        ($expression:expr, $operation:literal) => {
            assert_eq!(
                $expression,
                Err(RdbcError::UnsupportedOperation {
                    operation: $operation,
                })
            );
        };
    }

    // VALUE_ADD / V5_HOST：默认 Filter 保留真实 Toasty SQLite 的 14 个精确
    // capability error，而不是统一成通用 update 错误。
    assert_resource_unsupported!(
        result_set.update_reference(ColumnIndex::new(1).unwrap(), None),
        "result_set_update_ref"
    );
    assert_resource_unsupported!(
        result_set.update_reference_by_label("ref", None),
        "result_set_update_ref_by_label"
    );
    assert_resource_unsupported!(
        result_set.update_blob(ColumnIndex::new(2).unwrap(), None),
        "result_set_update_blob"
    );
    assert_resource_unsupported!(
        result_set.update_blob_by_label("blob", None),
        "result_set_update_blob_by_label"
    );
    assert_resource_unsupported!(
        result_set.update_clob(ColumnIndex::new(3).unwrap(), None),
        "result_set_update_clob"
    );
    assert_resource_unsupported!(
        result_set.update_clob_by_label("clob", None),
        "result_set_update_clob_by_label"
    );
    assert_resource_unsupported!(
        result_set.update_array(ColumnIndex::new(4).unwrap(), None),
        "result_set_update_array"
    );
    assert_resource_unsupported!(
        result_set.update_array_by_label("array", None),
        "result_set_update_array_by_label"
    );
    assert_resource_unsupported!(
        result_set.update_row_id(ColumnIndex::new(5).unwrap(), Some(row_id.clone())),
        "result_set_update_row_id"
    );
    assert_resource_unsupported!(
        result_set.update_row_id_by_label("row_id", Some(row_id.clone())),
        "result_set_update_row_id_by_label"
    );
    assert_resource_unsupported!(
        result_set.update_n_clob(ColumnIndex::new(6).unwrap(), None),
        "result_set_update_n_clob"
    );
    assert_resource_unsupported!(
        result_set.update_n_clob_by_label("n_clob", None),
        "result_set_update_n_clob_by_label"
    );
    assert_resource_unsupported!(
        result_set.update_sql_xml(ColumnIndex::new(7).unwrap(), None),
        "result_set_update_sql_xml"
    );
    assert_resource_unsupported!(
        result_set.update_sql_xml_by_label("sql_xml", None),
        "result_set_update_sql_xml_by_label"
    );
    assert_eq!(statement.exception_count(), 14);
    assert!(!result_set.is_closed().unwrap());
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 保留固定BASE七行操作短路、精确刷新错误与真实SQLite能力拒绝。
#[tokio::test]
async fn task6_pooled_result_set_routes_row_mutations_and_preserves_sqlite_capability_errors() {
    use super::row_mutation_short_circuit_filter::RowMutationShortCircuitFilter;
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let fail_refresh = Arc::new(AtomicBool::new(false));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(RowMutationShortCircuitFilter {
        calls: Arc::clone(&call_log),
        fail_refresh: Arc::clone(&fail_refresh),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        85,
        "sqlite-row-mutation-short-circuit",
        Arc::new(filter_chain),
    )
    .await;
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();

    // SOURCE_PARITY / V2_MIRRORED：Filter 可以短路七个 raw mutation 调用。
    result_set.insert_row().unwrap();
    result_set.update_row().unwrap();
    result_set.delete_row().unwrap();
    result_set.refresh_row().unwrap();
    result_set.cancel_row_updates().unwrap();
    result_set.move_to_insert_row().unwrap();
    result_set.move_to_current_row().unwrap();
    assert_eq!(
        calls(&call_log),
        [
            "insert_row",
            "update_row",
            "delete_row",
            "refresh_row",
            "cancel_row_updates",
            "move_to_insert_row",
            "move_to_current_row",
        ]
    );

    // RUST_OBLIGATION / V1_RUST_LOCAL：Filter 错误必须在原 Statement 计数，
    // 且短路后不能触达物理 ResultSet。
    fail_refresh.store(true, Ordering::Release);
    assert_eq!(
        result_set.refresh_row(),
        Err(RdbcError::DriverError(
            "filtered refreshRow failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    assert_eq!(calls(&call_log).last().unwrap(), "refresh_row");
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    // VALUE_ADD / V5_HOST：真实 Toasty SQLite 的只读 RowSet 必须经默认
    // Filter 保留每个物理方法自己的精确 capability error，而不是统一报错。
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        86,
        "sqlite-row-mutation-pass-through",
        Arc::new(pass_chain),
    )
    .await;
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();

    macro_rules! assert_capability_error {
        ($method:ident, $operation:literal) => {
            assert_eq!(
                result_set.$method(),
                Err(RdbcError::UnsupportedOperation {
                    operation: $operation,
                })
            );
        };
    }

    assert_capability_error!(insert_row, "result_set_insert_row");
    assert_capability_error!(update_row, "result_set_update_row");
    assert_capability_error!(delete_row, "result_set_delete_row");
    assert_capability_error!(refresh_row, "result_set_refresh_row");
    assert_capability_error!(cancel_row_updates, "result_set_cancel_row_updates");
    assert_capability_error!(move_to_insert_row, "result_set_move_to_insert_row");
    assert_capability_error!(move_to_current_row, "result_set_move_to_current_row");
    assert_eq!(statement.exception_count(), 7);
    assert!(!result_set.is_closed().unwrap());
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 七种行操作均在Filter之前拒绝旧租约及关闭，且不新增原语句异常。
#[tokio::test]
async fn task6_row_operation_filters_reject_expired_lease_and_closed_cursor_before_callbacks() {
    use super::row_mutation_short_circuit_filter::RowMutationShortCircuitFilter;
    use crate::core::{FilterChain, RdbcError};
    use std::sync::Mutex;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(RowMutationShortCircuitFilter {
        calls: Arc::clone(&calls),
        fail_refresh: Arc::new(AtomicBool::new(false)),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let (mut statement, mut connection) =
        task6_sqlite_metadata_statement(&factory, 87, "row-operation-guards", Arc::new(filters))
            .await;
    let mut rows = statement.execute_query("SELECT 1 AS value").await.unwrap();
    statement.inner.lease_active.store(false, Ordering::Release);
    macro_rules! assert_guard {
        ($method:ident) => {
            assert_eq!(rows.$method(), Err(RdbcError::ConnectionDiscarded));
        };
    }
    for closed in [false, true] {
        if closed {
            statement.inner.lease_active.store(true, Ordering::Release);
            rows.close().await.unwrap();
        }
        assert_guard!(insert_row);
        assert_guard!(update_row);
        assert_guard!(delete_row);
        assert_guard!(refresh_row);
        assert_guard!(cancel_row_updates);
        assert_guard!(move_to_insert_row);
        assert_guard!(move_to_current_row);
        assert!(calls.lock().unwrap().is_empty());
        assert_eq!(statement.exception_count(), 0);
    }
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 恢复完整原标量更新短路、精确错误与真实SQLite能力合同。
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn task6_pooled_result_set_routes_scalar_updates_and_preserves_sqlite_capability_errors() {
    use super::scalar_update_short_circuit_filter::ScalarUpdateShortCircuitFilter;
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(ScalarUpdateShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        87,
        "sqlite-scalar-update-short-circuit",
        Arc::new(filter_chain),
    )
    .await;
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_opt(10, 11, 12).unwrap();

    macro_rules! invoke_pair {
        ($variant:ident, $index:ident, $label:ident, $index_value:expr, $label_value:expr) => {
            result_set
                .update_value(
                    ColumnIndex::new(1).unwrap(),
                    ResultSetUpdate::$variant($index_value),
                )
                .unwrap();
            result_set
                .update_value_by_label("value", ResultSetUpdate::$variant($label_value))
                .unwrap();
        };
    }

    // SOURCE_PARITY / V2_MIRRORED：真实池化结果集上的 28 个调用全部被
    // 各自的 Filter 方法短路，证明没有退化成单一通用 update 回调。
    result_set
        .update_value(ColumnIndex::new(1).unwrap(), ResultSetUpdate::Null)
        .unwrap();
    result_set
        .update_value_by_label("value", ResultSetUpdate::Null)
        .unwrap();
    invoke_pair!(
        Boolean,
        update_boolean,
        update_boolean_by_label,
        true,
        false
    );
    invoke_pair!(Byte, update_byte, update_byte_by_label, -8, 8);
    invoke_pair!(Short, update_short, update_short_by_label, -16, 16);
    invoke_pair!(Int, update_int, update_int_by_label, -32, 32);
    invoke_pair!(Long, update_long, update_long_by_label, -64, 64);
    invoke_pair!(Float, update_float, update_float_by_label, 1.25, -1.25);
    invoke_pair!(Double, update_double, update_double_by_label, 2.5, -2.5);
    invoke_pair!(
        BigDecimal,
        update_big_decimal,
        update_big_decimal_by_label,
        Some(BigDecimal::from_str("12.340").unwrap()),
        None
    );
    invoke_pair!(
        String,
        update_string,
        update_string_by_label,
        Some("index".to_string()),
        None
    );
    invoke_pair!(
        Bytes,
        update_bytes,
        update_bytes_by_label,
        Some(vec![0, 255]),
        None
    );
    invoke_pair!(Date, update_date, update_date_by_label, Some(date), None);
    invoke_pair!(Time, update_time, update_time_by_label, Some(time), None);
    invoke_pair!(
        Timestamp,
        update_timestamp,
        update_timestamp_by_label,
        Some(date.and_time(time)),
        None
    );
    assert_eq!(calls(&call_log).len(), 28);
    assert_eq!(
        calls(&call_log),
        vec![
            "result_set_update_null:1".to_owned(),
            "result_set_update_null_by_label:value".to_owned(),
            format!("result_set_update_boolean:1:{:?}", true),
            format!("result_set_update_boolean_by_label:value:{:?}", false),
            format!("result_set_update_byte:1:{:?}", -8),
            format!("result_set_update_byte_by_label:value:{:?}", 8),
            format!("result_set_update_short:1:{:?}", -16),
            format!("result_set_update_short_by_label:value:{:?}", 16),
            format!("result_set_update_int:1:{:?}", -32),
            format!("result_set_update_int_by_label:value:{:?}", 32),
            format!("result_set_update_long:1:{:?}", -64),
            format!("result_set_update_long_by_label:value:{:?}", 64),
            format!("result_set_update_float:1:{:?}", 1.25),
            format!("result_set_update_float_by_label:value:{:?}", -1.25),
            format!("result_set_update_double:1:{:?}", 2.5),
            format!("result_set_update_double_by_label:value:{:?}", -2.5),
            format!(
                "result_set_update_big_decimal:1:{:?}",
                Some(BigDecimal::from_str("12.340").unwrap())
            ),
            format!(
                "result_set_update_big_decimal_by_label:value:{:?}",
                None::<BigDecimal>
            ),
            format!("result_set_update_string:1:{:?}", Some("index".to_string())),
            format!(
                "result_set_update_string_by_label:value:{:?}",
                None::<String>
            ),
            format!("result_set_update_bytes:1:{:?}", Some(vec![0u8, 255])),
            format!(
                "result_set_update_bytes_by_label:value:{:?}",
                None::<Vec<u8>>
            ),
            format!("result_set_update_date:1:{:?}", Some(date)),
            format!(
                "result_set_update_date_by_label:value:{:?}",
                None::<NaiveDate>
            ),
            format!("result_set_update_time:1:{:?}", Some(time)),
            format!(
                "result_set_update_time_by_label:value:{:?}",
                None::<NaiveTime>
            ),
            format!(
                "result_set_update_timestamp:1:{:?}",
                Some(date.and_time(time))
            ),
            format!(
                "result_set_update_timestamp_by_label:value:{:?}",
                None::<druid_rdbc::types::RdbcNaiveDateTime>
            )
        ]
    );

    // RUST_OBLIGATION / V1_RUST_LOCAL：Filter 错误归属原 Statement，
    // 且失败的标签重载不触达只读 SQLite RowSet。
    assert_eq!(
        result_set.update_value_by_label("fail", ResultSetUpdate::Timestamp(None)),
        Err(RdbcError::DriverError(
            "filtered scalar update failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    // VALUE_ADD / V5_HOST：默认 Filter 对真实 Toasty SQLite 的 28 个
    // 调用保持物理只读能力错误，并区分下标与标签终端。
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        88,
        "sqlite-scalar-update-pass-through",
        Arc::new(pass_chain),
    )
    .await;
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();

    macro_rules! assert_index_error {
        ($variant:ident, $method:ident $(, $value:expr)?) => {
            assert_eq!(
                result_set.update_value(ColumnIndex::new(1).unwrap(), ResultSetUpdate::$variant $(($value))?),
                Err(RdbcError::UnsupportedOperation {
                    operation: "result_set_update_value",
                })
            );
        };
    }
    macro_rules! assert_label_error {
        ($variant:ident, $method:ident $(, $value:expr)?) => {
            assert_eq!(
                result_set.update_value_by_label("value", ResultSetUpdate::$variant $(($value))?),
                Err(RdbcError::UnsupportedOperation {
                    operation: "result_set_update_value_by_label",
                })
            );
        };
    }
    macro_rules! assert_pair_error {
        ($variant:ident, $index:ident, $label:ident, $index_value:expr, $label_value:expr) => {
            assert_index_error!($variant, $index, $index_value);
            assert_label_error!($variant, $label, $label_value);
        };
    }

    assert_index_error!(Null, update_null);
    assert_label_error!(Null, update_null_by_label);
    assert_pair_error!(
        Boolean,
        update_boolean,
        update_boolean_by_label,
        true,
        false
    );
    assert_pair_error!(Byte, update_byte, update_byte_by_label, -8, 8);
    assert_pair_error!(Short, update_short, update_short_by_label, -16, 16);
    assert_pair_error!(Int, update_int, update_int_by_label, -32, 32);
    assert_pair_error!(Long, update_long, update_long_by_label, -64, 64);
    assert_pair_error!(Float, update_float, update_float_by_label, 1.25, -1.25);
    assert_pair_error!(Double, update_double, update_double_by_label, 2.5, -2.5);
    assert_pair_error!(
        BigDecimal,
        update_big_decimal,
        update_big_decimal_by_label,
        Some(BigDecimal::from(1)),
        None
    );
    assert_pair_error!(
        String,
        update_string,
        update_string_by_label,
        Some("index".to_string()),
        None
    );
    assert_pair_error!(
        Bytes,
        update_bytes,
        update_bytes_by_label,
        Some(vec![0, 255]),
        None
    );
    assert_pair_error!(Date, update_date, update_date_by_label, Some(date), None);
    assert_pair_error!(Time, update_time, update_time_by_label, Some(time), None);
    assert_pair_error!(
        Timestamp,
        update_timestamp,
        update_timestamp_by_label,
        Some(date.and_time(time)),
        None
    );
    assert_eq!(statement.exception_count(), 28);
    assert!(!result_set.is_closed().unwrap());
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
