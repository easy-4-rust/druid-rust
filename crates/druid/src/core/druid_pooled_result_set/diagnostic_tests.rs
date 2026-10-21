//! 用真实 SQLite 验证私有结果集上下文，保留迁移前的状态合同。

use super::DruidPooledResultSet;
use crate::core::druid_pooled_statement::DruidPooledStatement;
use crate::core::owner_disposition_control::OwnerDispositionControl;
use crate::core::standard_logical_policy::StandardLogicalPolicy;
use crate::core::standard_resource_registry::StandardResourceRegistry;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{Statement, StatementOptions};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

mod initial_column_map_filter;
mod n_string_update_short_circuit_filter;
mod ordered_n_string_update_filter;

/// 默认链保留完整描述符；A/B/C 正向进入，反向退出，B 短路不触及 C 或驱动。
#[test]
fn task6_n_string_update_default_order_short_circuit_and_error_contracts() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError, ResultSetFilterContext};
    use druid_rdbc::result_set::ResultSetUpdate;
    use ordered_n_string_update_filter::OrderedNStringUpdateFilter;
    use std::sync::Mutex;
    use update_call::UpdateCall;
    use update_identity_probe::UpdateIdentityProbe;
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
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetUpdate;
    use n_string_update_short_circuit_filter::NStringUpdateShortCircuitFilter;
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
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetUpdate;
    use n_string_update_short_circuit_filter::NStringUpdateShortCircuitFilter;
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

mod sql_capture_filter;

mod lob_stream_update_short_circuit_filter;

#[tokio::test]
async fn task6_pooled_result_set_routes_lob_stream_updates_and_preserves_sqlite_capability_errors()
{
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::{RdbcCharacterLength, RdbcInputStream, RdbcReader, RdbcStreamLength};
    use lob_stream_update_short_circuit_filter::LobStreamUpdateShortCircuitFilter;
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

#[test]
fn task6_result_set_scalar_update_filter_defaults_delegate_all_java_overloads() {
    use crate::core::{FilterAdapter, FilterChain, ResultSetFilterContext};
    use druid_rdbc::result_set::ResultSetUpdate;
    use druid_rdbc::types::{RdbcBigDecimal, RdbcNaiveDate, RdbcNaiveTime};
    use std::str::FromStr;
    use update_call::UpdateCall;
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut raw = update_identity_probe::UpdateIdentityProbe::new(Arc::clone(&calls));
    let context = ResultSetFilterContext::new();
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::new(FilterAdapter::new()));
    let decimal = RdbcBigDecimal::from_str("12.340").unwrap();
    let date = RdbcNaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = RdbcNaiveTime::from_hms_opt(10, 11, 12).unwrap();
    let timestamp = date.and_time(time);
    // 完整保留 BASE 的14种setter及下标/原列名28次调用，默认Filter继续到驱动末端。
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(1).unwrap(),
            ResultSetUpdate::Null,
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "null_value",
            ResultSetUpdate::Null,
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(2).unwrap(),
            ResultSetUpdate::Boolean(true),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "boolean_value",
            ResultSetUpdate::Boolean(false),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(3).unwrap(),
            ResultSetUpdate::Byte(-8),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "byte_value",
            ResultSetUpdate::Byte(8),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(4).unwrap(),
            ResultSetUpdate::Short(-16),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "short_value",
            ResultSetUpdate::Short(16),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(5).unwrap(),
            ResultSetUpdate::Int(-32),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "int_value",
            ResultSetUpdate::Int(32),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(6).unwrap(),
            ResultSetUpdate::Long(-64),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "long_value",
            ResultSetUpdate::Long(64),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(7).unwrap(),
            ResultSetUpdate::Float(1.25),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "float_value",
            ResultSetUpdate::Float(-1.25),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(8).unwrap(),
            ResultSetUpdate::Double(2.5),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "double_value",
            ResultSetUpdate::Double(-2.5),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(9).unwrap(),
            ResultSetUpdate::BigDecimal(Some(decimal.clone())),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "decimal_value",
            ResultSetUpdate::BigDecimal(None),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(10).unwrap(),
            ResultSetUpdate::String(Some("index".to_owned())),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "string_value",
            ResultSetUpdate::String(None),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(11).unwrap(),
            ResultSetUpdate::Bytes(Some(vec![0, 255])),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "bytes_value",
            ResultSetUpdate::Bytes(None),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(12).unwrap(),
            ResultSetUpdate::Date(Some(date)),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "date_value",
            ResultSetUpdate::Date(None),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(13).unwrap(),
            ResultSetUpdate::Time(Some(time)),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "time_value",
            ResultSetUpdate::Time(None),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(14).unwrap(),
            ResultSetUpdate::Timestamp(Some(timestamp)),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "timestamp_value",
            ResultSetUpdate::Timestamp(None),
        )
        .unwrap();
    let observed = calls.lock().unwrap();
    assert_eq!(observed.len(), 28);
    assert_eq!(observed[0], UpdateCall::Index(1, ResultSetUpdate::Null));
    assert_eq!(
        observed[1],
        UpdateCall::Label("null_value".to_owned(), ResultSetUpdate::Null)
    );
    assert_eq!(
        observed[2],
        UpdateCall::Index(2, ResultSetUpdate::Boolean(true))
    );
    assert_eq!(
        observed[3],
        UpdateCall::Label("boolean_value".to_owned(), ResultSetUpdate::Boolean(false))
    );
    assert_eq!(
        format!("{:?}", observed[16]),
        "Index(9, BigDecimal(Some(BigDecimal(sign=Plus, scale=3, digits=[12340]))))"
    );
    assert_eq!(
        observed[17],
        UpdateCall::Label(
            "decimal_value".to_owned(),
            ResultSetUpdate::BigDecimal(None)
        )
    );
    assert_eq!(
        observed[20],
        UpdateCall::Index(11, ResultSetUpdate::Bytes(Some(vec![0, 255])))
    );
    assert_eq!(
        observed[27],
        UpdateCall::Label(
            "timestamp_value".to_owned(),
            ResultSetUpdate::Timestamp(None)
        )
    );
}

