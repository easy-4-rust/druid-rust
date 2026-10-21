//! 结果集诊断：n_string_updates 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

/// 默认链保留完整描述符；A/B/C 正向进入，反向退出，B 短路不触及 C 或驱动。
#[test]
fn task6_n_string_update_default_order_short_circuit_and_error_contracts() {
    use super::ordered_n_string_update_filter::OrderedNStringUpdateFilter;
    use super::update_call::UpdateCall;
    use super::update_identity_probe::UpdateIdentityProbe;
    use crate::core::{FilterAdapter, FilterChain, RdbcError, ResultSetFilterContext};
    use druid_rdbc::result_set::ResultSetUpdate;
    use std::sync::Mutex;
    let native = Arc::new(Mutex::new(Vec::new()));
    let mut raw = UpdateIdentityProbe::new(Arc::clone(&native));
    let context = ResultSetFilterContext::new();
    let mut defaults = FilterChain::new();
    defaults.add_result_set(Arc::new(FilterAdapter::new()));
    defaults.add_result_set(Arc::new(FilterAdapter::new()));
    defaults
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(9).unwrap(),
            ResultSetUpdate::NString(Some("默认-索引".to_owned())),
        )
        .unwrap();
    defaults
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "名称",
            ResultSetUpdate::NString(None),
        )
        .unwrap();
    defaults
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "普通",
            ResultSetUpdate::String(Some("String".to_owned())),
        )
        .unwrap();
    assert_eq!(
        *native.lock().unwrap(),
        [
            UpdateCall::Index(9, ResultSetUpdate::NString(Some("默认-索引".to_owned()))),
            UpdateCall::Label("名称".to_owned(), ResultSetUpdate::NString(None)),
            UpdateCall::Label(
                "普通".to_owned(),
                ResultSetUpdate::String(Some("String".to_owned()))
            ),
        ]
    );
    for mode in [0, 1, 2] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut filters = FilterChain::new();
        for name in ["A", "B", "C"] {
            filters.add_result_set(Arc::new(OrderedNStringUpdateFilter {
                name,
                mode: if name == "B" { mode } else { 0 },
                calls: Arc::clone(&calls),
            }));
        }
        for label in [false, true] {
            calls.lock().unwrap().clear();
            native.lock().unwrap().clear();
            let descriptor = ResultSetUpdate::NString(Some("完整-😀".to_owned()));
            let result = if label {
                filters.standard_result_set_update_by_label(
                    &mut raw,
                    &context,
                    "虚拟",
                    descriptor.clone(),
                )
            } else {
                filters.standard_result_set_update(
                    &mut raw,
                    &context,
                    ColumnIndex::new(7).unwrap(),
                    descriptor.clone(),
                )
            }
            .map_err(RdbcError::from);
            let target = if label { "label:虚拟" } else { "index:7" };
            if mode == 2 {
                assert_eq!(
                    result,
                    Err(RdbcError::DriverError(
                        "ordered NString update failure".to_owned()
                    ))
                );
            } else {
                assert_eq!(result, Ok(()));
            }
            let expected = if mode == 0 {
                vec![
                    format!("A:{target}:before"),
                    format!("B:{target}:before"),
                    format!("C:{target}:before"),
                    format!("C:{target}:ok"),
                    format!("B:{target}:ok"),
                    format!("A:{target}:ok"),
                ]
            } else {
                let status = if mode == 2 { "error" } else { "ok" };
                vec![
                    format!("A:{target}:before"),
                    format!("B:{target}:before"),
                    format!("B:{target}:{status}"),
                    format!("A:{target}:{status}"),
                ]
            };
            assert_eq!(*calls.lock().unwrap(), expected);
            let expected_native = if mode == 0 {
                vec![if label {
                    UpdateCall::Label("虚拟".to_owned(), descriptor)
                } else {
                    UpdateCall::Index(7, descriptor)
                }]
            } else {
                Vec::new()
            };
            assert_eq!(*native.lock().unwrap(), expected_native);
        }
    }
}

