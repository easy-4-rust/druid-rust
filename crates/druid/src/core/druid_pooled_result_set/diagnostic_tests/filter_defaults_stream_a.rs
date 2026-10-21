//! 结果集诊断：filter_defaults_stream_a 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

/// 保留 Blob、Clob、NClob 的十二个默认重载和共享流不预读、不关闭合同。
#[test]
fn task6_result_set_lob_stream_update_filter_defaults_preserve_all_twelve_overloads() {
    use super::lob_stream_defaults_result_set_probe::LobStreamDefaultsResultSetProbe;
    use crate::core::{FilterAdapter, FilterChain, ResultSetFilterContext};
    use druid_rdbc::result_set::ResultSetUpdate;
    use druid_rdbc::types::{RdbcCharacterLength, RdbcInputStream, RdbcReader, RdbcStreamLength};
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

/// 完整保留原默认导航、属性值和二十五条原生调用观察。
#[tokio::test]
async fn task6_result_set_navigation_filter_defaults_delegate_every_physical_method() {
    use super::navigation_defaults_chain_harness::NavigationDefaultsChainHarness;
    use super::navigation_defaults_result_set_probe::NavigationDefaultsResultSetProbe;
    use crate::core::{FilterAdapter, ResultSetFilterContext};
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
    use super::navigation_defaults_result_set_probe::NavigationDefaultsResultSetProbe;
    use super::ordered_navigation_property_filter::OrderedNavigationPropertyFilter;
    use crate::core::{DruidError, FilterAdapter, FilterChain, RdbcError, ResultSetFilterContext};
    use druid_rdbc::result_set::ResultSetNavigation;
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
