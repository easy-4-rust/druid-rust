// @generated receiver-exact group: ResultSetExt
#[test]
fn task6_method_behavior_physical_result_set_reference() {
    let expected = "druid_rdbc::result_set::ResultSetExt::reference";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::reference(&target, ColumnIndex::new(1).unwrap());
    assert_trace(actual, "druid_rdbc::result_set::ResultSetExt::reference");
}

#[test]
fn task6_method_behavior_physical_result_set_reference_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::reference_by_label";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::reference_by_label(&target, "task6");
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::reference_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_row_id() {
    let expected = "druid_rdbc::result_set::ResultSetExt::row_id";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::row_id(&target, ColumnIndex::new(1).unwrap());
    assert_trace(actual, "druid_rdbc::result_set::ResultSetExt::row_id");
}

#[test]
fn task6_method_behavior_physical_result_set_row_id_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::row_id_by_label";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::row_id_by_label(&target, "task6");
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::row_id_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_short() {
    let expected = "druid_rdbc::result_set::ResultSetExt::short";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::short(&target, ColumnIndex::new(1).unwrap());
    assert_trace(actual, "druid_rdbc::result_set::ResultSetExt::short");
}

#[test]
fn task6_method_behavior_physical_result_set_short_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::short_by_label";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::short_by_label(&target, "task6");
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::short_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_sql_xml() {
    let expected = "druid_rdbc::result_set::ResultSetExt::sql_xml";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::sql_xml(&target, ColumnIndex::new(1).unwrap());
    assert_trace(actual, "druid_rdbc::result_set::ResultSetExt::sql_xml");
}

#[test]
fn task6_method_behavior_physical_result_set_sql_xml_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::sql_xml_by_label";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::sql_xml_by_label(&target, "task6");
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::sql_xml_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_string() {
    let expected = "druid_rdbc::result_set::ResultSetExt::string";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::string(&target, ColumnIndex::new(1).unwrap());
    assert_trace(actual, "druid_rdbc::result_set::ResultSetExt::string");
}

#[test]
fn task6_method_behavior_physical_result_set_string_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::string_by_label";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::string_by_label(&target, "task6");
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::string_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_time() {
    let expected = "druid_rdbc::result_set::ResultSetExt::time";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::time(
        &target,
        ColumnIndex::new(1).unwrap(),
        &RdbcCalendarArgument::Unspecified,
    );
    assert_trace(actual, "druid_rdbc::result_set::ResultSetExt::time");
}

#[test]
fn task6_method_behavior_physical_result_set_time_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::time_by_label";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::time_by_label(&target, "task6", &RdbcCalendarArgument::Unspecified);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::time_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_timestamp() {
    let expected = "druid_rdbc::result_set::ResultSetExt::timestamp";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::timestamp(
        &target,
        ColumnIndex::new(1).unwrap(),
        &RdbcCalendarArgument::Unspecified,
    );
    assert_trace(actual, "druid_rdbc::result_set::ResultSetExt::timestamp");
}

#[test]
fn task6_method_behavior_physical_result_set_timestamp_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::timestamp_by_label";
    let target = StrictResultSet::new(expected);
    let actual =
        ResultSetExt::timestamp_by_label(&target, "task6", &RdbcCalendarArgument::Unspecified);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::timestamp_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_unicode_stream() {
    let expected = "druid_rdbc::result_set::ResultSetExt::unicode_stream";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::unicode_stream(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::unicode_stream",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_unicode_stream_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::unicode_stream_by_label";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::unicode_stream_by_label(&target, "task6");
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::unicode_stream_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_array() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_array";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_array(&mut target, ColumnIndex::new(1).unwrap(), None);
    assert_trace(actual, "druid_rdbc::result_set::ResultSetExt::update_array");
}

#[test]
fn task6_method_behavior_physical_result_set_update_array_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_array_by_label";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_array_by_label(&mut target, "task6", None);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_array_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_blob() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_blob";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_blob(&mut target, ColumnIndex::new(1).unwrap(), None);
    assert_trace(actual, "druid_rdbc::result_set::ResultSetExt::update_blob");
}

#[test]
fn task6_method_behavior_physical_result_set_update_blob_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_blob_by_label";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_blob_by_label(&mut target, "task6", None);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_blob_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_blob_stream() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_blob_stream";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_blob_stream(
        &mut target,
        ColumnIndex::new(1).unwrap(),
        None,
        RdbcStreamLength::Unspecified,
    );
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_blob_stream",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_blob_stream_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_blob_stream_by_label";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_blob_stream_by_label(
        &mut target,
        "task6",
        None,
        RdbcStreamLength::Unspecified,
    );
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_blob_stream_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_clob() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_clob";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_clob(&mut target, ColumnIndex::new(1).unwrap(), None);
    assert_trace(actual, "druid_rdbc::result_set::ResultSetExt::update_clob");
}

#[test]
fn task6_method_behavior_physical_result_set_update_clob_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_clob_by_label";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_clob_by_label(&mut target, "task6", None);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_clob_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_clob_reader() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_clob_reader";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_clob_reader(
        &mut target,
        ColumnIndex::new(1).unwrap(),
        None,
        RdbcCharacterLength::Unspecified,
    );
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_clob_reader",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_clob_reader_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_clob_reader_by_label";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_clob_reader_by_label(
        &mut target,
        "task6",
        None,
        RdbcCharacterLength::Unspecified,
    );
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_clob_reader_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_n_clob() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_n_clob";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_n_clob(&mut target, ColumnIndex::new(1).unwrap(), None);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_n_clob",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_n_clob_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_n_clob_by_label";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_n_clob_by_label(&mut target, "task6", None);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_n_clob_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_n_clob_reader() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_n_clob_reader";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_n_clob_reader(
        &mut target,
        ColumnIndex::new(1).unwrap(),
        None,
        RdbcCharacterLength::Unspecified,
    );
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_n_clob_reader",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_n_clob_reader_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_n_clob_reader_by_label";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_n_clob_reader_by_label(
        &mut target,
        "task6",
        None,
        RdbcCharacterLength::Unspecified,
    );
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_n_clob_reader_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_reference() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_reference";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_reference(&mut target, ColumnIndex::new(1).unwrap(), None);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_reference",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_reference_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_reference_by_label";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_reference_by_label(&mut target, "task6", None);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_reference_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_row_id() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_row_id";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_row_id(&mut target, ColumnIndex::new(1).unwrap(), None);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_row_id",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_row_id_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_row_id_by_label";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_row_id_by_label(&mut target, "task6", None);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_row_id_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_sql_xml() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_sql_xml";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_sql_xml(&mut target, ColumnIndex::new(1).unwrap(), None);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_sql_xml",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_update_sql_xml_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::update_sql_xml_by_label";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSetExt::update_sql_xml_by_label(&mut target, "task6", None);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSetExt::update_sql_xml_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_url() {
    let expected = "druid_rdbc::result_set::ResultSetExt::url";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::url(&target, ColumnIndex::new(1).unwrap());
    assert_trace(actual, "druid_rdbc::result_set::ResultSetExt::url");
}

#[test]
fn task6_method_behavior_physical_result_set_url_by_label() {
    let expected = "druid_rdbc::result_set::ResultSetExt::url_by_label";
    let target = StrictResultSet::new(expected);
    let actual = ResultSetExt::url_by_label(&target, "task6");
    assert_trace(actual, "druid_rdbc::result_set::ResultSetExt::url_by_label");
}