#[tokio::test]
async fn task6_strong_typed_getter_conversion_errors_follow_statement_check_exception_path() {
    use druid_rdbc::extensions::BufferedRowSet;
    use druid_rdbc::types::{RdbcCalendarArgument, Row, Value};
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut result_set = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(BufferedRowSet::with_column_labels(
            vec![Row::new(vec![Value::String("not-a-date".to_string())])],
            vec!["event_date".to_string()],
        )),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    assert!(result_set.next().await.unwrap());
    let error = result_set
        .date_by_label("event_date", &RdbcCalendarArgument::Unspecified)
        .unwrap_err();
    assert_eq!(error.class_name(), "java.sql.SQLDataException");
    assert_eq!(error.sql_exception().unwrap().sql_state(), Some("22007"));
    assert!(!error.to_string().contains("not-a-date"));
    assert!(matches!(
        result_set.blob(ColumnIndex::new(1).unwrap()),
        Err(crate::core::RdbcError::UnsupportedOperation {
            operation: "result_set_blob"
        })
    ));
    assert_eq!(statement.exception_count(), 2);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_map_get_object_overloads_preserve_index_label_and_nullable_map_identity() {
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::{RdbcObject, RdbcTargetType, RdbcTypeMap, Value};
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
    let mut type_map = RdbcTypeMap::new();
    type_map.insert("APP.MONEY", RdbcTargetType::BigDecimal);
    assert_eq!(
        result_set
            .object_with_type_map(ColumnIndex::new(31).unwrap(), Some(&type_map))
            .unwrap(),
        RdbcObject::Scalar(Value::Int(71))
    );
    assert_eq!(
        result_set
            .object_by_label_with_type_map("mapped", None)
            .unwrap(),
        RdbcObject::Scalar(Value::Int(72))
    );
    assert_eq!(
        *probe.lock().unwrap(),
        vec![
            strong_getter_call::StrongGetterCall::ObjectMapIndex(31, Some(type_map)),
            strong_getter_call::StrongGetterCall::ObjectMapLabel("mapped".to_owned(), None),
        ]
    );
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_typed_get_object_overloads_preserve_raw_target_type_identity() {
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::{RdbcObject, RdbcTargetType};
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
    assert_eq!(
        result_set
            .object_as(
                ColumnIndex::new(41).unwrap(),
                &RdbcTargetType::Custom("com.example.Money".to_owned())
            )
            .unwrap(),
        RdbcObject::String("typed-index".to_owned())
    );
    assert_eq!(
        result_set
            .object_by_label_as("payload", &RdbcTargetType::Bytes)
            .unwrap(),
        RdbcObject::String("typed-label".to_owned())
    );
    assert_eq!(
        *probe.lock().unwrap(),
        vec![
            strong_getter_call::StrongGetterCall::ObjectTypedIndex(
                41,
                RdbcTargetType::Custom("com.example.Money".to_owned())
            ),
            strong_getter_call::StrongGetterCall::ObjectTypedLabel(
                "payload".to_owned(),
                RdbcTargetType::Bytes
            ),
        ]
    );
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn object_label_overloads_keep_dynamic_dispatch_visibility_maps_and_closed_guards() {
    use druid_rdbc::types::{RdbcObject, RdbcTargetType, RdbcTypeMap, Value};
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let probe = Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = statement.inner.begin_standard_admission().unwrap();
    let rows = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(strong_getter_probe::StrongGetterProbe::new(Arc::clone(
            &probe,
        ))),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    rows.set_hidden_columns(Some(vec![2]));
    assert!(rows.object_with_type_map_by_label("mapped", None).is_err());
    assert!(rows
        .object_as_by_label("mapped", &RdbcTargetType::Bytes)
        .is_err());
    assert!(probe.lock().unwrap().is_empty());
    rows.set_hidden_columns(None);
    rows.set_logic_column_map(Some(HashMap::from([(1, 2)])));
    rows.set_physical_column_map(Some(HashMap::from([(2, 1)])));
    let mut map = RdbcTypeMap::new();
    map.insert("APP.MONEY", RdbcTargetType::BigDecimal);
    let mut standard: Box<dyn ResultSet> = Box::new(rows);
    assert_eq!(
        standard
            .object_with_type_map_by_label("mapped", Some(&map))
            .unwrap(),
        RdbcObject::Scalar(Value::Int(72))
    );
    assert_eq!(
        standard
            .object_as_by_label("mapped", &RdbcTargetType::Bytes)
            .unwrap(),
        RdbcObject::String("typed-label".to_owned())
    );
    assert_eq!(
        standard
            .object_as(ColumnIndex::new(1).unwrap(), &RdbcTargetType::String)
            .unwrap(),
        RdbcObject::String("typed-index".to_owned())
    );
    assert!(standard
        .object_with_type_map_by_label("missing", None)
        .is_err());
    assert!(standard
        .object_as(ColumnIndex::new(99).unwrap(), &RdbcTargetType::Bytes)
        .is_err());
    assert_eq!(
        *probe.lock().unwrap(),
        vec![
            strong_getter_call::StrongGetterCall::ObjectMapLabel("mapped".to_owned(), Some(map)),
            strong_getter_call::StrongGetterCall::ObjectTypedLabel(
                "mapped".to_owned(),
                RdbcTargetType::Bytes
            ),
            strong_getter_call::StrongGetterCall::ObjectTypedIndex(2, RdbcTargetType::String),
        ]
    );
    standard.close().await.unwrap();
    assert_eq!(
        standard.object_with_type_map_by_label("mapped", None),
        Err(crate::core::RdbcError::ConnectionDiscarded)
    );
    assert_eq!(
        standard.object_as_by_label("mapped", &RdbcTargetType::Bytes),
        Err(crate::core::RdbcError::ConnectionDiscarded)
    );
    assert_eq!(probe.lock().unwrap().len(), 3);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 执行原五行 SQL，再由实际标准资源准入和池化代理接管驱动游标。
/// 参数为空；返回游标、创建语句与仍有效的真实驱动连接。
async fn task6_sqlite_diagnostic_cursor() -> (
    DruidPooledResultSet,
    DruidPooledStatement,
    Box<dyn Connection>,
) {
    task6_sqlite_diagnostic_cursor_with_filters(None).await
}

async fn task6_sqlite_diagnostic_cursor_with_filters(
    filters: Option<Arc<crate::core::FilterChain>>,
) -> (
    DruidPooledResultSet,
    DruidPooledStatement,
    Box<dyn Connection>,
) {
    task6_sqlite_diagnostic_cursor_with_sql_and_filters("SELECT * FROM t", filters).await
}

async fn task6_sqlite_diagnostic_cursor_with_sql_and_filters(
    sql: &str,
    filters: Option<Arc<crate::core::FilterChain>>,
) -> (
    DruidPooledResultSet,
    DruidPooledStatement,
    Box<dyn Connection>,
) {
    let (statement, connection) = task6_sqlite_diagnostic_statement_with_filters(filters).await;
    let admission = statement.inner.begin_standard_admission().unwrap();
    let raw = statement
        .inner
        .statement
        .lock()
        .await
        .execute_query(sql)
        .await
        .unwrap();
    let rows = DruidPooledResultSet::new_standard(Arc::clone(&statement.inner), raw, &admission)
        .await
        .unwrap();
    drop(admission);
    (rows, statement, connection)
}

/// 只装配真实五行 SQLite、标准 Statement 和 registry，不预先执行查询。
async fn task6_sqlite_diagnostic_statement_with_filters(
    filters: Option<Arc<crate::core::FilterChain>>,
) -> (DruidPooledStatement, Box<dyn Connection>) {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    connection
        .exec(
            "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, val REAL)",
            vec![],
        )
        .await
        .unwrap();
    for i in 1..=5 {
        connection
            .exec(
                &format!("INSERT INTO t (id, name, val) VALUES ({i}, 'row{i}', {i}.5)"),
                vec![],
            )
            .await
            .unwrap();
    }
    let resources = Arc::new(StandardResourceRegistry::default());
    let generation = resources.current_generation();
    let lease_active = Arc::new(AtomicBool::new(true));
    let policy = StandardLogicalPolicy::new(
        1,
        "sqlite-diagnostics".to_owned(),
        Arc::clone(&lease_active),
        Arc::new(AtomicBool::new(false)),
        filters.clone(),
        Some(true),
        generation,
        Arc::new(OwnerDispositionControl::new(Box::new(|_| {}))),
    );
    let admission = resources.begin_admission(generation).unwrap();
    let raw_statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let statement = DruidPooledStatement::new_standard(
        raw_statement,
        StatementOptions::default(),
        Arc::clone(&resources),
        policy,
        1,
        1,
        Arc::new(AtomicU64::new(1)),
        Arc::new(AtomicU64::new(1)),
        lease_active,
        filters,
        None,
    );
    resources
        .register(&admission, Arc::clone(&statement.inner))
        .await
        .unwrap();
    drop(admission);
    (statement, connection)
}

#[tokio::test]
async fn task6_open_filter_column_map_rejects_missing_keys() {
    let mut filters = crate::core::FilterChain::new();
    filters.add_result_set(Arc::new(initial_column_map_filter::InitialColumnMapFilter));
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_filters(Some(Arc::new(filters))).await;
    assert_eq!(rows.driver_column(1), Some(2));
    assert_eq!(rows.driver_column(99), None);
    assert!(rows
        .logical_column_index(druid_rdbc::result_set::ColumnIndex::new(99).unwrap())
        .is_err());
    rows.next().await.unwrap();
    assert_eq!(
        rows.value(druid_rdbc::result_set::ColumnIndex::new(1).unwrap())
            .unwrap(),
        druid_rdbc::types::Value::String("row1".to_owned())
    );
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_hidden_columns_none_by_default() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    let alias = DruidPooledResultSet::from_tracked_resource(
        Arc::clone(&statement.inner),
        Arc::clone(&rows.resource),
    )
    .unwrap();
    assert_eq!(rows.hidden_column_count(), 0);
    assert!(rows.hidden_columns().is_none());
    rows.set_hidden_columns(Some(vec![3, 4]));
    assert_eq!(rows.hidden_column_count(), 2);
    assert_eq!(rows.hidden_columns(), Some(vec![3, 4]));
    assert_eq!(alias.hidden_columns(), Some(vec![3, 4]));
    rows.next().await.unwrap();
    assert!(alias.value(ColumnIndex::new(3).unwrap()).is_err());
    alias.set_hidden_columns(None);
    assert!(rows.hidden_columns().is_none());
    assert_eq!(rows.hidden_column_count(), 0);
    assert_eq!(
        rows.value(ColumnIndex::new(3).unwrap()).unwrap(),
        druid_rdbc::types::Value::Float(1.5)
    );
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_physical_column_identity_without_map() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    assert_eq!(rows.physical_column(1), Some(1));
    assert_eq!(rows.logic_column(1), Some(1));
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_set_logic_column_map_and_reverse() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    let alias = DruidPooledResultSet::from_tracked_resource(
        Arc::clone(&statement.inner),
        Arc::clone(&rows.resource),
    )
    .unwrap();
    rows.set_logic_column_map(Some(HashMap::from([(1, 2), (2, 1)])));
    assert_eq!(rows.physical_column(1), Some(2));
    assert_eq!(rows.physical_column(2), Some(1));
    assert_eq!(rows.physical_column(99), None);
    assert_eq!(alias.physical_column(1), Some(2));
    rows.set_physical_column_map(Some(HashMap::from([(2, 1), (1, 2)])));
    assert_eq!(rows.logic_column(2), Some(1));
    assert_eq!(alias.logic_column(99), None);
    assert!(alias
        .logical_column_index(ColumnIndex::new(99).unwrap())
        .is_err());
    rows.next().await.unwrap();
    assert_eq!(
        alias.value(ColumnIndex::new(1).unwrap()).unwrap(),
        druid_rdbc::types::Value::String("row1".to_owned())
    );
    assert_eq!(
        alias
            .logical_column_index(ColumnIndex::new(2).unwrap())
            .unwrap(),
        ColumnIndex::new(1).unwrap()
    );
    assert_eq!(alias.find_column("name"), Ok(ColumnIndex::new(1).unwrap()));
    assert_eq!(
        alias.find_column("missing"),
        Err(druid_rdbc::error::RdbcError::InvalidArgument(
            "unknown column: missing".to_owned()
        ))
    );
    rows.set_logic_column_map(None);
    assert_eq!(rows.physical_column(1), Some(1));
    assert_eq!(
        alias.value(ColumnIndex::new(1).unwrap()).unwrap(),
        druid_rdbc::types::Value::Int(1)
    );
    // 正反映射独立清除，保留 BASE 设置器的语义。
    assert_eq!(alias.logic_column(2), Some(1));
    alias.set_physical_column_map(None);
    assert_eq!(rows.logic_column(2), Some(2));
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_close_count_zero_before_close() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    assert_eq!(rows.filter_context.close_count(), 0);
    rows.close().await.unwrap();
    assert_eq!(rows.filter_context.close_count(), 1);
    // 重复关闭、创建者级联关闭及 registry 收尾不能再次增加成功关闭计数。
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    statement.inner.resources.close_all().await.unwrap();
    assert_eq!(rows.filter_context.close_count(), 1);
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_cursor_index_starts_at_zero() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    assert_eq!(rows.shared.cursor_index.load(Ordering::Acquire), 0);
    rows.next().await.unwrap();
    assert_eq!(rows.shared.cursor_index.load(Ordering::Acquire), 1);
    rows.next().await.unwrap();
    assert_eq!(rows.shared.cursor_index.load(Ordering::Acquire), 2);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_fetch_row_count_increments_with_next() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    assert_eq!(rows.filter_context.fetch_row_count(), 0);
    rows.next().await.unwrap();
    assert_eq!(rows.filter_context.fetch_row_count(), 1);
    rows.next().await.unwrap();
    assert_eq!(rows.filter_context.fetch_row_count(), 2);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_construct_elapsed_returns_duration() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    // BASE 仅读取构造耗时，没有要求无 Filter 时也必须产生计时值。
    let _ = rows.filter_context.elapsed();
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_result_set_id_is_nonzero() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    assert!(rows.id() > 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_result_set_attributes_empty_initially() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    assert_eq!(rows.attributes_size(), 0);
    assert!(rows.shared.attributes.is_empty());
    assert!(rows.attribute("missing").is_none());
    rows.put_attribute("k", crate::core::ProxyAttributeValue::new(42));
    assert_eq!(rows.attributes_size(), 1);
    assert_eq!(*rows.attribute("k").unwrap().downcast::<i32>().unwrap(), 42);
    rows.clear_attributes();
    assert_eq!(rows.attributes_size(), 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_raw_result_set_returns_standard_driver_cursor() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    // 原始驱动句柄只在内部访问；不重新公开已退休的驱动端口。
    {
        let raw = rows.resource.result_set.lock().await;
        assert!(!raw.as_ref().unwrap().is_closed().unwrap());
    }
    rows.close().await.unwrap();
    assert!(rows.resource.result_set.lock().await.is_none());
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 用真实查询返回的代理保留初始属性及共享存储合同。
/// 对应 Java: DruidPooledResultSet#getMetaData。
#[tokio::test]
async fn task6_meta_data_proxy_returns_proxy_with_attributes() {
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters("SELECT id, name FROM t", None).await;
    let proxy = rows.meta_data_proxy_core().unwrap();
    assert_eq!(proxy.attributes_size(), 0);
    assert_eq!(
        druid_rdbc::metadata::ResultSetMetaData::column_count(&proxy),
        2
    );
    proxy.put_attribute("k", crate::core::ProxyAttributeValue::new(42));
    let second = rows.meta_data_proxy_core().unwrap();
    assert_eq!(second.attributes_size(), 1);
    assert_eq!(
        *second.attribute("k").unwrap().downcast::<i32>().unwrap(),
        42
    );
    second.clear_attributes();
    assert_eq!(proxy.attributes_size(), 0);
    assert_eq!(rows.meta_data().unwrap().column_count(), 2);
    rows.close().await.unwrap();
    assert!(matches!(
        rows.meta_data_proxy_core(),
        Err(druid_rdbc::error::RdbcError::ConnectionDiscarded)
    ));
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 保留真实五行查询成功 getString 后累计长度的完整 BASE 合同。
/// 对应 Java: DruidPooledResultSet#getString。
#[tokio::test]
async fn task6_read_string_length_increments_on_string_get() {
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters("SELECT name FROM t LIMIT 1", None)
            .await;
    assert_eq!(rows.filter_context.read_string_length(), 0);
    rows.next().await.unwrap();
    let _ =
        druid_rdbc::result_set::ResultSetExt::string(&rows, ColumnIndex::new(1).unwrap()).unwrap();
    assert!(rows.filter_context.read_string_length() > 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 标准动态读取与 Filter 路径只累计成功字符串的 UTF-16 长度。
#[tokio::test]
async fn task6_string_reads_count_utf16_without_counting_generic_values() {
    for filtered in [false, true] {
        let filters = if filtered {
            let mut chain = crate::core::FilterChain::new();
            chain.add_result_set(Arc::new(crate::core::FilterAdapter::new()));
            Some(Arc::new(chain))
        } else {
            None
        };
        let (mut rows, mut statement, mut connection) =
            task6_sqlite_diagnostic_cursor_with_sql_and_filters(
                "SELECT 'A😀' AS text_value, NULL AS absent, 7 AS number",
                filters,
            )
            .await;
        rows.next().await.unwrap();
        let text = ColumnIndex::new(1).unwrap();
        assert_eq!(
            rows.value(text).unwrap(),
            druid_rdbc::types::Value::String("A😀".to_owned())
        );
        assert!(matches!(
            rows.object(text).unwrap(),
            druid_rdbc::types::RdbcObject::Scalar(druid_rdbc::types::Value::String(_))
        ));
        assert_eq!(rows.filter_context.read_string_length(), 0);
        let alias = DruidPooledResultSet::from_tracked_resource(
            Arc::clone(&statement.inner),
            Arc::clone(&rows.resource),
        )
        .unwrap();
        let mut standard: Box<dyn ResultSet> = Box::new(alias);
        assert_eq!(
            druid_rdbc::result_set::ResultSetExt::string(standard.as_ref(), text).unwrap(),
            Some("A😀".to_owned())
        );
        assert_eq!(rows.filter_context.read_string_length(), 3);
        assert_eq!(
            druid_rdbc::result_set::ResultSetExt::string(standard.as_ref(), text).unwrap(),
            Some("A😀".to_owned())
        );
        assert_eq!(rows.filter_context.read_string_length(), 6);
        assert_eq!(
            druid_rdbc::result_set::ResultSetExt::string(
                standard.as_ref(),
                ColumnIndex::new(2).unwrap()
            )
            .unwrap(),
            None
        );
        let error = druid_rdbc::result_set::ResultSetExt::string(
            standard.as_ref(),
            ColumnIndex::new(3).unwrap(),
        )
        .unwrap_err();
        assert_eq!(error.class_name(), "java.sql.SQLDataException");
        assert_eq!(error.sql_exception().unwrap().sql_state(), Some("22005"));
        assert!(matches!(
            druid_rdbc::result_set::ResultSetExt::string_by_label(standard.as_ref(), "missing"),
            Err(druid_rdbc::error::RdbcError::InvalidArgument(_))
        ));
        assert_eq!(rows.filter_context.read_string_length(), 6);
        standard.close().await.unwrap();
        assert!(matches!(
            druid_rdbc::result_set::ResultSetExt::string(standard.as_ref(), text),
            Err(druid_rdbc::error::RdbcError::ConnectionDiscarded)
        ));
        assert_eq!(rows.filter_context.read_string_length(), 6);
        rows.close().await.unwrap();
        statement.close().await.unwrap();
        connection.close().await.unwrap();
    }
}

/// 原 SQL 来源合同通过真实 Statement 执行与生产 ResultSet 上下文观察。
#[tokio::test]
async fn task6_sql_returns_some() {
    let capture = Arc::new(sql_capture_filter::SqlCaptureFilter::default());
    let mut filters = crate::core::FilterChain::new();
    filters.add_result_set(capture.clone());
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
    let mut rows = statement.execute_query("SELECT * FROM t").await.unwrap();
    let sql = capture.sql();
    assert!(sql.is_some());
    assert!(sql.unwrap().contains("SELECT"));
    assert_eq!(statement.last_sql().as_deref(), Some("SELECT * FROM t"));
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 原 InputStream/Reader 初始零值合同使用真实标准游标。
#[tokio::test]
async fn task6_stream_reader_counts_zero_initially() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    assert_eq!(rows.filter_context.open_input_stream_count(), 0);
    assert_eq!(rows.filter_context.open_reader_count(), 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

mod update_call;
mod update_identity_probe;
mod vendor_object_probe;
use druid_rdbc::result_set::ResultSetUpdate;
use druid_rdbc::types::{
    RdbcBigDecimal as BigDecimal, RdbcCharacterLength, RdbcInputStream, RdbcNaiveDate as NaiveDate,
    RdbcNaiveTime as NaiveTime, RdbcObject, RdbcOpaqueObject, RdbcReader, RdbcStreamLength, Value,
};
use std::str::FromStr;

/// 完整保留 BASE 56 次更新及流身份，重载映射为标准描述符。
/// 对应 Java: java.sql.ResultSet#updateXxx。
#[tokio::test]
async fn task6_all_scalar_and_stream_update_overloads_preserve_raw_spi_argument_identity() {
    let (statement, mut connection) = task6_sqlite_diagnostic_statement_with_filters(None).await;
    let probe = Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut result_set = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(update_identity_probe::UpdateIdentityProbe::new(Arc::clone(
            &probe,
        ))),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_nano_opt(13, 14, 15, 123_456_789).unwrap();
    let timestamp = date.and_time(time);
    let input = RdbcInputStream::from_bytes([1, 2, 3]);
    let reader = RdbcReader::from_string("严格流身份");
    let vendor_object =
        RdbcOpaqueObject::new(Arc::new(vendor_object_probe::VendorObjectProbe { id: 99 }));
    assert_eq!(
        vendor_object.downcast_ref::<vendor_object_probe::VendorObjectProbe>(),
        Some(&vendor_object_probe::VendorObjectProbe { id: 99 })
    );

    macro_rules! assert_last_update {
        ($expression:expr, $expected:expr) => {{
            $expression.unwrap();
            assert_eq!(
                probe.lock().unwrap().clone().last().cloned(),
                Some($expected)
            );
        }};
    }

    assert_last_update!(
        result_set.update_value(ColumnIndex::new(1).unwrap(), ResultSetUpdate::Null),
        update_call::UpdateCall::Index(1, ResultSetUpdate::Null)
    );
    assert_last_update!(
        result_set.update_value_by_label("null_value", ResultSetUpdate::Null),
        update_call::UpdateCall::Label("null_value".to_string(), ResultSetUpdate::Null)
    );

    macro_rules! assert_scalar_pair {
        (
            $variant:ident, $index_method:ident, $label_method:ident, $index:expr, $label:literal,
            $index_value:expr, $label_value:expr, $index_update:expr, $label_update:expr
        ) => {
            assert_last_update!(
                result_set.update_value(
                    ColumnIndex::new($index).unwrap(),
                    ResultSetUpdate::$variant($index_value)
                ),
                update_call::UpdateCall::Index($index, $index_update)
            );
            assert_last_update!(
                result_set.update_value_by_label($label, ResultSetUpdate::$variant($label_value)),
                update_call::UpdateCall::Label($label.to_string(), $label_update)
            );
        };
    }

    assert_scalar_pair!(
        Boolean,
        update_boolean,
        update_boolean_by_label,
        2,
        "boolean_value",
        true,
        false,
        ResultSetUpdate::Boolean(true),
        ResultSetUpdate::Boolean(false)
    );
    assert_scalar_pair!(
        Byte,
        update_byte,
        update_byte_by_label,
        3,
        "byte_value",
        -8,
        8,
        ResultSetUpdate::Byte(-8),
        ResultSetUpdate::Byte(8)
    );
    assert_scalar_pair!(
        Short,
        update_short,
        update_short_by_label,
        4,
        "short_value",
        -16,
        16,
        ResultSetUpdate::Short(-16),
        ResultSetUpdate::Short(16)
    );
    assert_scalar_pair!(
        Int,
        update_int,
        update_int_by_label,
        5,
        "int_value",
        -32,
        32,
        ResultSetUpdate::Int(-32),
        ResultSetUpdate::Int(32)
    );
    assert_scalar_pair!(
        Long,
        update_long,
        update_long_by_label,
        6,
        "long_value",
        -64,
        64,
        ResultSetUpdate::Long(-64),
        ResultSetUpdate::Long(64)
    );
    assert_scalar_pair!(
        Float,
        update_float,
        update_float_by_label,
        7,
        "float_value",
        1.25,
        -1.25,
        ResultSetUpdate::Float(1.25),
        ResultSetUpdate::Float(-1.25)
    );
    assert_scalar_pair!(
        Double,
        update_double,
        update_double_by_label,
        8,
        "double_value",
        2.5,
        -2.5,
        ResultSetUpdate::Double(2.5),
        ResultSetUpdate::Double(-2.5)
    );
    let decimal = BigDecimal::from_str("1234567890.123456789").unwrap();
    assert_scalar_pair!(
        BigDecimal,
        update_big_decimal,
        update_big_decimal_by_label,
        9,
        "decimal_value",
        Some(decimal.clone()),
        None,
        ResultSetUpdate::BigDecimal(Some(decimal)),
        ResultSetUpdate::BigDecimal(None)
    );
    assert_scalar_pair!(
        String,
        update_string,
        update_string_by_label,
        10,
        "string_value",
        Some("index".to_string()),
        None,
        ResultSetUpdate::String(Some("index".to_string())),
        ResultSetUpdate::String(None)
    );
    assert_scalar_pair!(
        Bytes,
        update_bytes,
        update_bytes_by_label,
        11,
        "bytes_value",
        Some(vec![0, 255]),
        None,
        ResultSetUpdate::Bytes(Some(vec![0, 255])),
        ResultSetUpdate::Bytes(None)
    );
    assert_scalar_pair!(
        Date,
        update_date,
        update_date_by_label,
        12,
        "date_value",
        Some(date),
        None,
        ResultSetUpdate::Date(Some(date)),
        ResultSetUpdate::Date(None)
    );
    assert_scalar_pair!(
        Time,
        update_time,
        update_time_by_label,
        13,
        "time_value",
        Some(time),
        None,
        ResultSetUpdate::Time(Some(time)),
        ResultSetUpdate::Time(None)
    );
    assert_scalar_pair!(
        Timestamp,
        update_timestamp,
        update_timestamp_by_label,
        14,
        "timestamp_value",
        Some(timestamp),
        None,
        ResultSetUpdate::Timestamp(Some(timestamp)),
        ResultSetUpdate::Timestamp(None)
    );
    assert_last_update!(
        result_set.update_value(
            ColumnIndex::new(15).unwrap(),
            ResultSetUpdate::Object(RdbcObject::Custom(vendor_object.clone()))
        ),
        update_call::UpdateCall::Index(
            15,
            ResultSetUpdate::Object(RdbcObject::Custom(vendor_object))
        )
    );
    assert_last_update!(
        result_set
            .update_value_by_label("object_value", ResultSetUpdate::Object(Value::Null.into())),
        update_call::UpdateCall::Label(
            "object_value".to_string(),
            ResultSetUpdate::Object(Value::Null.into())
        )
    );
    assert_last_update!(
        result_set.update_value(
            ColumnIndex::new(16).unwrap(),
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: Value::Decimal(BigDecimal::from(7)).into(),
                scale_or_length: -3
            }
        ),
        update_call::UpdateCall::Index(
            16,
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: Value::Decimal(BigDecimal::from(7)).into(),
                scale_or_length: -3,
            }
        )
    );
    assert_last_update!(
        result_set.update_value_by_label(
            "scaled_object",
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: Value::Bytes(vec![9]).into(),
                scale_or_length: 99
            }
        ),
        update_call::UpdateCall::Label(
            "scaled_object".to_string(),
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: Value::Bytes(vec![9]).into(),
                scale_or_length: 99,
            }
        )
    );
    assert_scalar_pair!(
        NString,
        update_n_string,
        update_n_string_by_label,
        17,
        "n_string_value",
        Some("国家".to_string()),
        None,
        ResultSetUpdate::NString(Some("国家".to_string())),
        ResultSetUpdate::NString(None)
    );

    macro_rules! assert_stream_call {
        ($expression:expr, $target:expr, $variant:ident, $length:expr) => {
            assert_last_update!(
                $expression,
                $target(ResultSetUpdate::$variant {
                    stream: Some(input.clone()),
                    length: $length,
                })
            );
        };
    }
    assert_stream_call!(
        result_set.update_value(
            ColumnIndex::new(18).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Index(18, update),
        AsciiStream,
        RdbcStreamLength::Unspecified
    );
    assert_stream_call!(
        result_set.update_value_by_label(
            "ascii",
            ResultSetUpdate::AsciiStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Label("ascii".to_string(), update),
        AsciiStream,
        RdbcStreamLength::Unspecified
    );
    assert_stream_call!(
        result_set.update_value(
            ColumnIndex::new(19).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Int(-19)
            }
        ),
        |update| update_call::UpdateCall::Index(19, update),
        AsciiStream,
        RdbcStreamLength::Int(-19)
    );
    assert_stream_call!(
        result_set.update_value_by_label(
            "ascii_int",
            ResultSetUpdate::AsciiStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Int(20)
            }
        ),
        |update| update_call::UpdateCall::Label("ascii_int".to_string(), update),
        AsciiStream,
        RdbcStreamLength::Int(20)
    );
    assert_stream_call!(
        result_set.update_value(
            ColumnIndex::new(20).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Long(-21)
            }
        ),
        |update| update_call::UpdateCall::Index(20, update),
        AsciiStream,
        RdbcStreamLength::Long(-21)
    );
    assert_stream_call!(
        result_set.update_value_by_label(
            "ascii_long",
            ResultSetUpdate::AsciiStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Long(22)
            }
        ),
        |update| update_call::UpdateCall::Label("ascii_long".to_string(), update),
        AsciiStream,
        RdbcStreamLength::Long(22)
    );
    assert_stream_call!(
        result_set.update_value(
            ColumnIndex::new(21).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Index(21, update),
        BinaryStream,
        RdbcStreamLength::Unspecified
    );
    assert_stream_call!(
        result_set.update_value_by_label(
            "binary",
            ResultSetUpdate::BinaryStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Label("binary".to_string(), update),
        BinaryStream,
        RdbcStreamLength::Unspecified
    );
    assert_stream_call!(
        result_set.update_value(
            ColumnIndex::new(22).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Int(-23)
            }
        ),
        |update| update_call::UpdateCall::Index(22, update),
        BinaryStream,
        RdbcStreamLength::Int(-23)
    );
    assert_stream_call!(
        result_set.update_value_by_label(
            "binary_int",
            ResultSetUpdate::BinaryStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Int(24)
            }
        ),
        |update| update_call::UpdateCall::Label("binary_int".to_string(), update),
        BinaryStream,
        RdbcStreamLength::Int(24)
    );
    assert_stream_call!(
        result_set.update_value(
            ColumnIndex::new(23).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Long(-25)
            }
        ),
        |update| update_call::UpdateCall::Index(23, update),
        BinaryStream,
        RdbcStreamLength::Long(-25)
    );
    assert_stream_call!(
        result_set.update_value_by_label(
            "binary_long",
            ResultSetUpdate::BinaryStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Long(26)
            }
        ),
        |update| update_call::UpdateCall::Label("binary_long".to_string(), update),
        BinaryStream,
        RdbcStreamLength::Long(26)
    );

    macro_rules! assert_reader_call {
        ($expression:expr, $target:expr, $variant:ident, $length:expr) => {
            assert_last_update!(
                $expression,
                $target(ResultSetUpdate::$variant {
                    reader: Some(reader.clone()),
                    length: $length,
                })
            );
        };
    }
    assert_reader_call!(
        result_set.update_value(
            ColumnIndex::new(24).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Index(24, update),
        CharacterStream,
        RdbcCharacterLength::Unspecified
    );
    assert_reader_call!(
        result_set.update_value_by_label(
            "character",
            ResultSetUpdate::CharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Label("character".to_string(), update),
        CharacterStream,
        RdbcCharacterLength::Unspecified
    );
    assert_reader_call!(
        result_set.update_value(
            ColumnIndex::new(25).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Int(-27)
            }
        ),
        |update| update_call::UpdateCall::Index(25, update),
        CharacterStream,
        RdbcCharacterLength::Int(-27)
    );
    assert_reader_call!(
        result_set.update_value_by_label(
            "character_int",
            ResultSetUpdate::CharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Int(28)
            }
        ),
        |update| update_call::UpdateCall::Label("character_int".to_string(), update),
        CharacterStream,
        RdbcCharacterLength::Int(28)
    );
    assert_reader_call!(
        result_set.update_value(
            ColumnIndex::new(26).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Long(-29)
            }
        ),
        |update| update_call::UpdateCall::Index(26, update),
        CharacterStream,
        RdbcCharacterLength::Long(-29)
    );
    assert_reader_call!(
        result_set.update_value_by_label(
            "character_long",
            ResultSetUpdate::CharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Long(30)
            }
        ),
        |update| update_call::UpdateCall::Label("character_long".to_string(), update),
        CharacterStream,
        RdbcCharacterLength::Long(30)
    );
    assert_reader_call!(
        result_set.update_value(
            ColumnIndex::new(27).unwrap(),
            ResultSetUpdate::NCharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Index(27, update),
        NCharacterStream,
        RdbcCharacterLength::Unspecified
    );
    assert_reader_call!(
        result_set.update_value_by_label(
            "n_character",
            ResultSetUpdate::NCharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Label("n_character".to_string(), update),
        NCharacterStream,
        RdbcCharacterLength::Unspecified
    );
    assert_reader_call!(
        result_set.update_value(
            ColumnIndex::new(28).unwrap(),
            ResultSetUpdate::NCharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Long(-31)
            }
        ),
        |update| update_call::UpdateCall::Index(28, update),
        NCharacterStream,
        RdbcCharacterLength::Long(-31)
    );
    assert_reader_call!(
        result_set.update_value_by_label(
            "n_character_long",
            ResultSetUpdate::NCharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Long(32)
            }
        ),
        |update| update_call::UpdateCall::Label("n_character_long".to_string(), update),
        NCharacterStream,
        RdbcCharacterLength::Long(32)
    );

    assert_eq!(probe.lock().unwrap().clone().len(), 56);
    assert!(!input.is_closed());
    assert!(!reader.is_closed());
    result_set.close().await.unwrap();
    let mut statement = statement;
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
/// 动态标准入口保留列名，同时拒绝隐藏列、缺失映射和关闭租约。
#[tokio::test]
async fn task6_update_label_preserves_dynamic_identity_and_column_governance() {
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut rows = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(update_identity_probe::UpdateIdentityProbe::new(Arc::clone(
            &calls,
        ))),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    rows.set_hidden_columns(Some(vec![2]));
    assert!(
        matches!(rows.update_value_by_label("mapped", ResultSetUpdate::Null), Err(druid_rdbc::error::RdbcError::InvalidArgument(message)) if message == "column is hidden")
    );
    assert!(calls.lock().unwrap().is_empty());
    rows.set_hidden_columns(None);
    rows.set_logic_column_map(Some(HashMap::from([(7, 2)])));
    assert!(
        matches!(rows.update_value_by_label("mapped", ResultSetUpdate::Null), Err(druid_rdbc::error::RdbcError::InvalidArgument(message)) if message == "column is hidden")
    );
    assert!(calls.lock().unwrap().is_empty());
    rows.set_physical_column_map(Some(HashMap::from([(2, 7)])));
    rows.update_value_by_label("mapped", ResultSetUpdate::Int(31))
        .unwrap();
    assert_eq!(
        calls.lock().unwrap().last().cloned(),
        Some(update_call::UpdateCall::Label(
            "mapped".to_owned(),
            ResultSetUpdate::Int(31)
        ))
    );
    rows.set_logic_column_map(None);
    rows.set_physical_column_map(None);
    let alias = DruidPooledResultSet::from_tracked_resource(
        Arc::clone(&statement.inner),
        Arc::clone(&rows.resource),
    )
    .unwrap();
    let mut standard: Box<dyn ResultSet> = Box::new(alias);
    standard
        .update_value_by_label("raw_label", ResultSetUpdate::String(None))
        .unwrap();
    assert_eq!(
        calls.lock().unwrap().last().cloned(),
        Some(update_call::UpdateCall::Label(
            "raw_label".to_owned(),
            ResultSetUpdate::String(None)
        ))
    );
    standard.close().await.unwrap();
    assert!(matches!(
        rows.update_value_by_label("raw_label", ResultSetUpdate::Null),
        Err(druid_rdbc::error::RdbcError::ConnectionDiscarded)
    ));
    assert_eq!(calls.lock().unwrap().len(), 2);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

mod strong_getter_call;
mod strong_getter_probe;
use druid_rdbc::result_set::ResultSetExt;
use druid_rdbc::types::{RdbcCalendar, RdbcCalendarArgument};

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

mod strong_getter_filter;

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

mod sparse_result_set;
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

mod scalar_getter_delegation_probe;
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
    use druid_rdbc::types::{RdbcObject, RdbcOpaqueObject, Value};
    use vendor_object_probe::VendorObjectProbe;

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

mod n_string_read_probe;
mod n_string_short_circuit_filter;
mod strong_getter_short_circuit_filter;

/// 完整保留原 NString 默认重载、短路、错误计数和真实 Unicode 合同。
#[tokio::test]
async fn task6_pooled_result_set_preserves_distinct_n_string_filter_overloads() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }

    let physical_log = Arc::new(Mutex::new(Vec::new()));
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(pass_chain))).await;
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut physical_probe = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(n_string_read_probe::NStringReadProbe::new(Arc::clone(
            &physical_log,
        ))),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    assert_eq!(
        physical_probe
            .n_string(ColumnIndex::new(1).unwrap())
            .unwrap(),
        Some("索引-NString".to_string())
    );
    assert_eq!(
        physical_probe.n_string_by_label("name").unwrap(),
        Some("标签-NString".to_string())
    );
    assert_eq!(
        calls(&physical_log),
        ["physical:n_string:1", "physical:n_string_by_label:name"]
    );
    physical_probe.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(
        n_string_short_circuit_filter::NStringShortCircuitFilter {
            calls: Arc::clone(&call_log),
        },
    ));
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filter_chain))).await;
    let mut result_set = statement.execute_query("SELECT '原值'").await.unwrap();
    assert_eq!(
        result_set.n_string(ColumnIndex::new(3).unwrap()).unwrap(),
        Some("过滤-索引".to_string())
    );
    assert_eq!(
        result_set.n_string_by_label("name").unwrap(),
        Some("过滤-标签".to_string())
    );
    assert_eq!(
        calls(&call_log),
        [
            "result_set_get_n_string:3",
            "result_set_get_n_string_by_label:name"
        ]
    );
    assert_eq!(
        result_set.n_string_by_label("fail"),
        Err(RdbcError::DriverError(
            "filtered NString failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    // 真实 Toasty SQLite Unicode 值通过默认委托的独立 NString 入口读取。
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(pass_chain))).await;
    let mut result_set = statement
        .execute_query("SELECT '你好，Druid'")
        .await
        .unwrap();
    assert!(result_set.next().await.unwrap());
    assert_eq!(
        result_set.n_string(ColumnIndex::new(1).unwrap()).unwrap(),
        Some("你好，Druid".to_string())
    );
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// NString 的独立短路同样必须服从隐藏列与关闭边界。
#[tokio::test]
async fn task6_n_string_filter_short_circuit_obeys_hidden_columns_and_closed_guards() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    let log = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(
        n_string_short_circuit_filter::NStringShortCircuitFilter {
            calls: Arc::clone(&log),
        },
    ));
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters(
            "SELECT '原值' AS value",
            Some(Arc::new(filters)),
        )
        .await;
    rows.set_hidden_columns(Some(vec![1]));
    assert_eq!(
        rows.n_string_by_label("value"),
        Err(RdbcError::InvalidArgument("column is hidden".to_owned()))
    );
    assert!(log.lock().unwrap().is_empty());
    assert_eq!(statement.exception_count(), 1);
    rows.set_hidden_columns(None);
    assert_eq!(
        rows.n_string_by_label("fail"),
        Err(RdbcError::DriverError(
            "filtered NString failure".to_owned()
        ))
    );
    assert_eq!(statement.exception_count(), 2);
    assert_eq!(
        *log.lock().unwrap(),
        vec!["result_set_get_n_string_by_label:fail"]
    );
    rows.close().await.unwrap();
    assert_eq!(
        rows.n_string_by_label("virtual"),
        Err(RdbcError::ConnectionDiscarded)
    );
    assert_eq!(
        rows.n_string(ColumnIndex::new(3).unwrap()),
        Err(RdbcError::ConnectionDiscarded)
    );
    assert_eq!(statement.exception_count(), 2);
    assert_eq!(log.lock().unwrap().len(), 1);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_result_set_preserves_decimal_temporal_and_calendar_filter_overloads() {
    use crate::core::{FilterChain, RdbcError};
    use bigdecimal::BigDecimal;
    use chrono::{NaiveDate, NaiveTime};
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::{RdbcCalendar, RdbcCalendarArgument};
    use std::sync::Mutex;
    use strong_getter_short_circuit_filter::StrongGetterShortCircuitFilter;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(StrongGetterShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filter_chain))).await;
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();
    assert_eq!(
        result_set
            .big_decimal(ColumnIndex::new(1).unwrap())
            .unwrap(),
        Some(BigDecimal::from(101))
    );
    assert_eq!(
        result_set.big_decimal_by_label("decimal").unwrap(),
        Some(BigDecimal::from(102))
    );
    assert_eq!(
        result_set
            .big_decimal_with_scale(ColumnIndex::new(2).unwrap(), 7)
            .unwrap(),
        Some(BigDecimal::from(103))
    );
    assert_eq!(
        result_set
            .big_decimal_by_label_with_scale("decimal_scale", 8)
            .unwrap(),
        Some(BigDecimal::from(104))
    );

    assert_eq!(
        result_set
            .date(
                ColumnIndex::new(3).unwrap(),
                &RdbcCalendarArgument::Unspecified
            )
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2031, 1, 1).unwrap())
    );
    assert_eq!(
        result_set
            .date_by_label("date", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2031, 1, 2).unwrap())
    );
    assert_eq!(
        result_set
            .date(
                ColumnIndex::new(4).unwrap(),
                &RdbcCalendarArgument::Specified(None)
            )
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2031, 1, 3).unwrap())
    );
    assert_eq!(
        result_set
            .date_by_label(
                "date_calendar",
                &RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("UTC").unwrap()))
            )
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2031, 1, 3).unwrap())
    );

    assert_eq!(
        result_set
            .time(
                ColumnIndex::new(5).unwrap(),
                &RdbcCalendarArgument::Unspecified
            )
            .unwrap(),
        Some(NaiveTime::from_hms_opt(1, 2, 3).unwrap())
    );
    assert_eq!(
        result_set
            .time_by_label("time", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(NaiveTime::from_hms_opt(2, 3, 4).unwrap())
    );
    assert_eq!(
        result_set
            .time(
                ColumnIndex::new(6).unwrap(),
                &RdbcCalendarArgument::Specified(None)
            )
            .unwrap(),
        Some(NaiveTime::from_hms_opt(3, 4, 5).unwrap())
    );
    assert_eq!(
        result_set
            .time_by_label(
                "time_calendar",
                &RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("UTC").unwrap()))
            )
            .unwrap(),
        Some(NaiveTime::from_hms_opt(3, 4, 5).unwrap())
    );

    let index_timestamp = NaiveDate::from_ymd_opt(2031, 1, 1)
        .unwrap()
        .and_hms_opt(1, 2, 3)
        .unwrap();
    let label_timestamp = NaiveDate::from_ymd_opt(2031, 1, 2)
        .unwrap()
        .and_hms_opt(2, 3, 4)
        .unwrap();
    let calendar_timestamp = NaiveDate::from_ymd_opt(2031, 1, 3)
        .unwrap()
        .and_hms_opt(3, 4, 5)
        .unwrap();
    assert_eq!(
        result_set
            .timestamp(
                ColumnIndex::new(7).unwrap(),
                &RdbcCalendarArgument::Unspecified
            )
            .unwrap(),
        Some(index_timestamp)
    );
    assert_eq!(
        result_set
            .timestamp_by_label("timestamp", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(label_timestamp)
    );
    assert_eq!(
        result_set
            .timestamp(
                ColumnIndex::new(8).unwrap(),
                &RdbcCalendarArgument::Specified(None)
            )
            .unwrap(),
        Some(calendar_timestamp)
    );
    assert_eq!(
        result_set
            .timestamp_by_label(
                "timestamp_calendar",
                &RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("UTC").unwrap()))
            )
            .unwrap(),
        Some(calendar_timestamp)
    );

    assert_eq!(
        calls(&call_log),
        [
            "result_set_get_big_decimal:1",
            "result_set_get_big_decimal_by_label:decimal",
            "result_set_get_big_decimal_with_scale:2:7",
            "result_set_get_big_decimal_by_label_with_scale:decimal_scale:8",
            "result_set_get_date:3",
            "result_set_get_date_by_label:date",
            "result_set_get_date_with_calendar:4:specified:null",
            "result_set_get_date_by_label_with_calendar:date_calendar:specified:UTC",
            "result_set_get_time:5",
            "result_set_get_time_by_label:time",
            "result_set_get_time_with_calendar:6:specified:null",
            "result_set_get_time_by_label_with_calendar:time_calendar:specified:UTC",
            "result_set_get_timestamp:7",
            "result_set_get_timestamp_by_label:timestamp",
            "result_set_get_timestamp_with_calendar:8:specified:null",
            "result_set_get_timestamp_by_label_with_calendar:timestamp_calendar:specified:UTC",
        ]
    );

    assert_eq!(
        result_set.timestamp_by_label(
            "fail",
            &RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("UTC").unwrap()))
        ),
        Err(RdbcError::DriverError(
            "filtered temporal failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_temporal_filter_short_circuit_obeys_hidden_columns_and_closed_guards() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::RdbcCalendarArgument;
    let log = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(
        strong_getter_short_circuit_filter::StrongGetterShortCircuitFilter {
            calls: Arc::clone(&log),
        },
    ));
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters(
            "SELECT 1 AS value",
            Some(Arc::new(filters)),
        )
        .await;
    rows.set_hidden_columns(Some(vec![1]));
    assert_eq!(
        rows.timestamp_by_label("value", &RdbcCalendarArgument::Specified(None)),
        Err(RdbcError::InvalidArgument("column is hidden".to_owned()))
    );
    assert!(log.lock().unwrap().is_empty());
    assert_eq!(statement.exception_count(), 1);
    rows.set_hidden_columns(None);
    assert_eq!(
        rows.timestamp_by_label("fail", &RdbcCalendarArgument::Specified(None)),
        Err(RdbcError::DriverError(
            "filtered temporal failure".to_owned()
        ))
    );
    assert_eq!(statement.exception_count(), 2);
    assert_eq!(
        *log.lock().unwrap(),
        vec!["result_set_get_timestamp_by_label_with_calendar:fail:specified:null"]
    );
    rows.close().await.unwrap();
    assert_eq!(
        rows.timestamp_by_label("virtual", &RdbcCalendarArgument::Specified(None)),
        Err(RdbcError::ConnectionDiscarded)
    );
    assert_eq!(statement.exception_count(), 2);
    assert_eq!(log.lock().unwrap().len(), 1);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

mod object_short_circuit_filter;

mod object_read_probe;

mod object_payload;

mod ordered_object_filter;

#[tokio::test]
async fn task6_pooled_result_set_routes_all_object_overloads_and_preserves_argument_identity() {
    use crate::core::{FilterChain, RdbcError, Value};
    use druid_rdbc::types::{RdbcObject, RdbcTargetType, RdbcTypeMap};
    use object_short_circuit_filter::ObjectShortCircuitFilter;
    use std::sync::Mutex;
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let argument_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(ObjectShortCircuitFilter {
        calls: Arc::clone(&call_log),
        arguments: Arc::clone(&argument_log),
    }));

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let resources = Arc::new(StandardResourceRegistry::default());
    let generation = resources.current_generation();
    let lease_active = Arc::new(AtomicBool::new(true));
    let filters = Some(Arc::new(filter_chain));
    let policy = StandardLogicalPolicy::new(
        75,
        "sqlite-object-filter".to_owned(),
        Arc::clone(&lease_active),
        Arc::new(AtomicBool::new(false)),
        filters.clone(),
        Some(true),
        generation,
        Arc::new(OwnerDispositionControl::new(Box::new(|_| {}))),
    );
    let admission = resources.begin_admission(generation).unwrap();
    let raw_statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut statement = DruidPooledStatement::new_standard(
        raw_statement,
        StatementOptions::default(),
        Arc::clone(&resources),
        policy,
        75,
        1,
        Arc::new(AtomicU64::new(1)),
        Arc::new(AtomicU64::new(1)),
        lease_active,
        filters,
        None,
    );
    resources
        .register(&admission, Arc::clone(&statement.inner))
        .await
        .unwrap();
    drop(admission);
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();

    let empty_map = RdbcTypeMap::new();
    let mut populated_map = RdbcTypeMap::new();
    populated_map.insert(
        "example.address",
        RdbcTargetType::Custom("example.Address".to_string()),
    );
    let custom_target = RdbcTargetType::Custom("example.Target".to_string());
    let string_target = RdbcTargetType::String;

    assert_eq!(
        result_set.object(ColumnIndex::new(1).unwrap()).unwrap(),
        RdbcObject::Scalar(Value::Int(901))
    );
    assert_eq!(
        result_set.object_by_label("value").unwrap(),
        RdbcObject::Scalar(Value::String("filtered-label-object".to_string()))
    );
    assert_eq!(
        result_set
            .object_with_type_map(ColumnIndex::new(2).unwrap(), None)
            .unwrap(),
        RdbcObject::String("filtered-map:null".to_string())
    );
    assert_eq!(
        result_set
            .object_with_type_map(ColumnIndex::new(3).unwrap(), Some(&empty_map))
            .unwrap(),
        RdbcObject::String("filtered-map:empty".to_string())
    );
    assert_eq!(
        result_set
            .object_with_type_map_by_label("mapped", Some(&populated_map),)
            .unwrap(),
        RdbcObject::String(
            "filtered-label-map:example.address=Custom(\"example.Address\")".to_string()
        )
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(4).unwrap(), &custom_target)
            .unwrap(),
        RdbcObject::String("filtered-typed:Custom(\"example.Target\")".to_string())
    );
    assert_eq!(
        result_set
            .object_as_by_label("typed", &string_target)
            .unwrap(),
        RdbcObject::String("filtered-label-typed:String".to_string())
    );

    assert_eq!(
        call_log.lock().unwrap().clone(),
        [
            "result_set_get_object:1",
            "result_set_get_object_by_label:value",
            "result_set_get_object_with_type_map:2:null",
            "result_set_get_object_with_type_map:3:empty",
            "result_set_get_object_by_label_with_type_map:mapped:example.address=Custom(\"example.Address\")",
            "result_set_get_object_typed:4:Custom(\"example.Target\")",
            "result_set_get_object_typed_by_label:typed:String",
        ]
    );

    // 补充观察原映射和目标类型引用，不能只比较重新构造后的内容。
    assert_eq!(
        *argument_log.lock().unwrap(),
        vec![
            std::ptr::from_ref(&empty_map) as usize,
            std::ptr::from_ref(&populated_map) as usize,
            std::ptr::from_ref(&custom_target) as usize,
            std::ptr::from_ref(&string_target) as usize
        ],
    );
    assert_eq!(
        result_set.object_as_by_label("fail", &RdbcTargetType::Custom("example.Fail".to_string()),),
        Err(RdbcError::DriverError(
            "filtered object failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 六种对象读取短路均服从隐藏列、精确错误计数和关闭边界。
/// 对应 Java: DruidPooledResultSet#getObject 的所有重载。
#[tokio::test]
async fn task6_object_filter_short_circuit_obeys_hidden_columns_and_closed_guards() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::types::RdbcTargetType;
    use object_short_circuit_filter::ObjectShortCircuitFilter;
    use std::sync::Mutex;

    let calls = Arc::new(Mutex::new(Vec::new()));
    let arguments = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(ObjectShortCircuitFilter {
        calls: Arc::clone(&calls),
        arguments: Arc::clone(&arguments),
    }));
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters(
            "SELECT '原值' AS value",
            Some(Arc::new(filters)),
        )
        .await;
    let column = ColumnIndex::new(1).unwrap();
    let target = RdbcTargetType::Custom("example.Guarded".to_owned());

    // 所有重载使用同一真实列治理，拒绝发生在Filter执行之前。
    rows.set_hidden_columns(Some(vec![1]));
    let hidden_results = [
        rows.object(column),
        rows.object_by_label("value"),
        rows.object_with_type_map(column, None),
        rows.object_with_type_map_by_label("value", None),
        rows.object_as(column, &target),
        rows.object_as_by_label("value", &target),
    ];
    for result in hidden_results {
        assert_eq!(
            result,
            Err(RdbcError::InvalidArgument("column is hidden".to_owned()))
        );
    }
    assert!(calls.lock().unwrap().is_empty());
    assert!(arguments.lock().unwrap().is_empty());
    assert_eq!(statement.exception_count(), 6);

    rows.set_hidden_columns(None);
    assert_eq!(
        rows.object_as_by_label("fail", &target),
        Err(RdbcError::DriverError("filtered object failure".to_owned()))
    );
    assert_eq!(statement.exception_count(), 7);
    assert_eq!(
        *calls.lock().unwrap(),
        vec!["result_set_get_object_typed_by_label:fail:Custom(\"example.Guarded\")"]
    );
    assert_eq!(
        *arguments.lock().unwrap(),
        vec![std::ptr::from_ref(&target) as usize]
    );

    // 关闭后的虚拟标签不能通过短路复活游标，也不能额外记失败或调用Filter。
    rows.close().await.unwrap();
    let closed_results = [
        rows.object(column),
        rows.object_by_label("virtual"),
        rows.object_with_type_map(column, None),
        rows.object_with_type_map_by_label("virtual", None),
        rows.object_as(column, &target),
        rows.object_as_by_label("virtual", &target),
    ];
    for result in closed_results {
        assert_eq!(result, Err(RdbcError::ConnectionDiscarded));
    }
    assert_eq!(statement.exception_count(), 7);
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(arguments.lock().unwrap().len(), 1);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 默认对象Filter链保持六种原生入口、参数引用和资源返回身份。
#[tokio::test]
async fn task6_object_filter_default_preserves_resource_identity_and_native_overloads() {
    use crate::core::{FilterAdapter, FilterChain};
    use druid_rdbc::types::{
        RdbcObject, RdbcOpaqueObject, RdbcReader, RdbcTargetType, RdbcTypeMap,
    };
    use object_payload::ObjectPayload;
    use object_read_probe::ObjectReadProbe;
    use std::sync::Mutex;
    let stream = RdbcReader::from_string("资源对象😀");
    let payload = RdbcOpaqueObject::new(Arc::new(ObjectPayload("原对象".to_owned())));
    for object in [
        RdbcObject::CharacterStream(stream.clone()),
        RdbcObject::Custom(payload.clone()),
    ] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let arguments = Arc::new(Mutex::new(Vec::new()));
        let mut filters = FilterChain::new();
        filters.add_result_set(Arc::new(FilterAdapter::new()));
        filters.add_result_set(Arc::new(FilterAdapter::new()));
        let (mut statement, mut connection) =
            task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
        let admission = statement.inner.begin_standard_admission().unwrap();
        let mut rows = DruidPooledResultSet::new_standard(
            Arc::clone(&statement.inner),
            Box::new(ObjectReadProbe {
                calls: Arc::clone(&calls),
                arguments: Arc::clone(&arguments),
                object: object.clone(),
                closed: false,
            }),
            &admission,
        )
        .await
        .unwrap();
        drop(admission);
        let map = RdbcTypeMap::new();
        let target = RdbcTargetType::Custom("example.Resource".to_owned());
        let index = ColumnIndex::new(1).unwrap();
        // 无行定位、无额外find_column、无目标类型预转换，原资源句柄无损返回。
        for returned in [
            rows.object(index),
            rows.object_by_label("ordinary"),
            rows.object_with_type_map(index, None),
            rows.object_with_type_map_by_label("mapped", Some(&map)),
            rows.object_as(index, &target),
            rows.object_as_by_label("typed", &target),
        ] {
            assert_eq!(returned.unwrap(), object);
        }
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                "raw:ordinary:1",
                "raw:ordinary_label:ordinary",
                "raw:map:1:false",
                "raw:map_label:mapped:true",
                "raw:typed:1:Custom(\"example.Resource\")",
                "raw:typed_label:typed:Custom(\"example.Resource\")",
            ]
        );
        assert_eq!(
            *arguments.lock().unwrap(),
            vec![
                std::ptr::from_ref(&map) as usize,
                std::ptr::from_ref(&target) as usize,
                std::ptr::from_ref(&target) as usize,
            ]
        );
        assert_eq!(statement.exception_count(), 0);
        match rows.object_as_by_label("typed", &target).unwrap() {
            RdbcObject::CharacterStream(returned) => {
                let mut prefix = [0_u16; 2];
                assert_eq!(returned.read_utf16(&mut prefix).unwrap(), 2);
                assert_eq!(String::from_utf16(&prefix).unwrap(), "资源");
                assert_eq!(stream.read_to_string().unwrap(), "对象😀");
            }
            RdbcObject::Custom(returned) => {
                assert_eq!(returned, payload);
                assert!(std::ptr::eq(returned.value(), payload.value()));
                assert_eq!(
                    returned.downcast_ref::<ObjectPayload>().unwrap().0,
                    "原对象"
                );
            }
            other => panic!("完整对象不能转换为标量: {other:?}"),
        }
        rows.close().await.unwrap();
        statement.close().await.unwrap();
        connection.close().await.unwrap();
    }
}

