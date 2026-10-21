//! 结果集诊断：filter_defaults_stream_b 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::*;

/// 完整保留原十三家族双重载与二十六条默认委托事件。
#[test]
fn task6_result_set_resource_filter_defaults_preserve_all_physical_overload_identities() {
    use super::resource_defaults_chain_harness::ResourceDefaultsChainHarness;
    use super::resource_defaults_result_set_probe::ResourceDefaultsResultSetProbe;
    use crate::core::{FilterAdapter, ResultSetFilterContext};
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

/// 原七资源双入口、NULL 与 RowId 值以及十四完整事件保持不变。
#[test]
fn task6_result_set_resource_update_filter_defaults_preserve_all_fourteen_overloads() {
    use super::resource_update_defaults_chain_harness::ResourceUpdateDefaultsChainHarness;
    use super::resource_update_defaults_result_set_probe::ResourceUpdateDefaultsResultSetProbe;
    use crate::core::{FilterAdapter, ResultSetFilterContext};
    use druid_rdbc::types::RdbcRowId;
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

/// 原七个行修改默认委托与完整有序原生日志保持不变。
#[test]
fn task6_result_set_row_mutation_filter_defaults_delegate_every_physical_method() {
    use super::row_mutation_defaults_chain_harness::RowMutationDefaultsChainHarness;
    use super::row_mutation_defaults_result_set_probe::RowMutationDefaultsResultSetProbe;
    use crate::core::{FilterAdapter, ResultSetFilterContext};
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

/// 原二十二重载、长度边界与四个完整资源内容断言保持不变。
#[test]
fn task6_result_set_stream_update_filter_defaults_preserve_all_twenty_two_overloads() {
    use super::stream_update_defaults_chain_harness::StreamUpdateDefaultsChainHarness;
    use super::stream_update_defaults_result_set_probe::StreamUpdateDefaultsResultSetProbe;
    use crate::core::{FilterAdapter, ResultSetFilterContext};
    use druid_rdbc::types::{RdbcInputStream, RdbcReader};
    use std::sync::Mutex;
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

/// 保留原全部断言，并在下游关闭入口验证统计提交先于精确错误。
#[tokio::test]
async fn task6_stat_filter_commits_close_statistics_before_downstream_close_error() {
    use super::stat_close_error_chain_harness::StatCloseErrorChainHarness;
    use super::stat_close_error_result_set_probe::StatCloseErrorResultSetProbe;
    use crate::core::{DruidError, RdbcError, ResultSetFilterContext};
    use crate::stats::{StatFilter, StatsCollector};
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
