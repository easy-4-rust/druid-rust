// @generated receiver-exact group: ResultSetMetaData
#[allow(unused_variables)]
impl ResultSetMetaData for StrictResultSetMetaData {
    fn catalog_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.trace.fail()
    }
    fn column(&self, _column_index: ColumnIndex) -> Result<ColumnDescriptor, RdbcError> {
        self.trace.fail()
    }
    fn column_class_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.trace.fail()
    }
    fn column_count(&self) -> usize {
        7
    }
    fn column_display_size(&self, column: ColumnIndex) -> Result<usize, RdbcError> {
        self.trace.fail()
    }
    fn column_label(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.trace.fail()
    }
    fn column_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.trace.fail()
    }
    fn column_type(&self, column: ColumnIndex) -> Result<ResultSetColumnType, RdbcError> {
        self.trace.fail()
    }
    fn column_type_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.trace.fail()
    }
    fn is_auto_increment(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn is_case_sensitive(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn is_currency(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn is_definitely_writable(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn is_read_only(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn is_searchable(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn is_signed(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn is_writable(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn nullability(&self, column: ColumnIndex) -> Result<ColumnNullability, RdbcError> {
        self.trace.fail()
    }
    fn precision(&self, column: ColumnIndex) -> Result<usize, RdbcError> {
        self.trace.fail()
    }
    fn scale(&self, column: ColumnIndex) -> Result<usize, RdbcError> {
        self.trace.fail()
    }
    fn schema_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.trace.fail()
    }
    fn table_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.trace.fail()
    }
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_catalog_name() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::catalog_name";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::catalog_name(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::catalog_name",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_column_class_name() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::column_class_name";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::column_class_name(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::column_class_name",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_column_count() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::column_count";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::column_count(&target);
    assert_eq!(actual, 7);
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_column_display_size() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::column_display_size";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::column_display_size(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::column_display_size",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_column_label() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::column_label";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::column_label(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::column_label",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_column_name() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::column_name";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::column_name(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::column_name",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_column_type() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::column_type";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::column_type(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::column_type",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_column_type_name() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::column_type_name";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::column_type_name(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::column_type_name",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_is_auto_increment() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::is_auto_increment";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::is_auto_increment(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::is_auto_increment",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_is_case_sensitive() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::is_case_sensitive";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::is_case_sensitive(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::is_case_sensitive",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_is_currency() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::is_currency";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::is_currency(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::is_currency",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_is_definitely_writable() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::is_definitely_writable";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::is_definitely_writable(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::is_definitely_writable",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_is_read_only() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::is_read_only";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::is_read_only(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::is_read_only",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_is_searchable() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::is_searchable";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::is_searchable(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::is_searchable",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_is_signed() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::is_signed";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::is_signed(&target, ColumnIndex::new(1).unwrap());
    assert_trace(actual, "druid_rdbc::metadata::ResultSetMetaData::is_signed");
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_is_writable() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::is_writable";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::is_writable(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::is_writable",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_nullability() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::nullability";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::nullability(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::nullability",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_precision() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::precision";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::precision(&target, ColumnIndex::new(1).unwrap());
    assert_trace(actual, "druid_rdbc::metadata::ResultSetMetaData::precision");
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_scale() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::scale";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::scale(&target, ColumnIndex::new(1).unwrap());
    assert_trace(actual, "druid_rdbc::metadata::ResultSetMetaData::scale");
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_schema_name() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::schema_name";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::schema_name(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::schema_name",
    );
}

#[test]
fn task6_method_behavior_physical_result_set_meta_data_table_name() {
    let expected = "druid_rdbc::metadata::ResultSetMetaData::table_name";
    let target = StrictResultSetMetaData::new(expected);
    let actual = ResultSetMetaData::table_name(&target, ColumnIndex::new(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::metadata::ResultSetMetaData::table_name",
    );
}
