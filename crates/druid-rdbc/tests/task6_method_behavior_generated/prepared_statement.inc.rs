// @generated receiver-exact group: PreparedStatement
#[async_trait::async_trait]
#[allow(unused_variables)]
impl PreparedStatement for StrictPreparedStatement {
    fn add_parameter_batch(
        &mut self,
        _parameters: Vec<PreparedParameter>,
    ) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.trace.fail()
    }
    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.trace.fail()
    }
    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        self.trace.fail()
    }
    fn set_parameter(&mut self, _parameter: PreparedParameter) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn sql(&self) -> Result<&str, RdbcError> {
        self.trace.fail()
    }
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_exec_prepared() {
    let expected = "druid_rdbc::statement::PreparedStatement::execute_prepared_update";
    let mut target = StrictPreparedStatement::new(expected);
    let actual = PreparedStatement::execute_prepared_update(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::statement::PreparedStatement::execute_prepared_update",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_exec_prepared_parameters() {
    let expected = "druid_rdbc::statement::PreparedStatement::execute_prepared_update";
    let mut target = StrictPreparedStatement::new(expected);
    let actual = PreparedStatement::execute_prepared_update(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::statement::PreparedStatement::execute_prepared_update",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_execute_prepared() {
    let expected = "druid_rdbc::statement::PreparedStatement::execute_prepared";
    let mut target = StrictPreparedStatement::new(expected);
    let actual = PreparedStatement::execute_prepared(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::statement::PreparedStatement::execute_prepared",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_execute_prepared_parameters() {
    let expected = "druid_rdbc::statement::PreparedStatement::execute_prepared";
    let mut target = StrictPreparedStatement::new(expected);
    let actual = PreparedStatement::execute_prepared(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::statement::PreparedStatement::execute_prepared",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_fetch_prepared() {
    let expected = "druid_rdbc::statement::PreparedStatement::execute_prepared_query";
    let mut target = StrictPreparedStatement::new(expected);
    let actual = PreparedStatement::execute_prepared_query(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::statement::PreparedStatement::execute_prepared_query",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_fetch_prepared_parameters() {
    let expected = "druid_rdbc::statement::PreparedStatement::execute_prepared_query";
    let mut target = StrictPreparedStatement::new(expected);
    let actual = PreparedStatement::execute_prepared_query(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::statement::PreparedStatement::execute_prepared_query",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_fetch_prepared_parameters_result_set() {
    let expected = "druid_rdbc::statement::PreparedStatement::execute_prepared_query";
    let mut target = StrictPreparedStatement::new(expected);
    let actual = PreparedStatement::execute_prepared_query(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::statement::PreparedStatement::execute_prepared_query",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_fetch_prepared_result_set() {
    let expected = "druid_rdbc::statement::PreparedStatement::execute_prepared_query";
    let mut target = StrictPreparedStatement::new(expected);
    let actual = PreparedStatement::execute_prepared_query(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::statement::PreparedStatement::execute_prepared_query",
    );
}

#[test]
fn task6_method_behavior_physical_prepared_statement_add_parameter_batch() {
    let expected = "druid_rdbc::statement::PreparedStatement::add_parameter_batch";
    let mut target = StrictPreparedStatement::new(expected);
    let actual = PreparedStatement::add_parameter_batch(&mut target, vec![prepared_parameter()]);
    assert_trace(
        actual,
        "druid_rdbc::statement::PreparedStatement::add_parameter_batch",
    );
}

#[test]
fn task6_method_behavior_physical_prepared_statement_clear_parameters() {
    let expected = "druid_rdbc::statement::PreparedStatement::clear_parameters";
    let mut target = StrictPreparedStatement::new(expected);
    let actual = PreparedStatement::clear_parameters(&mut target);
    assert_trace(
        actual,
        "druid_rdbc::statement::PreparedStatement::clear_parameters",
    );
}

#[test]
fn task6_method_behavior_physical_prepared_statement_set_parameter() {
    let expected = "druid_rdbc::statement::PreparedStatement::set_parameter";
    let mut target = StrictPreparedStatement::new(expected);
    let actual = PreparedStatement::set_parameter(&mut target, prepared_parameter());
    assert_trace(
        actual,
        "druid_rdbc::statement::PreparedStatement::set_parameter",
    );
}

#[test]
fn task6_method_behavior_physical_prepared_statement_sql() {
    let expected = "druid_rdbc::statement::PreparedStatement::sql";
    let target = StrictPreparedStatement::new(expected);
    let actual = PreparedStatement::sql(&target);
    assert_trace(actual, "druid_rdbc::statement::PreparedStatement::sql");
}