/// 六入口共享注册顺序，错误按反向回退，资源短路不接触原驱动。
#[tokio::test]
async fn task6_object_filters_preserve_order_short_circuit_and_error_unwind() {
    use crate::core::{FilterChain, RdbcError, Value};
    use druid_rdbc::types::{RdbcObject, RdbcReader, RdbcTargetType, RdbcTypeMap};
    use object_read_probe::ObjectReadProbe;
    use ordered_object_filter::OrderedObjectFilter;
    use std::sync::Mutex;
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    for name in ["A", "B"] {
        filters.add_result_set(Arc::new(OrderedObjectFilter {
            name,
            calls: Arc::clone(&log),
            short_circuit: None,
        }));
    }
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut rows = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(ObjectReadProbe {
            calls: Arc::clone(&log),
            arguments: Arc::new(Mutex::new(Vec::new())),
            object: RdbcObject::Scalar(Value::Int(17)),
            closed: false,
        }),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    let map = RdbcTypeMap::new();
    let target = RdbcTargetType::Custom("example.Order".to_owned());
    let column = ColumnIndex::new(1).unwrap();
    let returned = [
        rows.object(column),
        rows.object_by_label("ordinary"),
        rows.object_with_type_map(column, None),
        rows.object_with_type_map_by_label("mapped", Some(&map)),
        rows.object_as(column, &target),
        rows.object_as_by_label("typed", &target),
    ];
    for result in returned {
        assert_eq!(result.unwrap(), RdbcObject::Scalar(Value::Int(17)));
    }
    let terminals = [
        "raw:ordinary:1",
        "raw:ordinary_label:ordinary",
        "raw:map:1:false",
        "raw:map_label:mapped:true",
        "raw:typed:1:Custom(\"example.Order\")",
        "raw:typed_label:typed:Custom(\"example.Order\")",
    ];
    let expected: Vec<String> = terminals
        .into_iter()
        .flat_map(|terminal| ["A:before", "B:before", terminal, "B:ok", "A:ok"].map(str::to_owned))
        .collect();
    assert_eq!(*log.lock().unwrap(), expected);
    assert_eq!(statement.exception_count(), 0);
    log.lock().unwrap().clear();
    assert_eq!(
        rows.object_as_by_label("fail", &target),
        Err(RdbcError::DriverError("object probe failure".to_owned()))
    );
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            "A:before",
            "B:before",
            "raw:typed_label:fail:Custom(\"example.Order\")",
            "B:error",
            "A:error"
        ]
    );
    assert_eq!(statement.exception_count(), 1);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    // 中间Filter原样返回资源，后续Filter和原生驱动均不得执行。
    let log = Arc::new(Mutex::new(Vec::new()));
    let stream = RdbcReader::from_string("短路资源😀");
    let object = RdbcObject::CharacterStream(stream.clone());
    let mut filters = FilterChain::new();
    for (name, short_circuit) in [("A", None), ("B", Some(object.clone())), ("C", None)] {
        filters.add_result_set(Arc::new(OrderedObjectFilter {
            name,
            calls: Arc::clone(&log),
            short_circuit,
        }));
    }
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters(
            "SELECT '原值' AS value",
            Some(Arc::new(filters)),
        )
        .await;
    let virtual_column = ColumnIndex::new(3).unwrap();
    for result in [
        rows.object(virtual_column),
        rows.object_by_label("virtual"),
        rows.object_with_type_map(virtual_column, None),
        rows.object_with_type_map_by_label("virtual", Some(&map)),
        rows.object_as(virtual_column, &target),
        rows.object_as_by_label("virtual", &target),
    ] {
        assert_eq!(result.unwrap(), object);
    }
    assert_eq!(
        *log.lock().unwrap(),
        ["A:before", "B:before", "B:ok", "A:ok"]
            .into_iter()
            .cycle()
            .take(24)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    );
    assert_eq!(stream.read_to_string().unwrap(), "短路资源😀");
    assert_eq!(statement.exception_count(), 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 活着的原生游标也不得接受旧逻辑租约的六种对象读取。
/// 对应 Java: DruidPooledResultSet#getObject 的逻辑连接生命周期检查。
#[tokio::test]
async fn task6_object_filter_expired_lease_rejects_before_hooks_and_native_reads() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::types::RdbcTargetType;
    use object_short_circuit_filter::ObjectShortCircuitFilter;
    use std::sync::Mutex;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let arguments = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(ObjectShortCircuitFilter {
        calls: Arc::clone(&calls),
        arguments: Arc::clone(&arguments),
    }));
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters(
            "SELECT '原值' AS value",
            Some(Arc::new(filters)),
        )
        .await;
    assert!(!rows.is_closed().unwrap());
    assert!(!rows
        .resource
        .result_set
        .lock()
        .await
        .as_ref()
        .unwrap()
        .is_closed()
        .unwrap());
    statement.inner.lease_active.store(false, Ordering::Release);
    let column = ColumnIndex::new(1).unwrap();
    let target = RdbcTargetType::Custom("example.Expired".to_owned());
    for result in [
        rows.object(column),
        rows.object_by_label("virtual"),
        rows.object_with_type_map(column, None),
        rows.object_with_type_map_by_label("virtual", None),
        rows.object_as(column, &target),
        rows.object_as_by_label("virtual", &target),
    ] {
        assert_eq!(result, Err(RdbcError::ConnectionDiscarded));
    }
    assert!(calls.lock().unwrap().is_empty());
    assert!(arguments.lock().unwrap().is_empty());
    assert_eq!(statement.exception_count(), 0);
    // 原游标没有被读取失败误关闭；后续显式清理不能恢复旧租约。
    assert!(!rows
        .resource
        .result_set
        .lock()
        .await
        .as_ref()
        .unwrap()
        .is_closed()
        .unwrap());
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

