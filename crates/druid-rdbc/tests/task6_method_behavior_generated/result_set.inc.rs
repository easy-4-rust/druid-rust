// @generated receiver-exact group: ResultSet
#[async_trait::async_trait]
#[allow(unused_variables)]
impl ResultSet for StrictResultSet {
    async fn absolute(&mut self, row: i32) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    async fn after_last(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn before_first(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn cancel_row_updates(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn concurrency(&self) -> Result<ResultSetConcurrency, RdbcError> {
        self.trace.fail()
    }
    fn cursor_name(&self) -> Result<Option<String>, RdbcError> {
        self.trace.fail()
    }
    fn delete_row(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        self.trace.fail()
    }
    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.trace.fail()
    }
    fn find_column(&self, _column_label: &str) -> Result<ColumnIndex, RdbcError> {
        self.trace.fail()
    }
    async fn first(&mut self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        self.trace.fail()
    }
    fn input_stream(
        &self,
        _column: ColumnIndex,
        kind: ResultSetStreamKind,
    ) -> Result<Option<RdbcInputStream>, RdbcError> {
        assert_eq!(_column, ColumnIndex::new(1).unwrap());
        let expected_kind = match self.trace.expected {
            "druid_rdbc::result_set::ResultSetExt::ascii_stream" => ResultSetStreamKind::Ascii,
            "druid_rdbc::result_set::ResultSetExt::unicode_stream" => ResultSetStreamKind::Unicode,
            "druid_rdbc::result_set::ResultSetExt::binary_stream" => ResultSetStreamKind::Binary,
            unexpected => panic!("unexpected stream probe: {unexpected}"),
        };
        assert_eq!(kind, expected_kind);
        self.trace.fail()
    }
    fn insert_row(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn is_after_last(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn is_before_first(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn is_first(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn is_last(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    async fn last(&mut self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.trace.fail()
    }
    fn move_to_current_row(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn move_to_insert_row(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn object_as(
        &self,
        column: ColumnIndex,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        self.trace.fail()
    }
    fn object_as_by_label(
        &self,
        label: &str,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        self.trace.fail()
    }
    fn object_with_type_map(
        &self,
        _column: ColumnIndex,
        _type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        self.trace.fail()
    }
    fn object_with_type_map_by_label(
        &self,
        _label: &str,
        _type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        self.trace.fail()
    }
    async fn previous(&mut self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn reader(
        &self,
        _column: ColumnIndex,
        national: bool,
    ) -> Result<Option<RdbcReader>, RdbcError> {
        assert_eq!(_column, ColumnIndex::new(1).unwrap());
        let expected_national = match self.trace.expected {
            "druid_rdbc::result_set::ResultSetExt::character_stream" => false,
            "druid_rdbc::result_set::ResultSetExt::n_character_stream" => true,
            unexpected => panic!("unexpected reader probe: {unexpected}"),
        };
        assert_eq!(national, expected_national);
        self.trace.fail()
    }
    fn refresh_row(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn relative(&mut self, rows: i32) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn result_set_type(&self) -> Result<ResultSetType, RdbcError> {
        self.trace.fail()
    }
    fn row(&self) -> Result<i32, RdbcError> {
        self.trace.fail()
    }
    fn row_deleted(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn row_inserted(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn row_updated(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn set_fetch_direction(&mut self, _direction: FetchDirection) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn set_fetch_size(&mut self, _rows: i32) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn update_row(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn update_value(
        &mut self,
        _column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn update_value_by_label(
        &mut self,
        _column_label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn value(&self, _column: ColumnIndex) -> Result<Value, RdbcError> {
        self.trace.fail()
    }
    fn value_by_label(&self, column_label: &str) -> Result<Value, RdbcError> {
        self.trace.fail()
    }
    fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        self.trace.fail()
    }
    fn was_null(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
}

#[tokio::test]
async fn task6_method_behavior_physical_result_set_absolute() {
    let expected = "druid_rdbc::result_set::ResultSet::absolute";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::absolute(&mut target, 1).await;
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::absolute");
}

#[tokio::test]
async fn task6_method_behavior_physical_result_set_after_last() {
    let expected = "druid_rdbc::result_set::ResultSet::after_last";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::after_last(&mut target).await;
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::after_last");
}

#[tokio::test]
async fn task6_method_behavior_physical_result_set_before_first() {
    let expected = "druid_rdbc::result_set::ResultSet::before_first";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::before_first(&mut target).await;
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::before_first");
}

#[test]
fn task6_method_behavior_physical_result_set_cancel_row_updates() {
    let expected = "druid_rdbc::result_set::ResultSet::cancel_row_updates";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::cancel_row_updates(&mut target);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSet::cancel_row_updates",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_clear_warnings() {
    let expected = "druid_rdbc::result_set::ResultSet::clear_warnings";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::clear_warnings(&mut target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::clear_warnings");
}

#[tokio::test]
async fn task6_method_behavior_physical_result_set_close() {
    let expected = "druid_rdbc::result_set::ResultSet::close";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::close(&mut target).await;
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::close");
}

#[test]
fn task6_method_behavior_physical_result_set_concurrency() {
    let expected = "druid_rdbc::result_set::ResultSet::concurrency";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::concurrency(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::concurrency");
}

#[test]
fn task6_method_behavior_physical_result_set_cursor_name() {
    let expected = "druid_rdbc::result_set::ResultSet::cursor_name";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::cursor_name(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::cursor_name");
}

#[test]
fn task6_method_behavior_physical_result_set_delete_row() {
    let expected = "druid_rdbc::result_set::ResultSet::delete_row";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::delete_row(&mut target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::delete_row");
}

#[test]
fn task6_method_behavior_physical_result_set_fetch_direction() {
    let expected = "druid_rdbc::result_set::ResultSet::fetch_direction";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::fetch_direction(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::fetch_direction");
}

#[test]
fn task6_method_behavior_physical_result_set_fetch_size() {
    let expected = "druid_rdbc::result_set::ResultSet::fetch_size";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::fetch_size(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::fetch_size");
}

#[test]
fn task6_method_behavior_physical_result_set_find_column() {
    let expected = "druid_rdbc::result_set::ResultSet::find_column";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::find_column(&target, "task6");
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::find_column");
}

#[tokio::test]
async fn task6_method_behavior_physical_result_set_first() {
    let expected = "druid_rdbc::result_set::ResultSet::first";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::first(&mut target).await;
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::first");
}

#[test]
fn task6_method_behavior_physical_result_set_holdability() {
    let expected = "druid_rdbc::result_set::ResultSet::holdability";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::holdability(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::holdability");
}

#[test]
fn task6_method_behavior_physical_result_set_insert_row() {
    let expected = "druid_rdbc::result_set::ResultSet::insert_row";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::insert_row(&mut target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::insert_row");
}

#[test]
fn task6_method_behavior_physical_result_set_is_after_last() {
    let expected = "druid_rdbc::result_set::ResultSet::is_after_last";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::is_after_last(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::is_after_last");
}

#[test]
fn task6_method_behavior_physical_result_set_is_before_first() {
    let expected = "druid_rdbc::result_set::ResultSet::is_before_first";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::is_before_first(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::is_before_first");
}

#[test]
fn task6_method_behavior_physical_result_set_is_closed() {
    let expected = "druid_rdbc::result_set::ResultSet::is_closed";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::is_closed(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::is_closed");
}

#[test]
fn task6_method_behavior_physical_result_set_is_first() {
    let expected = "druid_rdbc::result_set::ResultSet::is_first";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::is_first(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::is_first");
}

#[test]
fn task6_method_behavior_physical_result_set_is_last() {
    let expected = "druid_rdbc::result_set::ResultSet::is_last";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::is_last(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::is_last");
}

#[tokio::test]
async fn task6_method_behavior_physical_result_set_last() {
    let expected = "druid_rdbc::result_set::ResultSet::last";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::last(&mut target).await;
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::last");
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data() {
    let expected = "druid_rdbc::result_set::ResultSet::meta_data";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::meta_data(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::meta_data");
}

#[test]
fn task6_method_behavior_physical_result_set_move_to_current_row() {
    let expected = "druid_rdbc::result_set::ResultSet::move_to_current_row";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::move_to_current_row(&mut target);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSet::move_to_current_row",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_move_to_insert_row() {
    let expected = "druid_rdbc::result_set::ResultSet::move_to_insert_row";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::move_to_insert_row(&mut target);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSet::move_to_insert_row",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_result_set_next() {
    let expected = "druid_rdbc::result_set::ResultSet::next";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::next(&mut target).await;
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::next");
}

#[test]
fn task6_method_behavior_physical_result_set_object_as() {
    let expected = "druid_rdbc::result_set::ResultSet::object_as";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::object_as(
        &target,
        ColumnIndex::new(1).unwrap(),
        &RdbcTargetType::Integer,
    );
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::object_as");
}

#[test]
fn task6_method_behavior_physical_result_set_object_with_type_map() {
    let expected = "druid_rdbc::result_set::ResultSet::object_with_type_map";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::object_with_type_map(&target, ColumnIndex::new(1).unwrap(), None);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSet::object_with_type_map",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_result_set_previous() {
    let expected = "druid_rdbc::result_set::ResultSet::previous";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::previous(&mut target).await;
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::previous");
}

#[test]
fn task6_method_behavior_physical_result_set_refresh_row() {
    let expected = "druid_rdbc::result_set::ResultSet::refresh_row";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::refresh_row(&mut target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::refresh_row");
}

#[tokio::test]
async fn task6_method_behavior_physical_result_set_relative() {
    let expected = "druid_rdbc::result_set::ResultSet::relative";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::relative(&mut target, 1).await;
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::relative");
}

#[test]
fn task6_method_behavior_physical_result_set_result_set_type() {
    let expected = "druid_rdbc::result_set::ResultSet::result_set_type";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::result_set_type(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::result_set_type");
}

#[test]
fn task6_method_behavior_physical_result_set_row() {
    let expected = "druid_rdbc::result_set::ResultSet::row";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::row(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::row");
}

#[test]
fn task6_method_behavior_physical_result_set_row_deleted() {
    let expected = "druid_rdbc::result_set::ResultSet::row_deleted";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::row_deleted(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::row_deleted");
}

#[test]
fn task6_method_behavior_physical_result_set_row_inserted() {
    let expected = "druid_rdbc::result_set::ResultSet::row_inserted";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::row_inserted(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::row_inserted");
}

#[test]
fn task6_method_behavior_physical_result_set_row_updated() {
    let expected = "druid_rdbc::result_set::ResultSet::row_updated";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::row_updated(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::row_updated");
}

#[test]
fn task6_method_behavior_physical_result_set_set_fetch_direction() {
    let expected = "druid_rdbc::result_set::ResultSet::set_fetch_direction";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::set_fetch_direction(&mut target, FetchDirection::Forward);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSet::set_fetch_direction",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_set_fetch_size() {
    let expected = "druid_rdbc::result_set::ResultSet::set_fetch_size";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::set_fetch_size(&mut target, 1);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::set_fetch_size");
}

#[test]
fn task6_method_behavior_physical_result_set_update_row() {
    let expected = "druid_rdbc::result_set::ResultSet::update_row";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::update_row(&mut target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::update_row");
}

#[test]
fn task6_method_behavior_physical_result_set_update_value() {
    let expected = "druid_rdbc::result_set::ResultSet::update_value";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::update_value(
        &mut target,
        ColumnIndex::new(1).unwrap(),
        ResultSetUpdate::Null,
    );
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::update_value");
}

#[test]
fn task6_method_behavior_physical_result_set_update_value_by_label() {
    let expected = "druid_rdbc::result_set::ResultSet::update_value_by_label";
    let mut target = StrictResultSet::new(expected);
    let actual = ResultSet::update_value_by_label(&mut target, "task6", ResultSetUpdate::Null);
    assert_trace(
        actual,
        "druid_rdbc::result_set::ResultSet::update_value_by_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_value() {
    let expected = "druid_rdbc::result_set::ResultSet::value";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::value(&target, ColumnIndex::new(1).unwrap());
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::value");
}

#[test]
fn task6_method_behavior_physical_result_set_value_by_label() {
    let expected = "druid_rdbc::result_set::ResultSet::value_by_label";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::value_by_label(&target, "task6");
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::value_by_label");
}

#[test]
fn task6_method_behavior_physical_result_set_warnings() {
    let expected = "druid_rdbc::result_set::ResultSet::warnings";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::warnings(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::warnings");
}

#[test]
fn task6_method_behavior_physical_result_set_was_null() {
    let expected = "druid_rdbc::result_set::ResultSet::was_null";
    let target = StrictResultSet::new(expected);
    let actual = ResultSet::was_null(&target);
    assert_trace(actual, "druid_rdbc::result_set::ResultSet::was_null");
}
