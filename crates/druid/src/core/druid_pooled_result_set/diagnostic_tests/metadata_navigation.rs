//! 结果集诊断：metadata_navigation 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

/// 完整默认原生委托、平台句柄短路、精确错误与真实SQLite列信息合同。
#[tokio::test]
async fn task6_pooled_result_set_routes_metadata_platform_handle_through_filter() {
    use super::metadata_read_probe::MetadataReadProbe;
    use super::metadata_short_circuit_filter::MetadataShortCircuitFilter;
    use crate::core::{FilterAdapter, FilterChain, RdbcError, ResultSetFilterContext};
    use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
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

/// 完整around与既有before/after共用注册顺序，保留原生/替换Arc身份和精确驱动错误。
#[tokio::test]
async fn task6_metadata_filters_preserve_native_identity_legacy_observers_and_short_circuit() {
    use super::legacy_metadata_observer::LegacyMetadataObserver;
    use super::metadata_read_probe::MetadataReadProbe;
    use super::ordered_metadata_filter::OrderedMetadataFilter;
    use crate::core::{FilterChain, RdbcError, ResultSetFilterContext};
    use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
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
    use super::metadata_read_probe::MetadataReadProbe;
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::metadata::ResultSetMetaDataSnapshot;
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

/// 固定BASE的完整导航/属性短路及真实SQLite状态机。
#[tokio::test]
async fn task6_pooled_result_set_routes_navigation_properties_and_real_sqlite_state() {
    use super::navigation_short_circuit_filter::NavigationShortCircuitFilter;
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::{ResultSetExt, ResultSetNavigation};
    use druid_rdbc::statement::FetchDirection;
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

/// 防止短路仍移动原生游标、顺序颠倒或一次失败重复记错。
#[tokio::test]
async fn task6_navigation_property_order_short_circuit_and_error_counts() {
    use super::ordered_navigation_property_filter::OrderedNavigationPropertyFilter;
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::{ResultSetExt, ResultSetNavigation};
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
    use super::ordered_navigation_property_filter::OrderedNavigationPropertyFilter;
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::{ResultSetExt, ResultSetNavigation};
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

/// 固定BASE的四种对象更新短路、具体对象身份与真实SQLite拒绝合同。
#[tokio::test]
async fn task6_pooled_result_set_routes_object_updates_and_preserves_sqlite_capability_errors() {
    use super::object_update_short_circuit_filter::ObjectUpdateShortCircuitFilter;
    use super::object_update_vendor_probe::FilterVendorObjectProbe;
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetUpdate;
    use druid_rdbc::types::{RdbcObject, RdbcOpaqueObject};
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