mod ordered_scalar_filter;
mod scalar_read_probe;
mod scalar_short_circuit_filter;

/// 完整保留九类标量原重载、顺序、短路和精确错误计数；候选未运行。
#[tokio::test]
async fn task6_pooled_result_set_routes_all_scalar_getters_through_java_filter_overloads() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    use scalar_short_circuit_filter::ScalarShortCircuitFilter;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(ScalarShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let resources = Arc::new(StandardResourceRegistry::default());
    let generation = resources.current_generation();
    let lease_active = Arc::new(AtomicBool::new(true));
    let filters = Some(Arc::new(filter_chain));
    let policy = StandardLogicalPolicy::new(
        73,
        "sqlite-scalar-filter".to_owned(),
        Arc::clone(&lease_active),
        Arc::new(AtomicBool::new(false)),
        filters.clone(),
        Some(true),
        generation,
        Arc::new(OwnerDispositionControl::new(Box::new(|_| {}))),
    );
    let admission = resources.begin_admission(generation).unwrap();
    let raw_statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut statement = DruidPooledStatement::new_standard(
        raw_statement,
        StatementOptions::default(),
        Arc::clone(&resources),
        policy,
        73,
        1,
        Arc::new(AtomicU64::new(1)),
        Arc::new(AtomicU64::new(1)),
        lease_active,
        filters,
        None,
    );
    resources
        .register(&admission, Arc::clone(&statement.inner))
        .await
        .unwrap();
    drop(admission);
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();

    // Filter 在 raw ResultSet 前短路，因此未调用 next 也能返回改写值。
    assert_eq!(
        result_set.string(ColumnIndex::new(1).unwrap()).unwrap(),
        Some("filtered-index".to_string())
    );
    assert_eq!(
        result_set.string_by_label("string").unwrap(),
        Some("filtered-label".to_string())
    );
    assert!(result_set.boolean(ColumnIndex::new(2).unwrap()).unwrap());
    assert!(!result_set.boolean_by_label("boolean").unwrap());
    assert_eq!(result_set.byte(ColumnIndex::new(3).unwrap()).unwrap(), 11);
    assert_eq!(result_set.byte_by_label("byte").unwrap(), 12);
    assert_eq!(result_set.short(ColumnIndex::new(4).unwrap()).unwrap(), 21);
    assert_eq!(result_set.short_by_label("short").unwrap(), 22);
    assert_eq!(result_set.int(ColumnIndex::new(5).unwrap()).unwrap(), 31);
    assert_eq!(result_set.int_by_label("int").unwrap(), 32);
    assert_eq!(result_set.long(ColumnIndex::new(6).unwrap()).unwrap(), 41);
    assert_eq!(result_set.long_by_label("long").unwrap(), 42);
    assert_eq!(
        result_set.float(ColumnIndex::new(7).unwrap()).unwrap(),
        51.5
    );
    assert_eq!(result_set.float_by_label("float").unwrap(), 52.5);
    assert_eq!(
        result_set.double(ColumnIndex::new(8).unwrap()).unwrap(),
        61.5
    );
    assert_eq!(result_set.double_by_label("double").unwrap(), 62.5);
    assert_eq!(
        result_set.bytes(ColumnIndex::new(9).unwrap()).unwrap(),
        Some(vec![7, 8])
    );
    assert_eq!(
        result_set.bytes_by_label("bytes").unwrap(),
        Some(vec![9, 10])
    );

    assert_eq!(
        calls(&call_log),
        [
            "result_set_get_string:1",
            "result_set_get_string_by_label:string",
            "result_set_get_boolean:2",
            "result_set_get_boolean_by_label:boolean",
            "result_set_get_byte:3",
            "result_set_get_byte_by_label:byte",
            "result_set_get_short:4",
            "result_set_get_short_by_label:short",
            "result_set_get_int:5",
            "result_set_get_int_by_label:int",
            "result_set_get_long:6",
            "result_set_get_long_by_label:long",
            "result_set_get_float:7",
            "result_set_get_float_by_label:float",
            "result_set_get_double:8",
            "result_set_get_double_by_label:double",
            "result_set_get_bytes:9",
            "result_set_get_bytes_by_label:bytes",
        ]
    );

    assert_eq!(
        result_set.double_by_label("fail"),
        Err(RdbcError::DriverError(
            "filtered double failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 九类标量短路仍须服从全部下标/标签隐藏列、关闭及失效租约边界；候选未运行。
#[tokio::test]
async fn task6_scalar_filter_all_overloads_obey_hidden_closed_and_expired_lease_guards() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::{ResultSetExt, ResultSetRead};
    use scalar_short_circuit_filter::ScalarShortCircuitFilter;
    use std::sync::Mutex;

    let requests = [
        ResultSetRead::String,
        ResultSetRead::Boolean,
        ResultSetRead::Byte,
        ResultSetRead::Short,
        ResultSetRead::Int,
        ResultSetRead::Long,
        ResultSetRead::Float,
        ResultSetRead::Double,
        ResultSetRead::Bytes,
    ];
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(ScalarShortCircuitFilter {
        calls: Arc::clone(&log),
    }));
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters(
            "SELECT 1 AS value",
            Some(Arc::new(filters)),
        )
        .await;
    rows.set_hidden_columns(Some(vec![1]));
    for request in &requests {
        assert_eq!(
            rows.read_object(ColumnIndex::new(1).unwrap(), request),
            Err(RdbcError::InvalidArgument("column is hidden".to_owned()))
        );
        assert_eq!(
            rows.read_object_by_label("value", request),
            Err(RdbcError::InvalidArgument("column is hidden".to_owned()))
        );
    }
    assert!(log.lock().unwrap().is_empty());
    // 沿完整对象治理合同，每个实际失败入口只向原创建语句记错一次。
    assert_eq!(statement.exception_count(), 18);
    rows.set_hidden_columns(None);
    assert_eq!(
        rows.double_by_label("fail"),
        Err(RdbcError::DriverError("filtered double failure".to_owned()))
    );
    assert_eq!(statement.exception_count(), 19);
    assert_eq!(
        log.lock().unwrap().as_slice(),
        ["result_set_get_double_by_label:fail"]
    );
    rows.close().await.unwrap();
    for request in &requests {
        assert_eq!(
            rows.read_object(ColumnIndex::new(1).unwrap(), request),
            Err(RdbcError::ConnectionDiscarded)
        );
        assert_eq!(
            rows.read_object_by_label("value", request),
            Err(RdbcError::ConnectionDiscarded)
        );
    }
    assert_eq!(statement.exception_count(), 19);
    assert_eq!(log.lock().unwrap().len(), 1);
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    let expired_log = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(ScalarShortCircuitFilter {
        calls: Arc::clone(&expired_log),
    }));
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters(
            "SELECT 1 AS value",
            Some(Arc::new(filters)),
        )
        .await;
    statement.inner.lease_active.store(false, Ordering::Release);
    for request in &requests {
        assert_eq!(
            rows.read_object(ColumnIndex::new(1).unwrap(), request),
            Err(RdbcError::ConnectionDiscarded)
        );
        assert_eq!(
            rows.read_object_by_label("value", request),
            Err(RdbcError::ConnectionDiscarded)
        );
    }
    assert!(expired_log.lock().unwrap().is_empty());
    assert_eq!(statement.exception_count(), 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 九类原生默认委托及注册顺序/短路/错误回退；候选未运行。
#[tokio::test]
async fn task6_scalar_filter_preserves_all_native_overloads_order_short_circuit_and_error_unwind() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetRead;
    use druid_rdbc::types::{RdbcObject, Value};
    use ordered_scalar_filter::OrderedScalarFilter;
    use scalar_read_probe::{scalar_stem, ScalarReadProbe};
    use std::sync::Mutex;
    let requests = [
        ResultSetRead::String,
        ResultSetRead::Boolean,
        ResultSetRead::Byte,
        ResultSetRead::Short,
        ResultSetRead::Int,
        ResultSetRead::Long,
        ResultSetRead::Float,
        ResultSetRead::Double,
        ResultSetRead::Bytes,
    ];
    let expected = [
        Value::String("原生😀字符串".to_owned()),
        Value::Bool(true),
        Value::Int(11),
        Value::Int(21),
        Value::Int(31),
        Value::Int(41),
        Value::Float(51.5),
        Value::Float(61.5),
        Value::Bytes(vec![7, 8]),
    ];

    // 默认标签保留原生重载，不为下标value hook额外定位；完整read hook仍包围调用。
    let native_log = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(FilterAdapter::new()));
    filters.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut rows = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(ScalarReadProbe {
            calls: Arc::clone(&native_log),
            closed: false,
        }),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    let mut expected_log = Vec::new();
    for (request, value) in requests.iter().zip(&expected) {
        assert_eq!(
            rows.read_object(ColumnIndex::new(7).unwrap(), request),
            Ok(RdbcObject::Scalar(value.clone()))
        );
        assert_eq!(
            rows.read_object_by_label("virtual", request),
            Ok(RdbcObject::Scalar(value.clone()))
        );
        let stem = scalar_stem(request);
        expected_log.extend([
            format!("raw:{stem}:7"),
            format!("raw:{stem}_by_label:virtual"),
        ]);
    }
    assert_eq!(*native_log.lock().unwrap(), expected_log);
    // 两次真实字符串成功读取按Java UTF-16长度计数，emoji占两个单元。
    assert_eq!(rows.filter_context.read_string_length(), 14);
    assert_eq!(statement.exception_count(), 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    let log = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    for name in ["A", "B"] {
        filters.add_result_set(Arc::new(OrderedScalarFilter {
            name,
            calls: Arc::clone(&log),
            short_circuit: false,
        }));
    }
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut rows = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(ScalarReadProbe {
            calls: Arc::clone(&log),
            closed: false,
        }),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    for (request, value) in requests.iter().zip(&expected) {
        log.lock().unwrap().clear();
        assert_eq!(
            rows.read_object(ColumnIndex::new(7).unwrap(), request),
            Ok(RdbcObject::Scalar(value.clone()))
        );
        assert_eq!(
            *log.lock().unwrap(),
            [
                "A:before".to_owned(),
                "B:before".to_owned(),
                format!("raw:{}:7", scalar_stem(request)),
                "B:ok".to_owned(),
                "A:ok".to_owned()
            ]
        );
        log.lock().unwrap().clear();
        assert_eq!(
            rows.read_object_by_label("virtual", request),
            Ok(RdbcObject::Scalar(value.clone()))
        );
        assert_eq!(
            *log.lock().unwrap(),
            [
                "A:before".to_owned(),
                "B:before".to_owned(),
                format!("raw:{}_by_label:virtual", scalar_stem(request)),
                "B:ok".to_owned(),
                "A:ok".to_owned()
            ]
        );
    }
    log.lock().unwrap().clear();
    assert_eq!(
        rows.read_object_by_label("fail", &ResultSetRead::Double),
        Err(RdbcError::DriverError("scalar probe failure".to_owned()))
    );
    assert_eq!(
        *log.lock().unwrap(),
        [
            "A:before",
            "B:before",
            "raw:double_by_label:fail",
            "B:error",
            "A:error"
        ]
    );
    assert_eq!(statement.exception_count(), 1);
    assert_eq!(rows.filter_context.read_string_length(), 14);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    let log = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    for name in ["A", "B", "C"] {
        filters.add_result_set(Arc::new(OrderedScalarFilter {
            name,
            calls: Arc::clone(&log),
            short_circuit: name == "B",
        }));
    }
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
    let mut rows = statement.execute_query("SELECT 1 AS value").await.unwrap();
    for request in &requests {
        for label in [None, Some("virtual")] {
            log.lock().unwrap().clear();
            let result = match label {
                Some(label) => rows.read_object_by_label(label, request),
                None => rows.read_object(ColumnIndex::new(7).unwrap(), request),
            };
            assert_eq!(result, Ok(RdbcObject::Scalar(Value::Null)));
            assert_eq!(
                *log.lock().unwrap(),
                ["A:before", "B:before", "B:ok", "A:ok"]
            );
        }
    }
    assert_eq!(statement.exception_count(), 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

mod statement_short_circuit_filter;

/// 完整创建语句/替换对象身份与错误合同；通过双向状态变更观察共享逻辑身份。
#[tokio::test]
async fn task6_pooled_result_set_routes_dynamic_statement_identity_through_filter() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::statement::{CallableStatement, PreparedStatement};
    use statement_short_circuit_filter::StatementShortCircuitFilter;
    use std::any::TypeId;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let replacement = Arc::new(Mutex::new(None));
    let fail = Arc::new(AtomicBool::new(false));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(StatementShortCircuitFilter {
        calls: Arc::clone(&call_log),
        replacement: Arc::clone(&replacement),
        fail: Arc::clone(&fail),
    }));
    filter_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let resources = Arc::new(StandardResourceRegistry::default());
    let generation = resources.current_generation();
    let lease_active = Arc::new(AtomicBool::new(true));
    let filters = Some(Arc::new(filter_chain));
    let policy = StandardLogicalPolicy::new(
        84,
        "sqlite-statement-filter".to_owned(),
        Arc::clone(&lease_active),
        Arc::new(AtomicBool::new(false)),
        filters.clone(),
        Some(true),
        generation,
        Arc::new(OwnerDispositionControl::new(Box::new(|_| {}))),
    );
    let admission = resources.begin_admission(generation).unwrap();
    let mut statements = Vec::new();
    for id in [1, 2] {
        let raw = connection
            .create_statement(StatementOptions::default())
            .await
            .unwrap();
        let statement = DruidPooledStatement::new_standard(
            raw,
            StatementOptions::default(),
            Arc::clone(&resources),
            Arc::clone(&policy),
            84,
            id,
            Arc::new(AtomicU64::new(1)),
            Arc::new(AtomicU64::new(1)),
            Arc::clone(&lease_active),
            filters.clone(),
            None,
        );
        resources
            .register(&admission, Arc::clone(&statement.inner))
            .await
            .unwrap();
        statements.push(statement);
    }
    drop(admission);
    let mut source_statement = statements.pop().unwrap();
    let mut replacement_statement = statements.pop().unwrap();
    let mut result_set = source_statement.execute_query("SELECT 1").await.unwrap();
    source_statement.set_fetch_size(11).unwrap();
    replacement_statement.set_fetch_size(31).unwrap();
    let mut returned = result_set.get_statement().unwrap().unwrap();
    assert!(returned
        .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
        .unwrap());
    assert_eq!(returned.fetch_size().unwrap(), 11);
    returned.set_fetch_size(17).unwrap();
    assert_eq!(source_statement.fetch_size().unwrap(), 17);
    assert!(!returned
        .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
        .unwrap());
    assert!(!returned
        .is_wrapper_for(Some(TypeId::of::<dyn CallableStatement>()))
        .unwrap());
    assert!(!returned.is_closed().unwrap());
    drop(returned);
    replacement
        .lock()
        .unwrap()
        .replace(replacement_statement.clone());
    let mut returned = result_set.get_statement().unwrap().unwrap();
    assert!(returned
        .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
        .unwrap());
    assert_eq!(returned.fetch_size().unwrap(), 31);
    returned.set_fetch_size(37).unwrap();
    assert_eq!(replacement_statement.fetch_size().unwrap(), 37);
    assert_eq!(source_statement.fetch_size().unwrap(), 17);
    source_statement.set_fetch_size(19).unwrap();
    assert_eq!(returned.fetch_size().unwrap(), 37);
    assert!(!returned
        .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
        .unwrap());
    assert!(!returned
        .is_wrapper_for(Some(TypeId::of::<dyn CallableStatement>()))
        .unwrap());
    assert!(!returned.is_closed().unwrap());
    assert!(!source_statement.is_closed());
    assert!(!replacement_statement.is_closed());
    drop(returned);
    fail.store(true, Ordering::Release);
    assert_eq!(
        result_set.get_statement().map(|_| ()),
        Err(RdbcError::DriverError(
            "filtered statement failure".to_owned()
        ))
    );
    assert_eq!(source_statement.exception_count(), 1);
    assert_eq!(
        calls(&call_log),
        [
            "result_set_get_statement",
            "result_set_get_statement",
            "result_set_get_statement"
        ]
    );
    fail.store(false, Ordering::Release);
    result_set.close().await.unwrap();
    source_statement.close().await.unwrap();
    replacement_statement.close().await.unwrap();
    connection.close().await.unwrap();
}

mod ordered_statement_filter;

/// 原逻辑所有者的关闭身份继续可查；旧租约必须在Filter前拒绝。
#[tokio::test]
async fn task6_statement_filter_order_closed_identity_and_expired_lease_are_observable() {
    use crate::core::{FilterChain, RdbcError};
    use ordered_statement_filter::OrderedStatementFilter;
    use std::sync::Mutex;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    for name in ["A", "B"] {
        filters.add_result_set(Arc::new(OrderedStatementFilter {
            name,
            calls: Arc::clone(&calls),
            short_circuit: false,
            fail: false,
        }));
    }
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_filters(Some(Arc::new(filters))).await;
    let mut returned = rows.get_statement().unwrap().unwrap();
    returned.set_fetch_size(43).unwrap();
    assert_eq!(statement.fetch_size().unwrap(), 43);
    assert_eq!(
        *calls.lock().unwrap(),
        ["A:before", "B:before", "B:ok", "A:ok"]
    );
    drop(returned);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    calls.lock().unwrap().clear();
    let returned = rows.get_statement().unwrap().unwrap();
    assert!(returned.is_closed().unwrap());
    assert_eq!(
        *calls.lock().unwrap(),
        ["A:before", "B:before", "B:ok", "A:ok"]
    );
    drop(returned);
    calls.lock().unwrap().clear();
    statement.inner.lease_active.store(false, Ordering::Release);
    assert_eq!(
        rows.get_statement().map(|_| ()),
        Err(RdbcError::ConnectionDiscarded)
    );
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(statement.exception_count(), 0);
    connection.close().await.unwrap();
}

/// 空句柄短路不进入C或末端；精确错误反向退出且只记原语句一次。
#[tokio::test]
async fn task6_statement_filter_short_circuit_and_error_unwind_preserve_owner_counts() {
    use crate::core::{FilterChain, RdbcError};
    use ordered_statement_filter::OrderedStatementFilter;
    use std::sync::Mutex;
    for fail in [false, true] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut filters = FilterChain::new();
        for name in ["A", "B", "C"] {
            filters.add_result_set(Arc::new(OrderedStatementFilter {
                name,
                calls: Arc::clone(&calls),
                short_circuit: name == "B",
                fail: fail && name == "B",
            }));
        }
        let (mut rows, mut statement, mut connection) =
            task6_sqlite_diagnostic_cursor_with_filters(Some(Arc::new(filters))).await;
        if fail {
            assert_eq!(
                rows.get_statement().map(|_| ()),
                Err(RdbcError::DriverError(
                    "ordered statement failure".to_owned()
                ))
            );
            assert_eq!(
                *calls.lock().unwrap(),
                ["A:before", "B:before", "B:error", "A:error"]
            );
            assert_eq!(statement.exception_count(), 1);
        } else {
            assert!(rows.get_statement().unwrap().is_none());
            assert_eq!(
                *calls.lock().unwrap(),
                ["A:before", "B:before", "B:ok", "A:ok"]
            );
            assert_eq!(statement.exception_count(), 0);
        }
        assert!(!statement.is_closed());
        rows.close().await.unwrap();
        statement.close().await.unwrap();
        connection.close().await.unwrap();
    }
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    rows.has_creating_statement = false;
    assert!(rows.get_statement().unwrap().is_none());
    assert_eq!(statement.exception_count(), 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

mod metadata_read_probe;
mod metadata_short_circuit_filter;

/// 创建指定原上下文的真实SQLite标准语句，登记实际资源所有权。
async fn task6_sqlite_metadata_statement(
    factory: &ToastyConnectionFactory,
    id: u64,
    name: &str,
    filters: Arc<crate::core::FilterChain>,
) -> (DruidPooledStatement, Box<dyn Connection>) {
    let mut connection = factory.create().await.unwrap();
    let resources = Arc::new(StandardResourceRegistry::default());
    let generation = resources.current_generation();
    let lease_active = Arc::new(AtomicBool::new(true));
    let policy = StandardLogicalPolicy::new(
        id,
        name.to_owned(),
        Arc::clone(&lease_active),
        Arc::new(AtomicBool::new(false)),
        Some(Arc::clone(&filters)),
        Some(true),
        generation,
        Arc::new(OwnerDispositionControl::new(Box::new(|_| {}))),
    );
    let admission = resources.begin_admission(generation).unwrap();
    let raw = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let statement = DruidPooledStatement::new_standard(
        raw,
        StatementOptions::default(),
        Arc::clone(&resources),
        policy,
        id,
        1,
        Arc::new(AtomicU64::new(1)),
        Arc::new(AtomicU64::new(1)),
        lease_active,
        Some(filters),
        None,
    );
    resources
        .register(&admission, Arc::clone(&statement.inner))
        .await
        .unwrap();
    drop(admission);
    (statement, connection)
}

/// 完整默认原生委托、平台句柄短路、精确错误与真实SQLite列信息合同。
#[tokio::test]
async fn task6_pooled_result_set_routes_metadata_platform_handle_through_filter() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError, ResultSetFilterContext};
    use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
    use metadata_read_probe::MetadataReadProbe;
    use metadata_short_circuit_filter::MetadataShortCircuitFilter;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let physical_log = Arc::new(Mutex::new(Vec::new()));
    let native_metadata: Arc<dyn ResultSetMetaData> =
        Arc::new(ResultSetMetaDataSnapshot::new(Vec::new()));
    let mut physical_probe = MetadataReadProbe {
        calls: Arc::clone(&physical_log),
        metadata: Arc::clone(&native_metadata),
        fail: Arc::new(AtomicBool::new(false)),
        closed: false,
    };
    let context = ResultSetFilterContext::new();
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let returned = pass_chain
        .standard_result_set_meta_data(&mut physical_probe, &context)
        .unwrap();
    assert_eq!(returned.column_count(), 0);
    assert_eq!(calls(&physical_log), ["physical:meta_data"]);
    assert!(Arc::ptr_eq(&returned, &native_metadata));
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let fail = Arc::new(AtomicBool::new(false));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(MetadataShortCircuitFilter {
        calls: Arc::clone(&call_log),
        fail: Arc::clone(&fail),
        metadata: Arc::clone(&native_metadata),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        82,
        "sqlite-metadata-filter",
        Arc::new(filter_chain),
    )
    .await;
    let mut result_set = statement.execute_query("SELECT 1").await.unwrap();
    assert_eq!(result_set.meta_data().unwrap().column_count(), 0);
    fail.store(true, Ordering::Release);
    assert_eq!(
        result_set.meta_data().map(|_| ()),
        Err(RdbcError::DriverError(
            "filtered metadata failure".to_owned()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    assert_eq!(
        calls(&call_log),
        ["result_set_get_meta_data", "result_set_get_meta_data"]
    );
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        83,
        "sqlite-metadata-pass-through",
        Arc::new(pass_chain),
    )
    .await;
    let mut result_set = statement.execute_query("SELECT 1, 'two'").await.unwrap();
    let metadata = result_set.meta_data().unwrap();
    assert_eq!(metadata.column_count(), 2);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

mod legacy_metadata_observer;
mod ordered_metadata_filter;

/// 完整around与既有before/after共用注册顺序，保留原生/替换Arc身份和精确驱动错误。
#[tokio::test]
async fn task6_metadata_filters_preserve_native_identity_legacy_observers_and_short_circuit() {
    use crate::core::{FilterChain, RdbcError, ResultSetFilterContext};
    use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
    use legacy_metadata_observer::LegacyMetadataObserver;
    use metadata_read_probe::MetadataReadProbe;
    use ordered_metadata_filter::OrderedMetadataFilter;
    use std::sync::Mutex;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let native: Arc<dyn ResultSetMetaData> = Arc::new(ResultSetMetaDataSnapshot::new(Vec::new()));
    let replacement: Arc<dyn ResultSetMetaData> =
        Arc::new(ResultSetMetaDataSnapshot::new(Vec::new()));
    let fail = Arc::new(AtomicBool::new(false));
    let mut raw = MetadataReadProbe {
        calls: Arc::clone(&calls),
        metadata: Arc::clone(&native),
        fail: Arc::clone(&fail),
        closed: false,
    };
    let context = ResultSetFilterContext::new();
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(OrderedMetadataFilter {
        name: "A",
        calls: Arc::clone(&calls),
        replacement: None,
    }));
    filters.add_result_set(Arc::new(LegacyMetadataObserver {
        calls: Arc::clone(&calls),
        replacement: None,
    }));
    filters.add_result_set(Arc::new(OrderedMetadataFilter {
        name: "B",
        calls: Arc::clone(&calls),
        replacement: None,
    }));
    let metadata = filters
        .standard_result_set_meta_data(&mut raw, &context)
        .unwrap();
    assert!(Arc::ptr_eq(&metadata, &native));
    assert_eq!(
        *calls.lock().unwrap(),
        [
            "A:before",
            "legacy:before",
            "B:before",
            "physical:meta_data",
            "B:ok",
            "legacy:after:ok",
            "A:ok"
        ]
    );
    calls.lock().unwrap().clear();
    fail.store(true, Ordering::Release);
    assert_eq!(
        filters
            .standard_result_set_meta_data(&mut raw, &context)
            .map(|_| ())
            .map_err(RdbcError::from),
        Err(RdbcError::DriverError("native metadata failure".to_owned()))
    );
    assert_eq!(
        *calls.lock().unwrap(),
        [
            "A:before",
            "legacy:before",
            "B:before",
            "physical:meta_data",
            "B:error",
            "legacy:after:error",
            "A:error"
        ]
    );
    calls.lock().unwrap().clear();
    let mut short = FilterChain::new();
    for name in ["A", "B", "C"] {
        short.add_result_set(Arc::new(OrderedMetadataFilter {
            name,
            calls: Arc::clone(&calls),
            replacement: (name == "B").then(|| Arc::clone(&replacement)),
        }));
    }
    let metadata = short
        .standard_result_set_meta_data(&mut raw, &context)
        .unwrap();
    assert!(Arc::ptr_eq(&metadata, &replacement));
    assert_eq!(
        *calls.lock().unwrap(),
        ["A:before", "B:before", "B:ok", "A:ok"]
    );
    calls.lock().unwrap().clear();
    let mut after_override = FilterChain::new();
    after_override.add_result_set(Arc::new(LegacyMetadataObserver {
        calls: Arc::clone(&calls),
        replacement: Some(Arc::clone(&replacement)),
    }));
    let metadata = after_override
        .standard_result_set_meta_data(&mut raw, &context)
        .unwrap();
    assert!(Arc::ptr_eq(&metadata, &replacement));
    assert_eq!(
        *calls.lock().unwrap(),
        ["legacy:before", "physical:meta_data", "legacy:after:error"]
    );
}

/// 原生元数据失败记实际语句一次；关闭或失效租约先拒绝且不继续原生读取。
#[tokio::test]
async fn task6_metadata_native_errors_closed_and_expired_lease_guards_are_observable() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::metadata::ResultSetMetaDataSnapshot;
    use metadata_read_probe::MetadataReadProbe;
    use std::sync::Mutex;
    for expire in [false, true] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let fail = Arc::new(AtomicBool::new(false));
        let mut filters = FilterChain::new();
        filters.add_result_set(Arc::new(FilterAdapter::new()));
        let (mut statement, mut connection) =
            task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
        let admission = statement.inner.begin_standard_admission().unwrap();
        let mut rows = DruidPooledResultSet::new_standard(
            Arc::clone(&statement.inner),
            Box::new(MetadataReadProbe {
                calls: Arc::clone(&calls),
                metadata: Arc::new(ResultSetMetaDataSnapshot::new(Vec::new())),
                fail: Arc::clone(&fail),
                closed: false,
            }),
            &admission,
        )
        .await
        .unwrap();
        drop(admission);
        assert_eq!(*calls.lock().unwrap(), ["physical:meta_data"]);
        calls.lock().unwrap().clear();
        assert_eq!(rows.meta_data().unwrap().column_count(), 0);
        fail.store(true, Ordering::Release);
        assert_eq!(
            rows.meta_data().map(|_| ()),
            Err(RdbcError::DriverError("native metadata failure".to_owned()))
        );
        assert_eq!(statement.exception_count(), 1);
        assert_eq!(
            *calls.lock().unwrap(),
            ["physical:meta_data", "physical:meta_data"]
        );
        if expire {
            statement.inner.lease_active.store(false, Ordering::Release);
        } else {
            rows.close().await.unwrap();
        }
        calls.lock().unwrap().clear();
        assert_eq!(
            rows.meta_data().map(|_| ()),
            Err(RdbcError::ConnectionDiscarded)
        );
        assert!(calls.lock().unwrap().is_empty());
        assert_eq!(statement.exception_count(), 1);
        rows.close().await.unwrap();
        statement.close().await.unwrap();
        connection.close().await.unwrap();
    }
}

mod navigation_short_circuit_filter;

/// 固定BASE的完整导航/属性短路及真实SQLite状态机。
#[tokio::test]
async fn task6_pooled_result_set_routes_navigation_properties_and_real_sqlite_state() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::{ResultSetExt, ResultSetNavigation};
    use druid_rdbc::statement::FetchDirection;
    use navigation_short_circuit_filter::NavigationShortCircuitFilter;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(NavigationShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        78,
        "sqlite-navigation-filter",
        Arc::new(filter_chain),
    )
    .await;
    let mut result_set = statement.execute_query("SELECT 1").await.unwrap();

    assert!(!result_set.was_null().unwrap());
    assert!(result_set
        .navigate(ResultSetNavigation::Previous)
        .await
        .unwrap());
    assert!(!result_set.is_before_first().unwrap());
    assert!(result_set.is_after_last().unwrap());
    assert!(result_set.is_first().unwrap());
    assert!(!result_set.is_last().unwrap());
    result_set
        .navigate(ResultSetNavigation::BeforeFirst)
        .await
        .unwrap();
    result_set
        .navigate(ResultSetNavigation::AfterLast)
        .await
        .unwrap();
    assert!(!result_set
        .navigate(ResultSetNavigation::First)
        .await
        .unwrap());
    assert!(!result_set
        .navigate(ResultSetNavigation::Last)
        .await
        .unwrap());
    assert_eq!(result_set.row().unwrap(), 99);
    assert!(result_set
        .navigate(ResultSetNavigation::Absolute(50))
        .await
        .unwrap());
    assert!(result_set
        .navigate(ResultSetNavigation::Relative(-20))
        .await
        .unwrap());
    result_set
        .set_fetch_direction(FetchDirection::DriverValue(2000))
        .unwrap();
    assert_eq!(result_set.fetch_direction().unwrap().rdbc_code(), 2000);
    result_set.set_fetch_size(128).unwrap();
    assert_eq!(result_set.fetch_size().unwrap(), 128);
    assert_eq!(result_set.result_set_type().unwrap().rdbc_code(), 2004);
    assert_eq!(result_set.concurrency().unwrap().rdbc_code(), 2007);
    assert_eq!(result_set.holdability().unwrap().rdbc_code(), 2);
    assert_eq!(
        result_set.cursor_name().unwrap(),
        Some("filtered-cursor".to_string())
    );
    assert!(!result_set.row_updated().unwrap());
    assert!(result_set.row_inserted().unwrap());
    assert!(!result_set.row_deleted().unwrap());
    assert_eq!(result_set.find_column("named").unwrap().get(), 88);
    assert!(!result_set.is_closed().unwrap());
    assert_eq!(calls(&call_log).len(), 26);
    assert_eq!(
        calls(&call_log),
        [
            "result_set_was_null",
            "result_set_previous",
            "result_set_is_before_first",
            "result_set_is_after_last",
            "result_set_is_first",
            "result_set_is_last",
            "result_set_before_first",
            "result_set_after_last",
            "result_set_first",
            "result_set_last",
            "result_set_get_row",
            "result_set_absolute:50",
            "result_set_relative:-20",
            "result_set_set_fetch_direction:2000",
            "result_set_get_fetch_direction",
            "result_set_set_fetch_size:128",
            "result_set_get_fetch_size",
            "result_set_get_type",
            "result_set_get_concurrency",
            "result_set_get_holdability",
            "result_set_get_cursor_name",
            "result_set_row_updated",
            "result_set_row_inserted",
            "result_set_row_deleted",
            "result_set_find_column:named",
            "result_set_is_closed",
        ]
    );

    assert_eq!(
        result_set.find_column("fail"),
        Err(RdbcError::DriverError(
            "filtered findColumn failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    // VALUE_ADD / V5_HOST：真实 Toasty SQLite RowSet 经默认 Filter 执行游标状态机。
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        79,
        "sqlite-navigation-pass-through",
        Arc::new(pass_chain),
    )
    .await;
    let mut result_set = statement
        .execute_query("SELECT 10 AS value UNION ALL SELECT 20")
        .await
        .unwrap();
    assert!(result_set.is_before_first().unwrap());
    assert!(result_set.next().await.unwrap());
    assert_eq!(result_set.row().unwrap(), 1);
    assert!(result_set
        .navigate(ResultSetNavigation::Last)
        .await
        .unwrap());
    assert_eq!(result_set.row().unwrap(), 2);
    assert!(result_set
        .navigate(ResultSetNavigation::Previous)
        .await
        .unwrap());
    assert_eq!(result_set.int(ColumnIndex::new(1).unwrap()).unwrap(), 10);
    result_set
        .navigate(ResultSetNavigation::AfterLast)
        .await
        .unwrap();
    assert!(result_set.is_after_last().unwrap());
    result_set
        .navigate(ResultSetNavigation::BeforeFirst)
        .await
        .unwrap();
    assert!(result_set.is_before_first().unwrap());
    assert!(!result_set.is_closed().unwrap());
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    assert!(result_set.is_closed().unwrap());
}

mod ordered_navigation_property_filter;

/// 防止短路仍移动原生游标、顺序颠倒或一次失败重复记错。
#[tokio::test]
async fn task6_navigation_property_order_short_circuit_and_error_counts() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::{ResultSetExt, ResultSetNavigation};
    use ordered_navigation_property_filter::OrderedNavigationPropertyFilter;
    use std::sync::Mutex;
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    for mode in [0, 1, 2, 3] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut filters = FilterChain::new();
        for name in ["A", "B", "C"] {
            filters.add_result_set(Arc::new(OrderedNavigationPropertyFilter {
                name,
                calls: Arc::clone(&calls),
                mode: if name == "B" { mode } else { 0 },
            }));
        }
        let (mut statement, mut connection) = task6_sqlite_metadata_statement(
            &factory,
            180 + u64::from(mode),
            "navigation-controls",
            Arc::new(filters),
        )
        .await;
        let mut rows = statement
            .execute_query("SELECT 10 AS value UNION ALL SELECT 20")
            .await
            .unwrap();
        let navigation = rows.navigate(ResultSetNavigation::Last).await;
        if mode == 2 {
            assert_eq!(
                navigation,
                Err(RdbcError::DriverError(
                    "ordered navigation failure".to_owned()
                ))
            );
            assert_eq!(
                *calls.lock().unwrap(),
                [
                    "A:nav:Last:before",
                    "B:nav:Last:before",
                    "B:nav:error",
                    "A:nav:error"
                ]
            );
            assert_eq!(statement.exception_count(), 1);
        } else {
            assert_eq!(navigation, Ok(true));
            let expected: &[&str] = if mode == 1 {
                &[
                    "A:nav:Last:before",
                    "B:nav:Last:before",
                    "B:nav:ok",
                    "A:nav:ok",
                ]
            } else {
                &[
                    "A:nav:Last:before",
                    "B:nav:Last:before",
                    "C:nav:Last:before",
                    "C:nav:ok",
                    "B:nav:ok",
                    "A:nav:ok",
                ]
            };
            assert_eq!(*calls.lock().unwrap(), expected);
        }
        calls.lock().unwrap().clear();
        let property = rows.row();
        match mode {
            0 => assert_eq!(property, Ok(2)),
            1 => assert_eq!(property, Ok(77)),
            2 => assert_eq!(
                property,
                Err(RdbcError::DriverError(
                    "ordered property failure".to_owned()
                ))
            ),
            3 => assert_eq!(
                property,
                Err(RdbcError::InvalidArgument(
                    "incompatible result set property filter value".to_owned()
                ))
            ),
            _ => unreachable!(),
        }
        let expected: &[&str] = match mode {
            0 => &[
                "A:property:Row:before",
                "B:property:Row:before",
                "C:property:Row:before",
                "C:property:ok",
                "B:property:ok",
                "A:property:ok",
            ],
            2 => &[
                "A:property:Row:before",
                "B:property:Row:before",
                "B:property:error",
                "A:property:error",
            ],
            _ => &[
                "A:property:Row:before",
                "B:property:Row:before",
                "B:property:ok",
                "A:property:ok",
            ],
        };
        assert_eq!(*calls.lock().unwrap(), expected);
        assert_eq!(
            statement.exception_count(),
            match mode {
                2 => 2,
                3 => 1,
                _ => 0,
            }
        );
        // 读取真实当前值验证下游是否被调用，不从 Filter 的返回值推断驱动状态。
        if mode == 1 || mode == 2 {
            assert!(matches!(
                rows.int(ColumnIndex::new(1).unwrap()),
                Err(RdbcError::InvalidArgument(_))
            ));
            assert!(rows.next().await.unwrap());
            assert_eq!(rows.int(ColumnIndex::new(1).unwrap()).unwrap(), 10);
        } else {
            assert_eq!(rows.int(ColumnIndex::new(1).unwrap()).unwrap(), 20);
        }
        rows.close().await.unwrap();
        calls.lock().unwrap().clear();
        assert_eq!(rows.row(), Err(RdbcError::ConnectionDiscarded));
        assert_eq!(
            rows.navigate(ResultSetNavigation::Last).await,
            Err(RdbcError::ConnectionDiscarded)
        );
        assert!(rows.is_closed().unwrap());
        assert!(calls.lock().unwrap().is_empty());
        statement.close().await.unwrap();
        connection.close().await.unwrap();
    }
}

/// 取消惰性导航不轮询后续Filter、不移动真实游标，且原资源仍可继续使用。
#[tokio::test]
async fn task6_cancelled_navigation_preserves_cursor_and_rejects_expired_lease() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::{ResultSetExt, ResultSetNavigation};
    use ordered_navigation_property_filter::OrderedNavigationPropertyFilter;
    use std::sync::Mutex;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    for name in ["A", "B", "C"] {
        filters.add_result_set(Arc::new(OrderedNavigationPropertyFilter {
            name,
            calls: Arc::clone(&calls),
            mode: if name == "B" { 4 } else { 0 },
        }));
    }
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let (mut statement, mut connection) =
        task6_sqlite_metadata_statement(&factory, 190, "cancelled-navigation", Arc::new(filters))
            .await;
    let mut rows = statement
        .execute_query("SELECT 10 AS value UNION ALL SELECT 20")
        .await
        .unwrap();
    let mut future = rows.navigate(ResultSetNavigation::Last);
    assert!(matches!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(future.as_mut().poll(cx))).await,
        std::task::Poll::Pending
    ));
    drop(future);
    assert_eq!(
        *calls.lock().unwrap(),
        ["A:nav:Last:before", "B:nav:Last:before"]
    );
    assert_eq!(statement.exception_count(), 0);
    assert!(rows.next().await.unwrap());
    assert_eq!(rows.int(ColumnIndex::new(1).unwrap()).unwrap(), 10);
    calls.lock().unwrap().clear();
    statement.inner.lease_active.store(false, Ordering::Release);
    assert_eq!(rows.row(), Err(RdbcError::ConnectionDiscarded));
    assert_eq!(
        rows.navigate(ResultSetNavigation::Last).await,
        Err(RdbcError::ConnectionDiscarded)
    );
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(statement.exception_count(), 0);
    statement.inner.lease_active.store(true, Ordering::Release);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

