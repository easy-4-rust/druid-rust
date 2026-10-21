//! 结果集诊断：filter_defaults_scalar 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

/// 恢复完整原Filter开放、next、close错误退出、短路与标量叠加合同。
#[tokio::test]
async fn task6_result_set_filter_chain_preserves_java_order_short_circuit_and_error_unwind() {
    use super::filter_order_result_set_probe::FilterOrderResultSetProbe;
    use super::int_wrapping_filter::IntWrappingFilter;
    use super::ordering_result_set_filter::OrderingResultSetFilter;
    use crate::core::{DruidError, FilterChain, RdbcError, ResultSetFilterContext};
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

/// 恢复完整原默认重载委托、上下文与调用日志合同。
#[tokio::test]
async fn task6_result_set_filter_defaults_delegate_and_context_defaults_are_observable() {
    use super::filter_defaults_chain_harness::FilterDefaultsChainHarness;
    use super::filter_defaults_result_set_probe::FilterDefaultsResultSetProbe;
    use crate::core::{DruidError, RdbcError, ResultSetFilterContext};
    use druid_rdbc::types::{RdbcCalendar, RdbcCalendarArgument, RdbcTargetType, RdbcTypeMap};
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
