// @generated receiver-exact group: CallableStatement
#[async_trait::async_trait]
#[allow(unused_variables)]
impl CallableStatement for StrictCallableStatement {
    fn array_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcArray>, RdbcError> {
        self.trace.fail()
    }
    fn big_decimal_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<BigDecimal>, RdbcError> {
        self.trace.fail()
    }
    fn big_decimal_out_parameter_with_scale(
        &self,
        parameter: &CallableParameter,
        scale: i32,
    ) -> Result<Option<BigDecimal>, RdbcError> {
        self.trace.fail()
    }
    fn blob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcBlob>, RdbcError> {
        self.trace.fail()
    }
    fn boolean_out_parameter(&self, parameter: &CallableParameter) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn byte_out_parameter(&self, parameter: &CallableParameter) -> Result<i8, RdbcError> {
        self.trace.fail()
    }
    fn bytes_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<Vec<u8>>, RdbcError> {
        self.trace.fail()
    }
    fn character_stream_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcReader>, RdbcError> {
        self.trace.fail()
    }
    fn clob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcClob>, RdbcError> {
        self.trace.fail()
    }
    fn date_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDate>, RdbcError> {
        self.trace.fail()
    }
    fn double_out_parameter(&self, parameter: &CallableParameter) -> Result<f64, RdbcError> {
        self.trace.fail()
    }
    fn float_out_parameter(&self, parameter: &CallableParameter) -> Result<f32, RdbcError> {
        self.trace.fail()
    }
    fn int_out_parameter(&self, parameter: &CallableParameter) -> Result<i32, RdbcError> {
        self.trace.fail()
    }
    fn integer_out_parameter(&self, parameter: &CallableParameter) -> Result<i64, RdbcError> {
        self.trace.fail()
    }
    fn long_out_parameter(&self, parameter: &CallableParameter) -> Result<i64, RdbcError> {
        self.trace.fail()
    }
    fn n_character_stream_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcReader>, RdbcError> {
        self.trace.fail()
    }
    fn n_clob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcNClob>, RdbcError> {
        self.trace.fail()
    }
    fn n_string_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<String>, RdbcError> {
        self.trace.fail()
    }
    fn out_parameter(&self, parameter: &CallableParameter) -> Result<RdbcObject, RdbcError> {
        self.trace.fail()
    }
    fn out_parameter_as(
        &self,
        _parameter: &CallableParameter,
        _target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        self.trace.fail()
    }
    fn out_parameter_with_type_map(
        &self,
        _parameter: &CallableParameter,
        _type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        self.trace.fail()
    }
    fn ref_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcRef>, RdbcError> {
        self.trace.fail()
    }
    fn register_out_parameter(
        &mut self,
        _parameter: CallableParameter,
        _out_parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn row_id_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcRowId>, RdbcError> {
        self.trace.fail()
    }
    fn set_named_parameter(
        &mut self,
        _parameter_name: &str,
        _parameter: CallableInputParameter,
    ) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn short_out_parameter(&self, parameter: &CallableParameter) -> Result<i16, RdbcError> {
        self.trace.fail()
    }
    fn sql_xml_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcSqlXml>, RdbcError> {
        self.trace.fail()
    }
    fn string_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<String>, RdbcError> {
        self.trace.fail()
    }
    fn time_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveTime>, RdbcError> {
        self.trace.fail()
    }
    fn timestamp_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, RdbcError> {
        self.trace.fail()
    }
    fn url_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcUrl>, RdbcError> {
        self.trace.fail()
    }
    fn was_null(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
}

#[test]
fn task6_method_behavior_physical_callable_statement_array_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::array_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::array_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::array_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_big_decimal_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::big_decimal_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual = CallableStatement::big_decimal_out_parameter(
        &target,
        &CallableParameter::by_index(1).unwrap(),
    );
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::big_decimal_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_big_decimal_out_parameter_with_scale() {
    let expected = "druid_rdbc::statement::CallableStatement::big_decimal_out_parameter_with_scale";
    let target = StrictCallableStatement::new(expected);
    let actual = CallableStatement::big_decimal_out_parameter_with_scale(
        &target,
        &CallableParameter::by_index(1).unwrap(),
        1,
    );
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::big_decimal_out_parameter_with_scale",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_blob_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::blob_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::blob_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::blob_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_boolean_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::boolean_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::boolean_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::boolean_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_byte_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::byte_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::byte_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::byte_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_bytes_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::bytes_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::bytes_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::bytes_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_character_stream_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::character_stream_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual = CallableStatement::character_stream_out_parameter(
        &target,
        &CallableParameter::by_index(1).unwrap(),
    );
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::character_stream_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_clob_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::clob_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::clob_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::clob_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_date_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::date_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual = CallableStatement::date_out_parameter(
        &target,
        &CallableParameter::by_index(1).unwrap(),
        &RdbcCalendarArgument::Unspecified,
    );
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::date_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_double_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::double_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::double_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::double_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_float_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::float_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::float_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::float_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_int_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::int_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::int_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::int_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_integer_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::integer_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::integer_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::integer_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_long_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::long_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::long_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::long_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_n_character_stream_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::n_character_stream_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual = CallableStatement::n_character_stream_out_parameter(
        &target,
        &CallableParameter::by_index(1).unwrap(),
    );
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::n_character_stream_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_n_clob_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::n_clob_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::n_clob_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::n_clob_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_n_string_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::n_string_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual = CallableStatement::n_string_out_parameter(
        &target,
        &CallableParameter::by_index(1).unwrap(),
    );
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::n_string_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_out_parameter_as() {
    let expected = "druid_rdbc::statement::CallableStatement::out_parameter_as";
    let target = StrictCallableStatement::new(expected);
    let actual = CallableStatement::out_parameter_as(
        &target,
        &CallableParameter::by_index(1).unwrap(),
        &RdbcTargetType::Integer,
    );
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::out_parameter_as",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_out_parameter_with_type_map() {
    let expected = "druid_rdbc::statement::CallableStatement::out_parameter_with_type_map";
    let target = StrictCallableStatement::new(expected);
    let actual = CallableStatement::out_parameter_with_type_map(
        &target,
        &CallableParameter::by_index(1).unwrap(),
        None,
    );
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::out_parameter_with_type_map",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_ref_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::ref_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::ref_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::ref_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_register_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::register_out_parameter";
    let mut target = StrictCallableStatement::new(expected);
    let actual = CallableStatement::register_out_parameter(
        &mut target,
        CallableParameter::by_index(1).unwrap(),
        CallableOutParameter::new(4),
    );
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::register_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_row_id_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::row_id_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::row_id_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::row_id_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_set_named_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::set_named_parameter";
    let mut target = StrictCallableStatement::new(expected);
    let actual = CallableStatement::set_named_parameter(
        &mut target,
        "task6",
        CallableInputParameter::Int(1),
    );
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::set_named_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_short_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::short_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::short_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::short_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_sql_xml_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::sql_xml_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::sql_xml_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::sql_xml_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_string_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::string_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::string_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::string_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_time_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::time_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual = CallableStatement::time_out_parameter(
        &target,
        &CallableParameter::by_index(1).unwrap(),
        &RdbcCalendarArgument::Unspecified,
    );
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::time_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_timestamp_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::timestamp_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual = CallableStatement::timestamp_out_parameter(
        &target,
        &CallableParameter::by_index(1).unwrap(),
        &RdbcCalendarArgument::Unspecified,
    );
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::timestamp_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_url_out_parameter() {
    let expected = "druid_rdbc::statement::CallableStatement::url_out_parameter";
    let target = StrictCallableStatement::new(expected);
    let actual =
        CallableStatement::url_out_parameter(&target, &CallableParameter::by_index(1).unwrap());
    assert_trace(
        actual,
        "druid_rdbc::statement::CallableStatement::url_out_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_callable_statement_was_null() {
    let expected = "druid_rdbc::statement::CallableStatement::was_null";
    let target = StrictCallableStatement::new(expected);
    let actual = CallableStatement::was_null(&target);
    assert_trace(actual, "druid_rdbc::statement::CallableStatement::was_null");
}