mod object_update_short_circuit_filter;
mod object_update_vendor_probe;

/// 固定BASE的四种对象更新短路、具体对象身份与真实SQLite拒绝合同。
#[tokio::test]
async fn task6_pooled_result_set_routes_object_updates_and_preserves_sqlite_capability_errors() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetUpdate;
    use druid_rdbc::types::{RdbcObject, RdbcOpaqueObject};
    use object_update_short_circuit_filter::ObjectUpdateShortCircuitFilter;
    use object_update_vendor_probe::FilterVendorObjectProbe;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    let vendor_object = RdbcOpaqueObject::new(Arc::new(FilterVendorObjectProbe { id: 99 }));
    filter_chain.add_result_set(Arc::new(ObjectUpdateShortCircuitFilter {
        calls: Arc::clone(&call_log),
        expected_custom: vendor_object.clone(),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        89,
        "sqlite-object-update-short-circuit",
        Arc::new(filter_chain),
    )
    .await;
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();

    // SOURCE_PARITY / V2_MIRRORED：四重载独立短路，不改写NULL或负scaleOrLength。
    result_set
        .update_value(
            ColumnIndex::new(15).unwrap(),
            ResultSetUpdate::Object(RdbcObject::Custom(vendor_object)),
        )
        .unwrap();
    result_set
        .update_value_by_label("object_value", ResultSetUpdate::Object(Value::Null.into()))
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(16).unwrap(),
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: RdbcObject::Integer(7),
                scale_or_length: -3,
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "scaled_object",
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: RdbcObject::Bytes(vec![9]),
                scale_or_length: 99,
            },
        )
        .unwrap();
    assert_eq!(
        calls(&call_log),
        [
            "result_set_update_object:15:custom:example.FilterVendorObject:99",
            "result_set_update_object_by_label:object_value:Scalar(Null)",
            "result_set_update_object_with_scale_or_length:16:Integer(7):-3",
            "result_set_update_object_by_label_with_scale_or_length:scaled_object:Bytes([9]):99",
        ]
    );

    // RUST_OBLIGATION / V1_RUST_LOCAL：原创建Statement只计一次精确Filter错误。
    assert_eq!(
        result_set.update_value_by_label("fail", ResultSetUpdate::Object(Value::Null.into())),
        Err(RdbcError::DriverError(
            "filtered updateObject failure".to_owned()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    // VALUE_ADD / V5_HOST：默认链抵达真实只读SQLite，保留index/label能力拒绝。
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        90,
        "sqlite-object-update-pass-through",
        Arc::new(pass_chain),
    )
    .await;
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();
    assert_eq!(
        result_set.update_value(
            ColumnIndex::new(15).unwrap(),
            ResultSetUpdate::Object(RdbcObject::String("index".to_owned()))
        ),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_update_value"
        })
    );
    assert_eq!(
        result_set
            .update_value_by_label("object_value", ResultSetUpdate::Object(Value::Null.into())),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_update_value_by_label"
        })
    );
    assert_eq!(
        result_set.update_value(
            ColumnIndex::new(16).unwrap(),
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: RdbcObject::Integer(7),
                scale_or_length: -3,
            }
        ),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_update_value"
        })
    );
    assert_eq!(
        result_set.update_value_by_label(
            "scaled_object",
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: RdbcObject::Bytes(vec![9]),
                scale_or_length: 99,
            }
        ),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_update_value_by_label"
        })
    );
    assert_eq!(statement.exception_count(), 4);
    assert!(!result_set.is_closed().unwrap());
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