/// 列治理、关闭与旧租约优先拒绝；成功短路不改真实 SQLite 值或普通 String 家族。
#[tokio::test]
async fn task6_n_string_update_preserves_governance_cursor_and_lease_boundaries() {
    use super::n_string_update_short_circuit_filter::NStringUpdateShortCircuitFilter;
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetUpdate;
    use std::sync::Mutex;
    for expire in [false, true] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut filters = FilterChain::new();
        filters.add_result_set(Arc::new(NStringUpdateShortCircuitFilter {
            calls: Arc::clone(&calls),
        }));
        let (mut rows, mut statement, mut connection) =
            task6_sqlite_diagnostic_cursor_with_sql_and_filters(
                "SELECT '原值' AS value",
                Some(Arc::new(filters)),
            )
            .await;
        rows.set_hidden_columns(Some(vec![1]));
        for label in [false, true] {
            let result = if label {
                rows.update_value_by_label("value", ResultSetUpdate::NString(None))
            } else {
                rows.update_value(ColumnIndex::new(1).unwrap(), ResultSetUpdate::NString(None))
            };
            assert_eq!(
                result,
                Err(RdbcError::InvalidArgument("column is hidden".to_owned()))
            );
        }
        assert_eq!(statement.exception_count(), 2);
        assert!(calls.lock().unwrap().is_empty());
        rows.set_hidden_columns(None);
        assert_eq!(
            rows.update_value(ColumnIndex::new(1).unwrap(), ResultSetUpdate::String(None)),
            Err(RdbcError::UnsupportedOperation {
                operation: "result_set_update_value"
            })
        );
        assert!(calls.lock().unwrap().is_empty());
        rows.update_value(ColumnIndex::new(1).unwrap(), ResultSetUpdate::NString(None))
            .unwrap();
        rows.update_value_by_label("虚拟", ResultSetUpdate::NString(Some("短路".to_owned())))
            .unwrap();
        assert!(rows.next().await.unwrap());
        assert_eq!(
            rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            druid_rdbc::types::Value::String("原值".to_owned())
        );
        assert_eq!(statement.exception_count(), 3);
        if expire {
            statement.inner.lease_active.store(false, Ordering::Release);
        } else {
            rows.close().await.unwrap();
        }
        calls.lock().unwrap().clear();
        assert_eq!(
            rows.update_value(ColumnIndex::new(1).unwrap(), ResultSetUpdate::NString(None)),
            Err(RdbcError::ConnectionDiscarded)
        );
        assert_eq!(
            rows.update_value_by_label("虚拟", ResultSetUpdate::NString(None)),
            Err(RdbcError::ConnectionDiscarded)
        );
        assert!(calls.lock().unwrap().is_empty());
        assert_eq!(statement.exception_count(), 3);
        rows.close().await.unwrap();
        statement.close().await.unwrap();
        connection.close().await.unwrap();
    }
}