mod resource_short_circuit_filter;

/// 完整固定BASE资源getter、共享流游标、精确错误与真实SQLite读取合同。
#[tokio::test]
async fn task6_pooled_result_set_routes_resource_getters_and_real_sqlite_streams() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::resource::{RdbcInputStream, RdbcReader};
    use druid_rdbc::result_set::ResultSetExt;
    use resource_short_circuit_filter::ResourceShortCircuitFilter;
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

mod ordered_resource_filter;
mod resource_read_probe;

/// 十三资源家族的原生重载身份、ABC顺序、B短路/错误及共享游标。
#[tokio::test]
async fn task6_resource_filters_preserve_native_overloads_order_and_stream_identity() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::resource::{RdbcInputStream, RdbcReader};
    use druid_rdbc::result_set::{ResultSetExt, ResultSetRead, ResultSetStreamKind};
    use ordered_resource_filter::OrderedResourceFilter;
    use resource_read_probe::ResourceReadProbe;
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
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::resource::{RdbcInputStream, RdbcReader};
    use druid_rdbc::result_set::{ResultSetRead, ResultSetStreamKind};
    use resource_short_circuit_filter::ResourceShortCircuitFilter;
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

mod resource_update_short_circuit_filter;

/// 恢复固定BASE十四种资源更新重载、精确错误和实际SQLite能力合同。
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn task6_pooled_result_set_routes_resource_updates_and_preserves_sqlite_capability_errors() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::RdbcRowId;
    use resource_update_short_circuit_filter::ResourceUpdateShortCircuitFilter;
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

mod row_mutation_short_circuit_filter;

/// 保留固定BASE七行操作短路、精确刷新错误与真实SQLite能力拒绝。
#[tokio::test]
async fn task6_pooled_result_set_routes_row_mutations_and_preserves_sqlite_capability_errors() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use row_mutation_short_circuit_filter::RowMutationShortCircuitFilter;
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
    use crate::core::{FilterChain, RdbcError};
    use row_mutation_short_circuit_filter::RowMutationShortCircuitFilter;
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

mod scalar_update_short_circuit_filter;

/// 恢复完整原标量更新短路、精确错误与真实SQLite能力合同。
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn task6_pooled_result_set_routes_scalar_updates_and_preserves_sqlite_capability_errors() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use scalar_update_short_circuit_filter::ScalarUpdateShortCircuitFilter;
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

mod stream_update_short_circuit_filter;

/// 恢复原22个流更新重载、共享读位置与真实SQLite能力错误。
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn task6_pooled_result_set_routes_stream_updates_and_preserves_sqlite_capability_errors() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use std::sync::Mutex;
    use stream_update_short_circuit_filter::StreamUpdateShortCircuitFilter;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(StreamUpdateShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        95,
        "sqlite-stream-update-short-circuit",
        Arc::new(filter_chain),
    )
    .await;
    let mut result_set = statement
        .execute_query("SELECT X'010203' AS value")
        .await
        .unwrap();
    let ascii = RdbcInputStream::from_bytes([1, 2, 3]);
    let binary = RdbcInputStream::from_bytes([4, 5, 6]);
    let character = RdbcReader::from_string("甲乙");
    let n_character = RdbcReader::from_string("丙丁");

    result_set
        .update_value(
            ColumnIndex::new(1).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&ascii).cloned(),
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "ascii",
            ResultSetUpdate::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(2).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&ascii).cloned(),
                length: RdbcStreamLength::Int(-7),
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "ascii_int",
            ResultSetUpdate::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Int(i32::MAX),
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(3).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&ascii).cloned(),
                length: RdbcStreamLength::Long(-11),
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "ascii_long",
            ResultSetUpdate::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Long(i64::MAX),
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(4).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&binary).cloned(),
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "binary",
            ResultSetUpdate::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(5).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&binary).cloned(),
                length: RdbcStreamLength::Int(i32::MIN),
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "binary_int",
            ResultSetUpdate::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Int(13),
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(6).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&binary).cloned(),
                length: RdbcStreamLength::Long(-17),
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "binary_long",
            ResultSetUpdate::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Long(19),
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(7).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&character).cloned(),
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "character",
            ResultSetUpdate::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(8).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&character).cloned(),
                length: RdbcCharacterLength::Int(-23),
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "character_int",
            ResultSetUpdate::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Int(29),
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(9).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&character).cloned(),
                length: RdbcCharacterLength::Long(-31),
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "character_long",
            ResultSetUpdate::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(37),
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(10).unwrap(),
            ResultSetUpdate::NCharacterStream {
                reader: Some(&n_character).cloned(),
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "n_character",
            ResultSetUpdate::NCharacterStream {
                reader: None,
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(11).unwrap(),
            ResultSetUpdate::NCharacterStream {
                reader: Some(&n_character).cloned(),
                length: RdbcCharacterLength::Long(-41),
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "n_character_long",
            ResultSetUpdate::NCharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(43),
            },
        )
        .unwrap();

    assert_eq!(
        calls(&call_log),
        [
            "result_set_update_ascii_stream:1:some",
            "result_set_update_ascii_stream_by_label:ascii:none",
            "result_set_update_ascii_stream_with_int_length:2:some:-7",
            "result_set_update_ascii_stream_by_label_with_int_length:ascii_int:none:2147483647",
            "result_set_update_ascii_stream_with_length:3:some:-11",
            "result_set_update_ascii_stream_by_label_with_length:ascii_long:none:9223372036854775807",
            "result_set_update_binary_stream:4:some",
            "result_set_update_binary_stream_by_label:binary:none",
            "result_set_update_binary_stream_with_int_length:5:some:-2147483648",
            "result_set_update_binary_stream_by_label_with_int_length:binary_int:none:13",
            "result_set_update_binary_stream_with_length:6:some:-17",
            "result_set_update_binary_stream_by_label_with_length:binary_long:none:19",
            "result_set_update_character_stream:7:some",
            "result_set_update_character_stream_by_label:character:none",
            "result_set_update_character_stream_with_int_length:8:some:-23",
            "result_set_update_character_stream_by_label_with_int_length:character_int:none:29",
            "result_set_update_character_stream_with_length:9:some:-31",
            "result_set_update_character_stream_by_label_with_length:character_long:none:37",
            "result_set_update_n_character_stream:10:some",
            "result_set_update_n_character_stream_by_label:n_character:none",
            "result_set_update_n_character_stream_with_length:11:some:-41",
            "result_set_update_n_character_stream_by_label_with_length:n_character_long:none:43",
        ]
    );
    assert_eq!(ascii.read_to_end().unwrap(), [2, 3]);
    assert_eq!(binary.read_to_end().unwrap(), [5, 6]);
    assert_eq!(character.read_to_string().unwrap(), "乙");
    assert_eq!(n_character.read_to_string().unwrap(), "丁");

    // RUST_OBLIGATION / V1_RUST_LOCAL：Filter 错误只在原 Statement 分类一次，
    // 短路后不触达真实只读 RowSet。
    assert_eq!(
        result_set.update_value_by_label(
            "fail",
            ResultSetUpdate::NCharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(-47)
            }
        ),
        Err(RdbcError::DriverError(
            "filtered stream update failure".to_string()
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
        96,
        "sqlite-stream-update-pass-through",
        Arc::new(pass_chain),
    )
    .await;
    let mut result_set = statement
        .execute_query("SELECT X'010203' AS value")
        .await
        .unwrap();

    macro_rules! assert_stream_unsupported {
        ($expression:expr, $operation:literal) => {
            assert_eq!(
                $expression,
                Err(RdbcError::UnsupportedOperation {
                    operation: $operation,
                })
            );
        };
    }

    // VALUE_ADD / V5_HOST：真实 Toasty SQLite RowSet 对 22 个精确 Filter
    // 入口保持 generic update capability error，且每次失败只计数一次。
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(1).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&ascii).cloned(),
                length: RdbcStreamLength::Unspecified
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "ascii",
            ResultSetUpdate::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Unspecified
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(2).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&ascii).cloned(),
                length: RdbcStreamLength::Int(-7)
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "ascii_int",
            ResultSetUpdate::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Int(i32::MAX)
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(3).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&ascii).cloned(),
                length: RdbcStreamLength::Long(-11)
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "ascii_long",
            ResultSetUpdate::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Long(i64::MAX)
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(4).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&binary).cloned(),
                length: RdbcStreamLength::Unspecified
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "binary",
            ResultSetUpdate::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Unspecified
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(5).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&binary).cloned(),
                length: RdbcStreamLength::Int(i32::MIN)
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "binary_int",
            ResultSetUpdate::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Int(13)
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(6).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&binary).cloned(),
                length: RdbcStreamLength::Long(-17)
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "binary_long",
            ResultSetUpdate::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Long(19)
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(7).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&character).cloned(),
                length: RdbcCharacterLength::Unspecified
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "character",
            ResultSetUpdate::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Unspecified
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(8).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&character).cloned(),
                length: RdbcCharacterLength::Int(-23)
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "character_int",
            ResultSetUpdate::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Int(29)
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(9).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&character).cloned(),
                length: RdbcCharacterLength::Long(-31)
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "character_long",
            ResultSetUpdate::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(37)
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(10).unwrap(),
            ResultSetUpdate::NCharacterStream {
                reader: Some(&n_character).cloned(),
                length: RdbcCharacterLength::Unspecified
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "n_character",
            ResultSetUpdate::NCharacterStream {
                reader: None,
                length: RdbcCharacterLength::Unspecified
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(11).unwrap(),
            ResultSetUpdate::NCharacterStream {
                reader: Some(&n_character).cloned(),
                length: RdbcCharacterLength::Long(-41)
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "n_character_long",
            ResultSetUpdate::NCharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(43)
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_eq!(statement.exception_count(), 22);
    assert!(!result_set.is_closed().unwrap());
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

mod native_result_set_close_observer;

/// 恢复完整原真实SQLite读取、统计与创建语句级联关闭合同。
#[tokio::test]
async fn task6_real_sqlite_result_sets_flow_through_stat_filter_and_statement_cascade() {
    use crate::core::FilterChain;
    use crate::stats::{StatFilter, StatsCollector};
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::{RdbcCalendar, RdbcCalendarArgument, RdbcTargetType};
    use native_result_set_close_observer::NativeResultSetCloseObserver;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;
    let collector = Arc::new(StatsCollector::new("sqlite-stat", Duration::from_secs(1)));
    let stat_filter = Arc::new(StatFilter::new(Arc::clone(&collector)));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_filter(stat_filter);

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let (mut first_statement, mut connection) =
        task6_sqlite_metadata_statement(&factory, 72, "sqlite-stat", Arc::new(filter_chain)).await;
    let mut first = first_statement
        .execute_query(
            "SELECT 1 AS n, '12.340' AS decimal_value, '2025-01-02' AS date_value, \
             '03:04:05' AS time_value, '2025-01-02 03:04:05' AS timestamp_value \
             UNION ALL SELECT 2, '56.780', '2025-02-03', '06:07:08', \
             '2025-02-03 06:07:08' UNION ALL SELECT 3, '90.120', '2025-03-04', \
             '09:10:11', '2025-03-04 09:10:11'",
        )
        .await
        .unwrap();
    assert_eq!(collector.result_set_stat().opening_count(), 1);
    assert!(first.next().await.unwrap());
    assert_eq!(first.int(ColumnIndex::new(1).unwrap()).unwrap(), 1);
    assert_eq!(
        first.object(ColumnIndex::new(1).unwrap()).unwrap(),
        RdbcObject::Scalar(Value::Int(1))
    );
    assert_eq!(
        first
            .object_as(ColumnIndex::new(1).unwrap(), &RdbcTargetType::Long)
            .unwrap(),
        RdbcObject::Long(1)
    );
    assert_eq!(
        first.big_decimal(ColumnIndex::new(2).unwrap()).unwrap(),
        Some(BigDecimal::from_str("12.340").unwrap())
    );
    assert_eq!(
        first
            .date(
                ColumnIndex::new(3).unwrap(),
                &RdbcCalendarArgument::Unspecified
            )
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2025, 1, 2).unwrap())
    );
    assert_eq!(
        first
            .time(
                ColumnIndex::new(4).unwrap(),
                &RdbcCalendarArgument::Specified(None)
            )
            .unwrap(),
        Some(NaiveTime::from_hms_opt(3, 4, 5).unwrap())
    );
    assert_eq!(
        first
            .timestamp(
                ColumnIndex::new(5).unwrap(),
                &RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("UTC").unwrap()))
            )
            .unwrap(),
        Some(
            NaiveDate::from_ymd_opt(2025, 1, 2)
                .unwrap()
                .and_hms_opt(3, 4, 5)
                .unwrap()
        )
    );
    assert!(first.next().await.unwrap());
    first.close().await.unwrap();
    assert_eq!(collector.result_set_stat().opening_count(), 0);
    assert_eq!(collector.result_set_stat().fetch_row_count(), 2);
    assert_eq!(collector.result_set_stat().close_count(), 1);

    first_statement.close().await.unwrap();
    assert_eq!(collector.result_set_stat().close_count(), 1);

    // 第二语句来自同一真实连接，复用同一策略、资源登记和ID种子。
    let admission = first_statement.inner.begin_standard_admission().unwrap();
    let raw = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut second_statement = DruidPooledStatement::new_standard(
        raw,
        StatementOptions::default(),
        Arc::clone(&first_statement.inner.resources),
        Arc::clone(&first_statement.inner.policy),
        72,
        2,
        Arc::clone(&first_statement.inner.result_set_id_seed),
        Arc::clone(&first_statement.inner.metadata_id_seed),
        Arc::clone(&first_statement.inner.lease_active),
        first_statement.inner.filter_chain.clone(),
        None,
    );
    first_statement
        .inner
        .resources
        .register(&admission, Arc::clone(&second_statement.inner))
        .await
        .unwrap();
    drop(admission);
    let mut second = second_statement
        .execute_query("SELECT 4 UNION ALL SELECT 5")
        .await
        .unwrap();
    // 包围原查询登记的同一原生游标，只观察实际关闭，不能合成关闭状态。
    let native_closed = Arc::new(AtomicBool::new(false));
    let native_close_count = Arc::new(AtomicUsize::new(0));
    let resource = second_statement.current_result_set_for_diagnostics();
    {
        let mut slot = resource.result_set.lock().await;
        let original = slot.take().expect("同一真实SQLite查询游标");
        let identity = std::ptr::from_ref::<dyn ResultSet>(original.as_ref()).cast::<()>() as usize;
        *slot = Some(Box::new(NativeResultSetCloseObserver::new(
            original,
            Arc::clone(&native_closed),
            Arc::clone(&native_close_count),
            identity,
        )));
    }
    assert!(second.next().await.unwrap());
    second_statement.close().await.unwrap();

    let stat = collector.result_set_stat();
    assert_eq!(stat.open_count(), 2);
    assert_eq!(stat.opening_count(), 0);
    assert_eq!(stat.opening_max(), 1);
    assert_eq!(stat.fetch_row_count(), 3);
    assert_eq!(stat.close_count(), 2);
    assert!(stat.last_open_time_millis().is_some());
    assert!(second.is_closed().unwrap());
    assert!(native_closed.load(Ordering::Acquire));
    assert_eq!(native_close_count.load(Ordering::Acquire), 1);
    second_statement.close().await.unwrap();
    second.close().await.unwrap();
    assert_eq!(native_close_count.load(Ordering::Acquire), 1);
    assert_eq!(stat.close_count(), 2);
    connection.close().await.unwrap();
}

mod filter_order_result_set_probe;
mod int_wrapping_filter;
mod ordering_result_set_filter;

/// 恢复完整原Filter开放、next、close错误退出、短路与标量叠加合同。
#[tokio::test]
async fn task6_result_set_filter_chain_preserves_java_order_short_circuit_and_error_unwind() {
    use crate::core::{DruidError, FilterChain, RdbcError, ResultSetFilterContext};
    use filter_order_result_set_probe::FilterOrderResultSetProbe;
    use int_wrapping_filter::IntWrappingFilter;
    use ordering_result_set_filter::OrderingResultSetFilter;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut physical = FilterOrderResultSetProbe::new(Arc::clone(&call_log), true);
    let context = ResultSetFilterContext::new();
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::new(OrderingResultSetFilter {
        label: "first",
        calls: Arc::clone(&call_log),
        short_circuit_next: false,
    }));
    chain.add_result_set(Arc::new(OrderingResultSetFilter {
        label: "second",
        calls: Arc::clone(&call_log),
        short_circuit_next: false,
    }));

    assert_eq!(chain.result_set_count(), 2);
    chain
        .standard_result_set_open_after_with_proxy(
            &mut crate::core::StandardResultSetOpenContext::new(
                &context,
                Arc::new(druid_rdbc::metadata::ResultSetMetaDataSnapshot::new(
                    Vec::new(),
                )),
            ),
        )
        .unwrap();
    assert!(chain
        .standard_result_set_next(&mut physical, &context)
        .await
        .unwrap());
    assert_eq!(
        calls(&call_log),
        vec![
            "second:open",
            "first:open",
            "first:next-before",
            "second:next-before",
            "physical:next",
            "second:next-after",
            "first:next-after",
        ]
    );

    assert_eq!(
        chain
            .standard_result_set_close(&mut physical, &context)
            .await,
        Err(DruidError::Rdbc(RdbcError::DriverError(
            "close failed".to_string()
        )))
    );
    assert_eq!(
        &calls(&call_log)[7..],
        [
            "first:close-before",
            "second:close-before",
            "physical:close"
        ]
    );

    let short_log = Arc::new(Mutex::new(Vec::new()));
    let mut short_physical = FilterOrderResultSetProbe::new(Arc::clone(&short_log), false);
    let short_context = ResultSetFilterContext::new();
    let mut short_chain = FilterChain::new();
    short_chain.add_result_set(Arc::new(OrderingResultSetFilter {
        label: "short",
        calls: Arc::clone(&short_log),
        short_circuit_next: true,
    }));
    assert!(!short_chain
        .standard_result_set_next(&mut short_physical, &short_context)
        .await
        .unwrap());
    assert_eq!(calls(&short_log), ["short:next-before", "short:next-short"]);
    assert_eq!(short_physical.next_count.load(Ordering::Acquire), 0);

    let scalar_log = Arc::new(Mutex::new(Vec::new()));
    let mut scalar_physical = FilterOrderResultSetProbe::new(Arc::clone(&scalar_log), false);
    let scalar_context = ResultSetFilterContext::new();
    let mut scalar_chain = FilterChain::new();
    scalar_chain.add_result_set(Arc::new(IntWrappingFilter {
        label: "first",
        delta: 1,
        calls: Arc::clone(&scalar_log),
    }));
    scalar_chain.add_result_set(Arc::new(IntWrappingFilter {
        label: "second",
        delta: 10,
        calls: Arc::clone(&scalar_log),
    }));
    assert_eq!(
        scalar_chain
            .standard_result_set_typed_value(
                &mut scalar_physical,
                &scalar_context,
                Some(ColumnIndex::new(5).unwrap()),
                &druid_rdbc::result_set::ResultSetRead::Int,
                None
            )
            .unwrap(),
        Value::Int(43)
    );
    assert_eq!(
        calls(&scalar_log),
        [
            "first:int-before:5",
            "second:int-before:5",
            "physical:int:5",
            "second:int-after:32",
            "first:int-after:42",
        ]
    );
}

mod filter_defaults_chain_harness;
mod filter_defaults_result_set_probe;