/// 恢复固定 BASE 的完整 NString 更新短路与真实 SQLite 能力错误合同。
/// 对应 Java: DruidPooledResultSet#updateNString。
#[tokio::test]
async fn task6_pooled_result_set_routes_n_string_updates_and_preserves_sqlite_capability_errors() {
    use super::n_string_update_short_circuit_filter::NStringUpdateShortCircuitFilter;
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetUpdate;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(NStringUpdateShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        97,
        "sqlite-n-string-update-short-circuit",
        Arc::new(filter_chain),
    )
    .await;
    let mut result_set = statement
        .execute_query("SELECT '原值' AS value")
        .await
        .unwrap();
    // 保留 NString 描述符，不折叠为 String；未定位的原游标允许 Filter 短路。
    result_set
        .update_value(
            ColumnIndex::new(1).unwrap(),
            ResultSetUpdate::NString(Some("过滤-索引".to_string())),
        )
        .unwrap();
    result_set
        .update_value_by_label("名称", ResultSetUpdate::NString(None))
        .unwrap();
    assert_eq!(
        calls(&call_log),
        [
            "result_set_update_n_string:1:Some(\"过滤-索引\")",
            "result_set_update_n_string_by_label:名称:None",
        ]
    );
    assert_eq!(
        result_set
            .update_value_by_label("fail", ResultSetUpdate::NString(Some("失败".to_string()))),
        Err(RdbcError::DriverError(
            "filtered NString update failure".to_string()
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
        98,
        "sqlite-n-string-update-pass-through",
        Arc::new(pass_chain),
    )
    .await;
    let mut result_set = statement
        .execute_query("SELECT '原值' AS value")
        .await
        .unwrap();
    // 原生只读能力错误从对应重载返回，实际创建语句分别计数一次。
    assert_eq!(
        result_set.update_value(
            ColumnIndex::new(1).unwrap(),
            ResultSetUpdate::NString(Some("索引".to_string()))
        ),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_update_value"
        })
    );
    assert_eq!(
        result_set.update_value_by_label("value", ResultSetUpdate::NString(None)),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_update_value_by_label"
        })
    );
    assert_eq!(statement.exception_count(), 2);
    assert!(!result_set.is_closed().unwrap());
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_result_set_routes_lob_stream_updates_and_preserves_sqlite_capability_errors()
{
    use super::lob_stream_update_short_circuit_filter::LobStreamUpdateShortCircuitFilter;
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::{RdbcCharacterLength, RdbcInputStream, RdbcReader, RdbcStreamLength};
    use std::sync::Mutex;
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(LobStreamUpdateShortCircuitFilter::new(
        Arc::clone(&call_log),
    )));
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filter_chain))).await;
    let mut result_set = statement
        .execute_query("SELECT X'010203' AS value")
        .await
        .unwrap();
    let blob = RdbcInputStream::from_bytes([1, 2, 3]);
    let clob = RdbcReader::from_string("甲乙");
    let n_clob = RdbcReader::from_string("丙丁");

    result_set
        .update_blob_stream(
            ColumnIndex::new(1).unwrap(),
            Some(&blob),
            RdbcStreamLength::Unspecified,
        )
        .unwrap();
    result_set
        .update_blob_stream_by_label("blob", None, RdbcStreamLength::Unspecified)
        .unwrap();
    result_set
        .update_blob_stream(
            ColumnIndex::new(2).unwrap(),
            Some(&blob),
            RdbcStreamLength::Long(-7),
        )
        .unwrap();
    result_set
        .update_blob_stream_by_label("blob_long", None, RdbcStreamLength::Long(i64::MAX))
        .unwrap();
    result_set
        .update_clob_reader(
            ColumnIndex::new(3).unwrap(),
            Some(&clob),
            RdbcCharacterLength::Unspecified,
        )
        .unwrap();
    result_set
        .update_clob_reader_by_label("clob", None, RdbcCharacterLength::Unspecified)
        .unwrap();
    result_set
        .update_clob_reader(
            ColumnIndex::new(4).unwrap(),
            Some(&clob),
            RdbcCharacterLength::Long(-11),
        )
        .unwrap();
    result_set
        .update_clob_reader_by_label("clob_long", None, RdbcCharacterLength::Long(13))
        .unwrap();
    result_set
        .update_n_clob_reader(
            ColumnIndex::new(5).unwrap(),
            Some(&n_clob),
            RdbcCharacterLength::Unspecified,
        )
        .unwrap();
    result_set
        .update_n_clob_reader_by_label("n_clob", None, RdbcCharacterLength::Unspecified)
        .unwrap();
    result_set
        .update_n_clob_reader(
            ColumnIndex::new(6).unwrap(),
            Some(&n_clob),
            RdbcCharacterLength::Long(-17),
        )
        .unwrap();
    result_set
        .update_n_clob_reader_by_label("n_clob_long", None, RdbcCharacterLength::Long(19))
        .unwrap();
    assert_eq!(
        call_log.lock().unwrap().clone(),
        [
            "result_set_update_blob_stream:1:some",
            "result_set_update_blob_stream_by_label:blob:none",
            "result_set_update_blob_stream_with_length:2:some:-7",
            "result_set_update_blob_stream_by_label_with_length:blob_long:none:9223372036854775807",
            "result_set_update_clob_reader:3:some",
            "result_set_update_clob_reader_by_label:clob:none",
            "result_set_update_clob_reader_with_length:4:some:-11",
            "result_set_update_clob_reader_by_label_with_length:clob_long:none:13",
            "result_set_update_n_clob_reader:5:some",
            "result_set_update_n_clob_reader_by_label:n_clob:none",
            "result_set_update_n_clob_reader_with_length:6:some:-17",
            "result_set_update_n_clob_reader_by_label_with_length:n_clob_long:none:19",
        ]
    );
    assert_eq!(blob.read_to_end().unwrap(), [2, 3]);
    assert_eq!(clob.read_to_string().unwrap(), "乙");
    assert_eq!(n_clob.read_to_string().unwrap(), "丁");

    // RUST_OBLIGATION / V1_RUST_LOCAL：Filter 错误只在原 Statement 分类一次，
    // 并且短路后不触达真实只读 RowSet。
    assert_eq!(
        result_set.update_n_clob_reader_by_label("fail", None, RdbcCharacterLength::Long(-23)),
        Err(RdbcError::DriverError(
            "filtered LOB stream update failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(pass_chain))).await;
    let mut result_set = statement
        .execute_query("SELECT X'010203' AS value")
        .await
        .unwrap();
    let blob = RdbcInputStream::from_bytes([4, 5, 6]);
    let clob = RdbcReader::from_string("戊己");
    let n_clob = RdbcReader::from_string("庚辛");

    macro_rules! assert_lob_stream_unsupported {
        ($expression:expr, $operation:literal) => {
            assert_eq!(
                $expression,
                Err(RdbcError::UnsupportedOperation {
                    operation: $operation,
                })
            );
        };
    }

    // VALUE_ADD / V5_HOST：真实 Toasty SQLite RowSet 对 12 个精确入口返回
    // 对应物理 SPI capability error，且每次失败只计数一次。
    assert_lob_stream_unsupported!(
        result_set.update_blob_stream(
            ColumnIndex::new(1).unwrap(),
            Some(&blob),
            RdbcStreamLength::Unspecified
        ),
        "result_set_update_blob_stream"
    );
    assert_lob_stream_unsupported!(
        result_set.update_blob_stream_by_label("blob", None, RdbcStreamLength::Unspecified),
        "result_set_update_blob_stream_by_label"
    );
    assert_lob_stream_unsupported!(
        result_set.update_blob_stream(
            ColumnIndex::new(2).unwrap(),
            Some(&blob),
            RdbcStreamLength::Long(-7)
        ),
        "result_set_update_blob_stream"
    );
    assert_lob_stream_unsupported!(
        result_set.update_blob_stream_by_label("blob_long", None, RdbcStreamLength::Long(i64::MAX)),
        "result_set_update_blob_stream_by_label"
    );
    assert_lob_stream_unsupported!(
        result_set.update_clob_reader(
            ColumnIndex::new(3).unwrap(),
            Some(&clob),
            RdbcCharacterLength::Unspecified
        ),
        "result_set_update_clob_reader"
    );
    assert_lob_stream_unsupported!(
        result_set.update_clob_reader_by_label("clob", None, RdbcCharacterLength::Unspecified),
        "result_set_update_clob_reader_by_label"
    );
    assert_lob_stream_unsupported!(
        result_set.update_clob_reader(
            ColumnIndex::new(4).unwrap(),
            Some(&clob),
            RdbcCharacterLength::Long(-11)
        ),
        "result_set_update_clob_reader"
    );
    assert_lob_stream_unsupported!(
        result_set.update_clob_reader_by_label("clob_long", None, RdbcCharacterLength::Long(13)),
        "result_set_update_clob_reader_by_label"
    );
    assert_lob_stream_unsupported!(
        result_set.update_n_clob_reader(
            ColumnIndex::new(5).unwrap(),
            Some(&n_clob),
            RdbcCharacterLength::Unspecified
        ),
        "result_set_update_n_clob_reader"
    );
    assert_lob_stream_unsupported!(
        result_set.update_n_clob_reader_by_label("n_clob", None, RdbcCharacterLength::Unspecified),
        "result_set_update_n_clob_reader_by_label"
    );
    assert_lob_stream_unsupported!(
        result_set.update_n_clob_reader(
            ColumnIndex::new(6).unwrap(),
            Some(&n_clob),
            RdbcCharacterLength::Long(-17)
        ),
        "result_set_update_n_clob_reader"
    );
    assert_lob_stream_unsupported!(
        result_set.update_n_clob_reader_by_label(
            "n_clob_long",
            None,
            RdbcCharacterLength::Long(19)
        ),
        "result_set_update_n_clob_reader_by_label"
    );
    assert_eq!(statement.exception_count(), 12);
    assert!(!result_set.is_closed().unwrap());
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