/// 恢复完整原默认重载委托、上下文与调用日志合同。
#[tokio::test]
async fn task6_result_set_filter_defaults_delegate_and_context_defaults_are_observable() {
    use crate::core::{DruidError, RdbcError, ResultSetFilterContext};
    use druid_rdbc::types::{RdbcCalendar, RdbcCalendarArgument, RdbcTargetType, RdbcTypeMap};
    use filter_defaults_chain_harness::FilterDefaultsChainHarness;
    use filter_defaults_result_set_probe::FilterDefaultsResultSetProbe;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut physical = FilterDefaultsResultSetProbe::new(call_log, false);
    let context = ResultSetFilterContext::default();
    let mut chain = FilterDefaultsChainHarness::new();
    chain.add_result_set(Arc::new(crate::core::FilterAdapter::new()));

    assert_eq!(context.elapsed(), None);
    assert_eq!(context.fetch_row_count(), 0);
    assert_eq!(context.close_count(), 0);
    chain.result_set_open_after(&context).unwrap();
    assert!(chain
        .result_set_next(&mut physical, &context)
        .await
        .unwrap());
    assert_eq!(
        chain
            .result_set_get_string(&mut physical, &context, 1)
            .unwrap(),
        Some("index-string".to_string())
    );
    assert_eq!(
        chain
            .result_set_get_string_by_label(&mut physical, &context, "string")
            .unwrap(),
        Some("label-string".to_string())
    );
    assert!(chain
        .result_set_get_boolean(&mut physical, &context, 2)
        .unwrap());
    assert!(!chain
        .result_set_get_boolean_by_label(&mut physical, &context, "boolean")
        .unwrap());
    assert_eq!(
        chain
            .result_set_get_byte(&mut physical, &context, 3)
            .unwrap(),
        8
    );
    assert_eq!(
        chain
            .result_set_get_byte_by_label(&mut physical, &context, "byte")
            .unwrap(),
        9
    );
    assert_eq!(
        chain
            .result_set_get_short(&mut physical, &context, 4)
            .unwrap(),
        16
    );
    assert_eq!(
        chain
            .result_set_get_short_by_label(&mut physical, &context, "short")
            .unwrap(),
        17
    );
    assert_eq!(
        chain
            .result_set_get_int(&mut physical, &context, 5)
            .unwrap(),
        32
    );
    assert_eq!(
        chain
            .result_set_get_int_by_label(&mut physical, &context, "int")
            .unwrap(),
        33
    );
    assert_eq!(
        chain
            .result_set_get_long(&mut physical, &context, 6)
            .unwrap(),
        64
    );
    assert_eq!(
        chain
            .result_set_get_long_by_label(&mut physical, &context, "long")
            .unwrap(),
        65
    );
    assert_eq!(
        chain
            .result_set_get_float(&mut physical, &context, 7)
            .unwrap(),
        1.25
    );
    assert_eq!(
        chain
            .result_set_get_float_by_label(&mut physical, &context, "float")
            .unwrap(),
        2.25
    );
    assert_eq!(
        chain
            .result_set_get_double(&mut physical, &context, 8)
            .unwrap(),
        3.5
    );
    assert_eq!(
        chain
            .result_set_get_double_by_label(&mut physical, &context, "double")
            .unwrap(),
        4.5
    );
    assert_eq!(
        chain
            .result_set_get_bytes(&mut physical, &context, 9)
            .unwrap(),
        Some(vec![1, 2])
    );
    assert_eq!(
        chain
            .result_set_get_bytes_by_label(&mut physical, &context, "bytes")
            .unwrap(),
        Some(vec![3, 4])
    );
    assert_eq!(
        chain
            .result_set_get_big_decimal(&mut physical, &context, 10)
            .unwrap(),
        Some(BigDecimal::from_str("12.340").unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_big_decimal_by_label(&mut physical, &context, "decimal")
            .unwrap(),
        Some(BigDecimal::from_str("56.780").unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_big_decimal_with_scale(&mut physical, &context, 11, 2)
            .unwrap(),
        Some(BigDecimal::from_str("12.340").unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_big_decimal_by_label_with_scale(
                &mut physical,
                &context,
                "decimal_scale",
                3,
            )
            .unwrap(),
        Some(BigDecimal::from_str("56.780").unwrap())
    );
    let null_calendar = RdbcCalendarArgument::specified(None);
    let shanghai_calendar =
        RdbcCalendarArgument::specified(Some(RdbcCalendar::new("Asia/Shanghai").unwrap()));
    assert_eq!(
        chain
            .result_set_get_date(&mut physical, &context, 12)
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2025, 1, 2).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_date_by_label(&mut physical, &context, "date")
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2025, 2, 3).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_date_with_calendar(&mut physical, &context, 13, &null_calendar)
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2025, 1, 2).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_date_by_label_with_calendar(
                &mut physical,
                &context,
                "date_calendar",
                &shanghai_calendar,
            )
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2025, 2, 3).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_time(&mut physical, &context, 14)
            .unwrap(),
        Some(NaiveTime::from_hms_opt(3, 4, 5).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_time_by_label(&mut physical, &context, "time")
            .unwrap(),
        Some(NaiveTime::from_hms_opt(6, 7, 8).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_time_with_calendar(&mut physical, &context, 15, &null_calendar)
            .unwrap(),
        Some(NaiveTime::from_hms_opt(3, 4, 5).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_time_by_label_with_calendar(
                &mut physical,
                &context,
                "time_calendar",
                &shanghai_calendar,
            )
            .unwrap(),
        Some(NaiveTime::from_hms_opt(6, 7, 8).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_timestamp(&mut physical, &context, 16)
            .unwrap(),
        Some(
            NaiveDate::from_ymd_opt(2025, 1, 2)
                .unwrap()
                .and_hms_opt(3, 4, 5)
                .unwrap()
        )
    );
    assert_eq!(
        chain
            .result_set_get_timestamp_by_label(&mut physical, &context, "timestamp")
            .unwrap(),
        Some(
            NaiveDate::from_ymd_opt(2025, 2, 3)
                .unwrap()
                .and_hms_opt(6, 7, 8)
                .unwrap()
        )
    );
    assert_eq!(
        chain
            .result_set_get_timestamp_with_calendar(&mut physical, &context, 17, &null_calendar,)
            .unwrap(),
        Some(
            NaiveDate::from_ymd_opt(2025, 1, 2)
                .unwrap()
                .and_hms_opt(3, 4, 5)
                .unwrap()
        )
    );
    assert_eq!(
        chain
            .result_set_get_timestamp_by_label_with_calendar(
                &mut physical,
                &context,
                "timestamp_calendar",
                &shanghai_calendar,
            )
            .unwrap(),
        Some(
            NaiveDate::from_ymd_opt(2025, 2, 3)
                .unwrap()
                .and_hms_opt(6, 7, 8)
                .unwrap()
        )
    );
    let mut type_map = RdbcTypeMap::new();
    type_map.insert(
        "example.address",
        RdbcTargetType::Custom("example.Address".to_string()),
    );
    assert_eq!(
        chain
            .result_set_get_object(&mut physical, &context, 18)
            .unwrap(),
        Value::Int(18)
    );
    assert_eq!(
        chain
            .result_set_get_object_by_label(&mut physical, &context, "object")
            .unwrap(),
        Value::String("object".to_string())
    );
    assert_eq!(
        chain
            .result_set_get_object_with_type_map(&mut physical, &context, 19, None)
            .unwrap(),
        RdbcObject::String("index-map:19:null".to_string())
    );
    assert_eq!(
        chain
            .result_set_get_object_by_label_with_type_map(
                &mut physical,
                &context,
                "mapped",
                Some(&type_map),
            )
            .unwrap(),
        RdbcObject::String(
            "label-map:mapped:example.address=Custom(\"example.Address\")".to_string()
        )
    );
    assert_eq!(
        chain
            .result_set_get_object_typed(
                &mut physical,
                &context,
                20,
                &RdbcTargetType::Custom("example.Typed".to_string()),
            )
            .unwrap(),
        RdbcObject::String("index-typed:20:Custom(\"example.Typed\")".to_string())
    );
    assert_eq!(
        chain
            .result_set_get_object_typed_by_label(
                &mut physical,
                &context,
                "typed",
                &RdbcTargetType::String,
            )
            .unwrap(),
        RdbcObject::String("label-typed:typed:String".to_string())
    );
    assert_eq!(
        chain.result_set_warnings(&mut physical, &context),
        Err(DruidError::Rdbc(RdbcError::UnsupportedOperation {
            operation: "result_set_warnings"
        }))
    );
    assert_eq!(
        chain.result_set_clear_warnings(&mut physical, &context),
        Err(DruidError::Rdbc(RdbcError::UnsupportedOperation {
            operation: "result_set_clear_warnings"
        }))
    );
    chain
        .result_set_close(&mut physical, &context)
        .await
        .unwrap();
    context.increment_close_count();
    assert_eq!(context.close_count(), 1);
    assert_eq!(
        calls(&physical.calls)[1..19],
        [
            "physical:string:1",
            "physical:string_by_label:string",
            "physical:boolean:2",
            "physical:boolean_by_label:boolean",
            "physical:byte:3",
            "physical:byte_by_label:byte",
            "physical:short:4",
            "physical:short_by_label:short",
            "physical:int:5",
            "physical:int_by_label:int",
            "physical:long:6",
            "physical:long_by_label:long",
            "physical:float:7",
            "physical:float_by_label:float",
            "physical:double:8",
            "physical:double_by_label:double",
            "physical:bytes:9",
            "physical:bytes_by_label:bytes",
        ]
    );
    assert_eq!(
        calls(&physical.calls)[19..35],
        [
            "physical:big_decimal:10:None",
            "physical:big_decimal_by_label:decimal:None",
            "physical:big_decimal:11:Some(2)",
            "physical:big_decimal_by_label:decimal_scale:Some(3)",
            "physical:date:12:unspecified",
            "physical:date_by_label:date:unspecified",
            "physical:date:13:specified:null",
            "physical:date_by_label:date_calendar:specified:Asia/Shanghai",
            "physical:time:14:unspecified",
            "physical:time_by_label:time:unspecified",
            "physical:time:15:specified:null",
            "physical:time_by_label:time_calendar:specified:Asia/Shanghai",
            "physical:timestamp:16:unspecified",
            "physical:timestamp_by_label:timestamp:unspecified",
            "physical:timestamp:17:specified:null",
            "physical:timestamp_by_label:timestamp_calendar:specified:Asia/Shanghai",
        ]
    );
    assert_eq!(
        calls(&physical.calls)[35..41],
        [
            "physical:value:18",
            "physical:value_by_label:object",
            "physical:object_with_type_map:19:null",
            "physical:object_by_label_with_type_map:mapped:example.address=Custom(\"example.Address\")",
            "physical:object_as:20:Custom(\"example.Typed\")",
            "physical:object_by_label_as:typed:String",
        ]
    );
}

mod lob_stream_defaults_result_set_probe;

/// 保留 Blob、Clob、NClob 的十二个默认重载和共享流不预读、不关闭合同。
#[test]
fn task6_result_set_lob_stream_update_filter_defaults_preserve_all_twelve_overloads() {
    use crate::core::{FilterAdapter, FilterChain, ResultSetFilterContext};
    use druid_rdbc::result_set::ResultSetUpdate;
    use druid_rdbc::types::{RdbcCharacterLength, RdbcInputStream, RdbcReader, RdbcStreamLength};
    use lob_stream_defaults_result_set_probe::LobStreamDefaultsResultSetProbe;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let physical_log = Arc::new(Mutex::new(Vec::new()));
    let mut physical = LobStreamDefaultsResultSetProbe::new(Arc::clone(&physical_log));
    let context = ResultSetFilterContext::new();
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let blob = RdbcInputStream::from_bytes([1, 2, 3]);
    let clob = RdbcReader::from_string("甲乙");
    let n_clob = RdbcReader::from_string("丙丁");
    // 原十二重载逐一进入实际标准过滤器链，保留 null、原下标/标签和长度。
    filter_chain
        .standard_result_set_update(
            &mut physical,
            &context,
            ColumnIndex::new(1).unwrap(),
            ResultSetUpdate::BlobStream {
                stream: Some(blob.clone()),
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    filter_chain
        .standard_result_set_update_by_label(
            &mut physical,
            &context,
            "blob",
            ResultSetUpdate::BlobStream {
                stream: None,
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    filter_chain
        .standard_result_set_update(
            &mut physical,
            &context,
            ColumnIndex::new(2).unwrap(),
            ResultSetUpdate::BlobStream {
                stream: Some(blob.clone()),
                length: RdbcStreamLength::Long(-7),
            },
        )
        .unwrap();
    filter_chain
        .standard_result_set_update_by_label(
            &mut physical,
            &context,
            "blob_long",
            ResultSetUpdate::BlobStream {
                stream: None,
                length: RdbcStreamLength::Long(i64::MAX),
            },
        )
        .unwrap();
    filter_chain
        .standard_result_set_update(
            &mut physical,
            &context,
            ColumnIndex::new(3).unwrap(),
            ResultSetUpdate::ClobReader {
                reader: Some(clob.clone()),
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    filter_chain
        .standard_result_set_update_by_label(
            &mut physical,
            &context,
            "clob",
            ResultSetUpdate::ClobReader {
                reader: None,
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    filter_chain
        .standard_result_set_update(
            &mut physical,
            &context,
            ColumnIndex::new(4).unwrap(),
            ResultSetUpdate::ClobReader {
                reader: Some(clob.clone()),
                length: RdbcCharacterLength::Long(-11),
            },
        )
        .unwrap();
    filter_chain
        .standard_result_set_update_by_label(
            &mut physical,
            &context,
            "clob_long",
            ResultSetUpdate::ClobReader {
                reader: None,
                length: RdbcCharacterLength::Long(13),
            },
        )
        .unwrap();
    filter_chain
        .standard_result_set_update(
            &mut physical,
            &context,
            ColumnIndex::new(5).unwrap(),
            ResultSetUpdate::NClobReader {
                reader: Some(n_clob.clone()),
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    filter_chain
        .standard_result_set_update_by_label(
            &mut physical,
            &context,
            "n_clob",
            ResultSetUpdate::NClobReader {
                reader: None,
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    filter_chain
        .standard_result_set_update(
            &mut physical,
            &context,
            ColumnIndex::new(6).unwrap(),
            ResultSetUpdate::NClobReader {
                reader: Some(n_clob.clone()),
                length: RdbcCharacterLength::Long(-17),
            },
        )
        .unwrap();
    filter_chain
        .standard_result_set_update_by_label(
            &mut physical,
            &context,
            "n_clob_long",
            ResultSetUpdate::NClobReader {
                reader: None,
                length: RdbcCharacterLength::Long(19),
            },
        )
        .unwrap();
    assert_eq!(
        calls(&physical_log),
        [
            "physical:update_blob_stream:1:true:Unspecified",
            "physical:update_blob_stream_by_label:blob:false:Unspecified",
            "physical:update_blob_stream:2:true:Long(-7)",
            "physical:update_blob_stream_by_label:blob_long:false:Long(9223372036854775807)",
            "physical:update_clob_reader:3:true:Unspecified",
            "physical:update_clob_reader_by_label:clob:false:Unspecified",
            "physical:update_clob_reader:4:true:Long(-11)",
            "physical:update_clob_reader_by_label:clob_long:false:Long(13)",
            "physical:update_n_clob_reader:5:true:Unspecified",
            "physical:update_n_clob_reader_by_label:n_clob:false:Unspecified",
            "physical:update_n_clob_reader:6:true:Long(-17)",
            "physical:update_n_clob_reader_by_label:n_clob_long:false:Long(19)",
        ]
    );

    // RUST_OBLIGATION / V1_RUST_LOCAL：默认 Filter 只克隆共享句柄，不预读、
    // 不关闭，也不校正负长度或超大 long。
    assert_eq!(blob.read_to_end().unwrap(), [1, 2, 3]);
    assert_eq!(clob.read_to_string().unwrap(), "甲乙");
    assert_eq!(n_clob.read_to_string().unwrap(), "丙丁");
}

mod navigation_defaults_chain_harness;
mod navigation_defaults_result_set_probe;

/// 完整保留原默认导航、属性值和二十五条原生调用观察。
#[tokio::test]
async fn task6_result_set_navigation_filter_defaults_delegate_every_physical_method() {
    use crate::core::{FilterAdapter, ResultSetFilterContext};
    use navigation_defaults_chain_harness::NavigationDefaultsChainHarness;
    use navigation_defaults_result_set_probe::NavigationDefaultsResultSetProbe;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut physical = NavigationDefaultsResultSetProbe::new(Arc::clone(&call_log));
    let context = ResultSetFilterContext::new();
    let mut chain = NavigationDefaultsChainHarness::new();
    chain.add_result_set(Arc::new(FilterAdapter::new()));

    assert!(chain.result_set_was_null(&mut physical, &context).unwrap());
    assert!(!chain
        .result_set_previous(&mut physical, &context)
        .await
        .unwrap());
    assert!(chain
        .result_set_is_before_first(&mut physical, &context)
        .unwrap());
    assert!(!chain
        .result_set_is_after_last(&mut physical, &context)
        .unwrap());
    assert!(!chain.result_set_is_first(&mut physical, &context).unwrap());
    assert!(chain.result_set_is_last(&mut physical, &context).unwrap());
    chain
        .result_set_before_first(&mut physical, &context)
        .await
        .unwrap();
    chain
        .result_set_after_last(&mut physical, &context)
        .await
        .unwrap();
    assert!(chain
        .result_set_first(&mut physical, &context)
        .await
        .unwrap());
    assert!(chain
        .result_set_last(&mut physical, &context)
        .await
        .unwrap());
    assert_eq!(
        chain.result_set_get_row(&mut physical, &context).unwrap(),
        7
    );
    assert!(chain
        .result_set_absolute(&mut physical, &context, 5)
        .await
        .unwrap());
    assert!(chain
        .result_set_relative(&mut physical, &context, -2)
        .await
        .unwrap());
    chain
        .result_set_set_fetch_direction(&mut physical, &context, 1000)
        .unwrap();
    assert_eq!(
        chain
            .result_set_get_fetch_direction(&mut physical, &context)
            .unwrap(),
        1000
    );
    chain
        .result_set_set_fetch_size(&mut physical, &context, 64)
        .unwrap();
    assert_eq!(
        chain
            .result_set_get_fetch_size(&mut physical, &context)
            .unwrap(),
        64
    );
    assert_eq!(
        chain.result_set_get_type(&mut physical, &context).unwrap(),
        1004
    );
    assert_eq!(
        chain
            .result_set_get_concurrency(&mut physical, &context)
            .unwrap(),
        1007
    );
    assert_eq!(
        chain
            .result_set_get_holdability(&mut physical, &context)
            .unwrap(),
        1
    );
    assert_eq!(
        chain
            .result_set_get_cursor_name(&mut physical, &context)
            .unwrap(),
        Some("cursor".to_string())
    );
    assert!(chain
        .result_set_row_updated(&mut physical, &context)
        .unwrap());
    assert!(!chain
        .result_set_row_inserted(&mut physical, &context)
        .unwrap());
    assert!(chain
        .result_set_row_deleted(&mut physical, &context)
        .unwrap());
    assert_eq!(
        chain
            .result_set_find_column(&mut physical, &context, "named")
            .unwrap(),
        23
    );
    assert!(!chain.result_set_is_closed(&mut physical, &context).unwrap());

    let observed = calls(&call_log);
    assert_eq!(observed.len(), 25);
    for expected in [
        "physical:was_null",
        "physical:previous",
        "physical:is_before_first",
        "physical:is_after_last",
        "physical:is_first",
        "physical:is_last",
        "physical:before_first",
        "physical:after_last",
        "physical:first",
        "physical:last",
        "physical:row",
        "physical:absolute:5",
        "physical:relative:-2",
        "physical:set_fetch_direction:1000",
        "physical:fetch_direction",
        "physical:set_fetch_size:64",
        "physical:fetch_size",
        "physical:result_set_type",
        "physical:concurrency",
        "physical:holdability",
        "physical:cursor_name",
        "physical:row_updated",
        "physical:row_inserted",
        "physical:row_deleted",
        "physical:find_column:named",
    ] {
        assert!(observed.iter().any(|call| call == expected), "{expected}");
    }
}

/// 默认导航不依赖 row 能力；池化跟踪保留 row 错误且短路不读取驱动。
#[tokio::test]
async fn task6_navigation_defaults_do_not_read_row_and_pooled_tracking_preserves_errors() {
    use crate::core::{DruidError, FilterAdapter, FilterChain, RdbcError, ResultSetFilterContext};
    use druid_rdbc::result_set::ResultSetNavigation;
    use navigation_defaults_result_set_probe::NavigationDefaultsResultSetProbe;
    use ordered_navigation_property_filter::OrderedNavigationPropertyFilter;
    use std::sync::Mutex;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut raw = NavigationDefaultsResultSetProbe::new(Arc::clone(&calls)).with_row_error();
    let context = ResultSetFilterContext::new();
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::new(FilterAdapter::new()));
    assert_eq!(
        chain
            .standard_result_set_navigation(
                &mut raw,
                &context,
                ResultSetNavigation::Previous,
                false
            )
            .await
            .unwrap(),
        (false, None)
    );
    assert_eq!(*calls.lock().unwrap(), ["physical:previous"]);
    calls.lock().unwrap().clear();
    assert_eq!(
        chain
            .standard_result_set_navigation(
                &mut raw,
                &context,
                ResultSetNavigation::Previous,
                true,
            )
            .await,
        Err(DruidError::from(RdbcError::UnsupportedOperation {
            operation: "result_set_row"
        }))
    );
    assert_eq!(
        *calls.lock().unwrap(),
        ["physical:previous", "physical:row"]
    );
    calls.lock().unwrap().clear();
    let filter_calls = Arc::new(Mutex::new(Vec::new()));
    let mut short = FilterChain::new();
    short.add_result_set(Arc::new(OrderedNavigationPropertyFilter {
        name: "short",
        calls: Arc::clone(&filter_calls),
        mode: 1,
    }));
    assert_eq!(
        short
            .standard_result_set_navigation(&mut raw, &context, ResultSetNavigation::Last, true)
            .await
            .unwrap(),
        (true, None)
    );
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(
        *filter_calls.lock().unwrap(),
        ["short:nav:Last:before", "short:nav:ok"]
    );
}

mod resource_defaults_chain_harness;
mod resource_defaults_result_set_probe;

/// 完整保留原十三家族双重载与二十六条默认委托事件。
#[test]
fn task6_result_set_resource_filter_defaults_preserve_all_physical_overload_identities() {
    use crate::core::{FilterAdapter, ResultSetFilterContext};
    use resource_defaults_chain_harness::ResourceDefaultsChainHarness;
    use resource_defaults_result_set_probe::ResourceDefaultsResultSetProbe;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut physical = ResourceDefaultsResultSetProbe::new(Arc::clone(&call_log));
    let context = ResultSetFilterContext::new();
    let mut chain = ResourceDefaultsChainHarness::new();
    chain.add_result_set(Arc::new(FilterAdapter::new()));

    macro_rules! assert_resource_pair {
        ($index:ident, $label:ident, $position:expr, $name:literal) => {
            assert!(chain
                .$index(&mut physical, &context, $position)
                .unwrap()
                .is_none());
            assert!(chain
                .$label(&mut physical, &context, $name)
                .unwrap()
                .is_none());
        };
    }

    assert_resource_pair!(result_set_get_ref, result_set_get_ref_by_label, 1, "ref");
    assert_resource_pair!(result_set_get_blob, result_set_get_blob_by_label, 2, "blob");
    assert_resource_pair!(result_set_get_clob, result_set_get_clob_by_label, 3, "clob");
    assert_resource_pair!(
        result_set_get_array,
        result_set_get_array_by_label,
        4,
        "array"
    );
    assert_resource_pair!(result_set_get_url, result_set_get_url_by_label, 5, "url");
    assert_resource_pair!(
        result_set_get_row_id,
        result_set_get_row_id_by_label,
        6,
        "row_id"
    );
    assert_resource_pair!(
        result_set_get_n_clob,
        result_set_get_n_clob_by_label,
        7,
        "n_clob"
    );
    assert_resource_pair!(
        result_set_get_sql_xml,
        result_set_get_sql_xml_by_label,
        8,
        "sql_xml"
    );
    assert_resource_pair!(
        result_set_get_ascii_stream,
        result_set_get_ascii_stream_by_label,
        9,
        "ascii"
    );
    assert_resource_pair!(
        result_set_get_unicode_stream,
        result_set_get_unicode_stream_by_label,
        10,
        "unicode"
    );
    assert_resource_pair!(
        result_set_get_binary_stream,
        result_set_get_binary_stream_by_label,
        11,
        "binary"
    );
    assert_resource_pair!(
        result_set_get_character_stream,
        result_set_get_character_stream_by_label,
        12,
        "character"
    );
    assert_resource_pair!(
        result_set_get_n_character_stream,
        result_set_get_n_character_stream_by_label,
        13,
        "n_character"
    );

    assert_eq!(
        calls(&call_log),
        [
            "physical:reference:1",
            "physical:reference_by_label:ref",
            "physical:blob:2",
            "physical:blob_by_label:blob",
            "physical:clob:3",
            "physical:clob_by_label:clob",
            "physical:array:4",
            "physical:array_by_label:array",
            "physical:url:5",
            "physical:url_by_label:url",
            "physical:row_id:6",
            "physical:row_id_by_label:row_id",
            "physical:n_clob:7",
            "physical:n_clob_by_label:n_clob",
            "physical:sql_xml:8",
            "physical:sql_xml_by_label:sql_xml",
            "physical:ascii_stream:9",
            "physical:ascii_stream_by_label:ascii",
            "physical:unicode_stream:10",
            "physical:unicode_stream_by_label:unicode",
            "physical:binary_stream:11",
            "physical:binary_stream_by_label:binary",
            "physical:character_stream:12",
            "physical:character_stream_by_label:character",
            "physical:n_character_stream:13",
            "physical:n_character_stream_by_label:n_character",
        ]
    );
}

mod resource_update_defaults_chain_harness;
mod resource_update_defaults_result_set_probe;

/// 原七资源双入口、NULL 与 RowId 值以及十四完整事件保持不变。
#[test]
fn task6_result_set_resource_update_filter_defaults_preserve_all_fourteen_overloads() {
    use crate::core::{FilterAdapter, ResultSetFilterContext};
    use druid_rdbc::types::RdbcRowId;
    use resource_update_defaults_chain_harness::ResourceUpdateDefaultsChainHarness;
    use resource_update_defaults_result_set_probe::ResourceUpdateDefaultsResultSetProbe;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let physical_log = Arc::new(Mutex::new(Vec::new()));
    let mut physical = ResourceUpdateDefaultsResultSetProbe::new(Arc::clone(&physical_log));
    let context = ResultSetFilterContext::new();
    let mut filter_chain = ResourceUpdateDefaultsChainHarness::new();
    filter_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let row_id = RdbcRowId::new([1, 2, 3]);

    // SOURCE_PARITY / V2_MIRRORED：七个资源类型的 index/label 重载分别进入
    // Filter，Java null 与 RowId 值身份在默认末端保持不变。
    filter_chain
        .result_set_update_reference(&mut physical, &context, 1, None)
        .unwrap();
    filter_chain
        .result_set_update_reference_by_label(&mut physical, &context, "ref", None)
        .unwrap();
    filter_chain
        .result_set_update_blob(&mut physical, &context, 2, None)
        .unwrap();
    filter_chain
        .result_set_update_blob_by_label(&mut physical, &context, "blob", None)
        .unwrap();
    filter_chain
        .result_set_update_clob(&mut physical, &context, 3, None)
        .unwrap();
    filter_chain
        .result_set_update_clob_by_label(&mut physical, &context, "clob", None)
        .unwrap();
    filter_chain
        .result_set_update_array(&mut physical, &context, 4, None)
        .unwrap();
    filter_chain
        .result_set_update_array_by_label(&mut physical, &context, "array", None)
        .unwrap();
    filter_chain
        .result_set_update_row_id(&mut physical, &context, 5, Some(row_id.clone()))
        .unwrap();
    filter_chain
        .result_set_update_row_id_by_label(&mut physical, &context, "row_id", Some(row_id))
        .unwrap();
    filter_chain
        .result_set_update_n_clob(&mut physical, &context, 6, None)
        .unwrap();
    filter_chain
        .result_set_update_n_clob_by_label(&mut physical, &context, "n_clob", None)
        .unwrap();
    filter_chain
        .result_set_update_sql_xml(&mut physical, &context, 7, None)
        .unwrap();
    filter_chain
        .result_set_update_sql_xml_by_label(&mut physical, &context, "sql_xml", None)
        .unwrap();

    assert_eq!(
        calls(&physical_log),
        [
            "physical:update_reference:1:None",
            "physical:update_reference_by_label:ref:None",
            "physical:update_blob:2:None",
            "physical:update_blob_by_label:blob:None",
            "physical:update_clob:3:None",
            "physical:update_clob_by_label:clob:None",
            "physical:update_array:4:None",
            "physical:update_array_by_label:array:None",
            "physical:update_row_id:5:Some(RdbcRowId { bytes: [1, 2, 3] })",
            "physical:update_row_id_by_label:row_id:Some(RdbcRowId { bytes: [1, 2, 3] })",
            "physical:update_n_clob:6:None",
            "physical:update_n_clob_by_label:n_clob:None",
            "physical:update_sql_xml:7:None",
            "physical:update_sql_xml_by_label:sql_xml:None",
        ]
    );
}

mod row_mutation_defaults_chain_harness;
mod row_mutation_defaults_result_set_probe;

/// 原七个行修改默认委托与完整有序原生日志保持不变。
#[test]
fn task6_result_set_row_mutation_filter_defaults_delegate_every_physical_method() {
    use crate::core::{FilterAdapter, ResultSetFilterContext};
    use row_mutation_defaults_chain_harness::RowMutationDefaultsChainHarness;
    use row_mutation_defaults_result_set_probe::RowMutationDefaultsResultSetProbe;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut physical = RowMutationDefaultsResultSetProbe::new(Arc::clone(&call_log));
    let context = ResultSetFilterContext::new();
    let mut chain = RowMutationDefaultsChainHarness::new();
    chain.add_result_set(Arc::new(FilterAdapter::new()));

    // SOURCE_PARITY / V2_MIRRORED：七个 Java 无参方法均从 position 0
    // 进入默认 Filter，并在末端调用同名 raw ResultSet 方法。
    chain
        .result_set_insert_row(&mut physical, &context)
        .unwrap();
    chain
        .result_set_update_row(&mut physical, &context)
        .unwrap();
    chain
        .result_set_delete_row(&mut physical, &context)
        .unwrap();
    chain
        .result_set_refresh_row(&mut physical, &context)
        .unwrap();
    chain
        .result_set_cancel_row_updates(&mut physical, &context)
        .unwrap();
    chain
        .result_set_move_to_insert_row(&mut physical, &context)
        .unwrap();
    chain
        .result_set_move_to_current_row(&mut physical, &context)
        .unwrap();

    assert_eq!(
        calls(&call_log),
        [
            "physical:insert_row",
            "physical:update_row",
            "physical:delete_row",
            "physical:refresh_row",
            "physical:cancel_row_updates",
            "physical:move_to_insert_row",
            "physical:move_to_current_row",
        ]
    );
}

mod stream_update_defaults_chain_harness;
mod stream_update_defaults_result_set_probe;

/// 原二十二重载、长度边界与四个完整资源内容断言保持不变。
#[test]
fn task6_result_set_stream_update_filter_defaults_preserve_all_twenty_two_overloads() {
    use crate::core::{FilterAdapter, ResultSetFilterContext};
    use druid_rdbc::types::{RdbcInputStream, RdbcReader};
    use std::sync::Mutex;
    use stream_update_defaults_chain_harness::StreamUpdateDefaultsChainHarness;
    use stream_update_defaults_result_set_probe::StreamUpdateDefaultsResultSetProbe;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let physical_log = Arc::new(Mutex::new(Vec::new()));
    let mut physical = StreamUpdateDefaultsResultSetProbe::new(Arc::clone(&physical_log));
    let context = ResultSetFilterContext::new();
    let mut filter_chain = StreamUpdateDefaultsChainHarness::new();
    filter_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let ascii = RdbcInputStream::from_bytes([1, 2, 3]);
    let binary = RdbcInputStream::from_bytes([4, 5, 6]);
    let character = RdbcReader::from_string("甲乙");
    let n_character = RdbcReader::from_string("丙丁");

    // SOURCE_PARITY / V2_MIRRORED：Java Filter/FilterChainImpl 的 ASCII、
    // Binary、Character × index/label × unspecified/int/long，以及
    // NCharacter × index/label × unspecified/long，共 22 个重载逐一穿透。
    filter_chain
        .result_set_update_ascii_stream(&mut physical, &context, 1, Some(ascii.clone()))
        .unwrap();
    filter_chain
        .result_set_update_ascii_stream_by_label(&mut physical, &context, "ascii", None)
        .unwrap();
    filter_chain
        .result_set_update_ascii_stream_with_int_length(
            &mut physical,
            &context,
            2,
            Some(ascii.clone()),
            -7,
        )
        .unwrap();
    filter_chain
        .result_set_update_ascii_stream_by_label_with_int_length(
            &mut physical,
            &context,
            "ascii_int",
            None,
            i32::MAX,
        )
        .unwrap();
    filter_chain
        .result_set_update_ascii_stream_with_length(
            &mut physical,
            &context,
            3,
            Some(ascii.clone()),
            -11,
        )
        .unwrap();
    filter_chain
        .result_set_update_ascii_stream_by_label_with_length(
            &mut physical,
            &context,
            "ascii_long",
            None,
            i64::MAX,
        )
        .unwrap();
    filter_chain
        .result_set_update_binary_stream(&mut physical, &context, 4, Some(binary.clone()))
        .unwrap();
    filter_chain
        .result_set_update_binary_stream_by_label(&mut physical, &context, "binary", None)
        .unwrap();
    filter_chain
        .result_set_update_binary_stream_with_int_length(
            &mut physical,
            &context,
            5,
            Some(binary.clone()),
            i32::MIN,
        )
        .unwrap();
    filter_chain
        .result_set_update_binary_stream_by_label_with_int_length(
            &mut physical,
            &context,
            "binary_int",
            None,
            13,
        )
        .unwrap();
    filter_chain
        .result_set_update_binary_stream_with_length(
            &mut physical,
            &context,
            6,
            Some(binary.clone()),
            -17,
        )
        .unwrap();
    filter_chain
        .result_set_update_binary_stream_by_label_with_length(
            &mut physical,
            &context,
            "binary_long",
            None,
            19,
        )
        .unwrap();
    filter_chain
        .result_set_update_character_stream(&mut physical, &context, 7, Some(character.clone()))
        .unwrap();
    filter_chain
        .result_set_update_character_stream_by_label(&mut physical, &context, "character", None)
        .unwrap();
    filter_chain
        .result_set_update_character_stream_with_int_length(
            &mut physical,
            &context,
            8,
            Some(character.clone()),
            -23,
        )
        .unwrap();
    filter_chain
        .result_set_update_character_stream_by_label_with_int_length(
            &mut physical,
            &context,
            "character_int",
            None,
            29,
        )
        .unwrap();
    filter_chain
        .result_set_update_character_stream_with_length(
            &mut physical,
            &context,
            9,
            Some(character.clone()),
            -31,
        )
        .unwrap();
    filter_chain
        .result_set_update_character_stream_by_label_with_length(
            &mut physical,
            &context,
            "character_long",
            None,
            37,
        )
        .unwrap();
    filter_chain
        .result_set_update_n_character_stream(
            &mut physical,
            &context,
            10,
            Some(n_character.clone()),
        )
        .unwrap();
    filter_chain
        .result_set_update_n_character_stream_by_label(&mut physical, &context, "n_character", None)
        .unwrap();
    filter_chain
        .result_set_update_n_character_stream_with_length(
            &mut physical,
            &context,
            11,
            Some(n_character.clone()),
            -41,
        )
        .unwrap();
    filter_chain
        .result_set_update_n_character_stream_by_label_with_length(
            &mut physical,
            &context,
            "n_character_long",
            None,
            43,
        )
        .unwrap();

    assert_eq!(
        calls(&physical_log),
        [
            "physical:update_value:1:AsciiStream:true:Unspecified",
            "physical:update_value_by_label:ascii:AsciiStream:false:Unspecified",
            "physical:update_value:2:AsciiStream:true:Int(-7)",
            "physical:update_value_by_label:ascii_int:AsciiStream:false:Int(2147483647)",
            "physical:update_value:3:AsciiStream:true:Long(-11)",
            "physical:update_value_by_label:ascii_long:AsciiStream:false:Long(9223372036854775807)",
            "physical:update_value:4:BinaryStream:true:Unspecified",
            "physical:update_value_by_label:binary:BinaryStream:false:Unspecified",
            "physical:update_value:5:BinaryStream:true:Int(-2147483648)",
            "physical:update_value_by_label:binary_int:BinaryStream:false:Int(13)",
            "physical:update_value:6:BinaryStream:true:Long(-17)",
            "physical:update_value_by_label:binary_long:BinaryStream:false:Long(19)",
            "physical:update_value:7:CharacterStream:true:Unspecified",
            "physical:update_value_by_label:character:CharacterStream:false:Unspecified",
            "physical:update_value:8:CharacterStream:true:Int(-23)",
            "physical:update_value_by_label:character_int:CharacterStream:false:Int(29)",
            "physical:update_value:9:CharacterStream:true:Long(-31)",
            "physical:update_value_by_label:character_long:CharacterStream:false:Long(37)",
            "physical:update_value:10:NCharacterStream:true:Unspecified",
            "physical:update_value_by_label:n_character:NCharacterStream:false:Unspecified",
            "physical:update_value:11:NCharacterStream:true:Long(-41)",
            "physical:update_value_by_label:n_character_long:NCharacterStream:false:Long(43)",
        ]
    );

    // RUST_OBLIGATION / V1_RUST_LOCAL：默认 Filter 只克隆共享句柄，不预读、
    // 不关闭，也不校正负长度、i32 边界或超大 long。
    assert_eq!(ascii.read_to_end().unwrap(), [1, 2, 3]);
    assert_eq!(binary.read_to_end().unwrap(), [4, 5, 6]);
    assert_eq!(character.read_to_string().unwrap(), "甲乙");
    assert_eq!(n_character.read_to_string().unwrap(), "丙丁");
}

mod stat_close_error_chain_harness;
mod stat_close_error_result_set_probe;

/// 保留原全部断言，并在下游关闭入口验证统计提交先于精确错误。
#[tokio::test]
async fn task6_stat_filter_commits_close_statistics_before_downstream_close_error() {
    use crate::core::{DruidError, RdbcError, ResultSetFilterContext};
    use crate::stats::{StatFilter, StatsCollector};
    use stat_close_error_chain_harness::StatCloseErrorChainHarness;
    use stat_close_error_result_set_probe::StatCloseErrorResultSetProbe;
    use std::sync::Mutex;
    use std::time::Duration;

    let collector = Arc::new(StatsCollector::new("probe", Duration::from_secs(1)));
    let stat_filter = Arc::new(StatFilter::new(Arc::clone(&collector)));
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let observations = Arc::new(Mutex::new(Vec::new()));
    let mut physical = StatCloseErrorResultSetProbe::new(
        Arc::clone(&call_log),
        Arc::clone(&collector),
        Arc::clone(&observations),
    );
    let context = ResultSetFilterContext::new();
    let mut chain = StatCloseErrorChainHarness::new();
    assert!(std::ptr::eq(
        stat_filter.result_set_stat(),
        collector.result_set_stat()
    ));
    chain.add_result_set(stat_filter);

    chain.result_set_open_after(&context).unwrap();
    context.record_fetch_row_count(3);
    assert_eq!(collector.result_set_stat().opening_count(), 1);
    assert_eq!(
        chain.result_set_close(&mut physical, &context).await,
        Err(DruidError::Rdbc(RdbcError::DriverError(
            "close failed".to_string()
        )))
    );

    let stat = collector.result_set_stat();
    assert_eq!(stat.open_count(), 1);
    assert_eq!(stat.opening_count(), 0);
    assert_eq!(stat.fetch_row_count(), 3);
    assert_eq!(stat.close_count(), 1);
    assert_eq!(context.close_count(), 0);
    assert_eq!(*call_log.lock().unwrap(), ["physical:close"]);
    assert_eq!(*observations.lock().unwrap(), ["1:0:3:1"]);
    assert!(physical.is_closed().unwrap());
}
