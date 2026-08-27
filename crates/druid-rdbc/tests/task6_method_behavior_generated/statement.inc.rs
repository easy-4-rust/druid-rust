// @generated receiver-exact group: Statement
#[async_trait::async_trait]
#[allow(unused_variables)]
impl Statement for StrictStatement {
    fn add_batch(&mut self, _sql: String) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn batch(&self) -> Result<Vec<String>, RdbcError> {
        self.trace.fail()
    }
    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn execute(
        &mut self,
        _sql: &str,
        _generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.trace.fail()
    }
    async fn execute_query(&mut self, _sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.trace.fail()
    }
    async fn get_generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.trace.fail()
    }
    async fn get_more_results(
        &mut self,
        _disposition: CurrentResultDisposition,
    ) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    async fn get_result_set(&mut self) -> Result<Option<Box<dyn ResultSet>>, RdbcError> {
        self.trace.fail()
    }
    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        self.trace.fail()
    }
    fn is_close_on_completion(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn is_poolable(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    fn options(&self) -> Result<StatementOptions, RdbcError> {
        self.trace.fail()
    }
    fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        self.trace.fail()
    }
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_execute() {
    let expected = "druid_rdbc::statement::Statement::execute";
    let mut target = StrictStatement::new(expected);
    let actual = Statement::execute(&mut target, "task6", GeneratedKeys::None).await;
    assert_trace(actual, "druid_rdbc::statement::Statement::execute");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_fetch_result_set() {
    let expected = "druid_rdbc::statement::Statement::execute_query";
    let mut target = StrictStatement::new(expected);
    let actual = Statement::execute_query(&mut target, "task6").await;
    assert_trace(actual, "druid_rdbc::statement::Statement::execute_query");
}

#[test]
fn task6_method_behavior_physical_prepared_statement_add_batch() {
    let expected = "druid_rdbc::statement::Statement::add_batch";
    let mut target = StrictStatement::new(expected);
    let actual = Statement::add_batch(&mut target, "task6".to_owned());
    assert_trace(actual, "druid_rdbc::statement::Statement::add_batch");
}

#[test]
fn task6_method_behavior_physical_prepared_statement_clear_batch() {
    let expected = "druid_rdbc::statement::Statement::clear_batch";
    let mut target = StrictStatement::new(expected);
    let actual = Statement::clear_batch(&mut target);
    assert_trace(actual, "druid_rdbc::statement::Statement::clear_batch");
}

#[test]
fn task6_method_behavior_physical_prepared_statement_clear_warnings() {
    let expected = "druid_rdbc::statement::Statement::clear_warnings";
    let mut target = StrictStatement::new(expected);
    let actual = Statement::clear_warnings(&mut target);
    assert_trace(actual, "druid_rdbc::statement::Statement::clear_warnings");
}

#[tokio::test]
async fn task6_method_behavior_physical_prepared_statement_get_generated_keys() {
    let expected = "druid_rdbc::statement::Statement::get_generated_keys";
    let mut target = StrictStatement::new(expected);
    let actual = Statement::get_generated_keys(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::statement::Statement::get_generated_keys",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_prepared_statement_get_more_results() {
    let expected = "druid_rdbc::statement::Statement::get_more_results";
    let mut target = StrictStatement::new(expected);
    let actual = Statement::get_more_results(&mut target, CurrentResultDisposition::Default).await;
    assert_trace(actual, "druid_rdbc::statement::Statement::get_more_results");
}

#[tokio::test]
async fn task6_method_behavior_physical_prepared_statement_get_result_set() {
    let expected = "druid_rdbc::statement::Statement::get_result_set";
    let mut target = StrictStatement::new(expected);
    let actual = Statement::get_result_set(&mut target).await;
    assert_trace(actual, "druid_rdbc::statement::Statement::get_result_set");
}

#[test]
fn task6_method_behavior_physical_prepared_statement_get_update_count() {
    let expected = "druid_rdbc::statement::Statement::get_update_count";
    let target = StrictStatement::new(expected);
    let actual = Statement::get_update_count(&target);
    assert_trace(actual, "druid_rdbc::statement::Statement::get_update_count");
}

#[test]
fn task6_method_behavior_physical_prepared_statement_is_close_on_completion() {
    let expected = "druid_rdbc::statement::Statement::is_close_on_completion";
    let target = StrictStatement::new(expected);
    let actual = Statement::is_close_on_completion(&target);
    assert_trace(
        actual,
        "druid_rdbc::statement::Statement::is_close_on_completion",
    );
}

#[test]
fn task6_method_behavior_physical_prepared_statement_is_closed() {
    let expected = "druid_rdbc::statement::Statement::is_closed";
    let target = StrictStatement::new(expected);
    let actual = Statement::is_closed(&target);
    assert_trace(actual, "druid_rdbc::statement::Statement::is_closed");
}

#[test]
fn task6_method_behavior_physical_prepared_statement_statement_options() {
    let expected = "druid_rdbc::statement::Statement::options";
    let target = StrictStatement::new(expected);
    let actual = Statement::options(&target);
    assert_trace(actual, "druid_rdbc::statement::Statement::options");
}

#[test]
fn task6_method_behavior_physical_prepared_statement_warnings() {
    let expected = "druid_rdbc::statement::Statement::warnings";
    let target = StrictStatement::new(expected);
    let actual = Statement::warnings(&target);
    assert_trace(actual, "druid_rdbc::statement::Statement::warnings");
}

#[test]
fn task6_method_behavior_physical_statement_add_batch() {
    let expected = "druid_rdbc::statement::Statement::add_batch";
    let mut target = StrictStatement::new(expected);
    let actual = Statement::add_batch(&mut target, "task6".to_owned());
    assert_trace(actual, "druid_rdbc::statement::Statement::add_batch");
}

#[test]
fn task6_method_behavior_physical_statement_batch() {
    let expected = "druid_rdbc::statement::Statement::batch";
    let target = StrictStatement::new(expected);
    let actual = Statement::batch(&target);
    assert_trace(actual, "druid_rdbc::statement::Statement::batch");
}

#[test]
fn task6_method_behavior_physical_statement_clear_batch() {
    let expected = "druid_rdbc::statement::Statement::clear_batch";
    let mut target = StrictStatement::new(expected);
    let actual = Statement::clear_batch(&mut target);
    assert_trace(actual, "druid_rdbc::statement::Statement::clear_batch");
}

#[test]
fn task6_method_behavior_physical_statement_clear_warnings() {
    let expected = "druid_rdbc::statement::Statement::clear_warnings";
    let mut target = StrictStatement::new(expected);
    let actual = Statement::clear_warnings(&mut target);
    assert_trace(actual, "druid_rdbc::statement::Statement::clear_warnings");
}

#[tokio::test]
async fn task6_method_behavior_physical_statement_get_generated_keys() {
    let expected = "druid_rdbc::statement::Statement::get_generated_keys";
    let mut target = StrictStatement::new(expected);
    let actual = Statement::get_generated_keys(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::statement::Statement::get_generated_keys",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_statement_get_more_results() {
    let expected = "druid_rdbc::statement::Statement::get_more_results";
    let mut target = StrictStatement::new(expected);
    let actual = Statement::get_more_results(&mut target, CurrentResultDisposition::Default).await;
    assert_trace(actual, "druid_rdbc::statement::Statement::get_more_results");
}

#[tokio::test]
async fn task6_method_behavior_physical_statement_get_result_set() {
    let expected = "druid_rdbc::statement::Statement::get_result_set";
    let mut target = StrictStatement::new(expected);
    let actual = Statement::get_result_set(&mut target).await;
    assert_trace(actual, "druid_rdbc::statement::Statement::get_result_set");
}

#[test]
fn task6_method_behavior_physical_statement_get_update_count() {
    let expected = "druid_rdbc::statement::Statement::get_update_count";
    let target = StrictStatement::new(expected);
    let actual = Statement::get_update_count(&target);
    assert_trace(actual, "druid_rdbc::statement::Statement::get_update_count");
}

#[test]
fn task6_method_behavior_physical_statement_is_close_on_completion() {
    let expected = "druid_rdbc::statement::Statement::is_close_on_completion";
    let target = StrictStatement::new(expected);
    let actual = Statement::is_close_on_completion(&target);
    assert_trace(
        actual,
        "druid_rdbc::statement::Statement::is_close_on_completion",
    );
}

#[test]
fn task6_method_behavior_physical_statement_is_closed() {
    let expected = "druid_rdbc::statement::Statement::is_closed";
    let target = StrictStatement::new(expected);
    let actual = Statement::is_closed(&target);
    assert_trace(actual, "druid_rdbc::statement::Statement::is_closed");
}

#[test]
fn task6_method_behavior_physical_statement_is_poolable() {
    let expected = "druid_rdbc::statement::Statement::is_poolable";
    let target = StrictStatement::new(expected);
    let actual = Statement::is_poolable(&target);
    assert_trace(actual, "druid_rdbc::statement::Statement::is_poolable");
}

#[test]
fn task6_method_behavior_physical_statement_options() {
    let expected = "druid_rdbc::statement::Statement::options";
    let target = StrictStatement::new(expected);
    let actual = Statement::options(&target);
    assert_trace(actual, "druid_rdbc::statement::Statement::options");
}

#[test]
fn task6_method_behavior_physical_statement_warnings() {
    let expected = "druid_rdbc::statement::Statement::warnings";
    let target = StrictStatement::new(expected);
    let actual = Statement::warnings(&target);
    assert_trace(actual, "druid_rdbc::statement::Statement::warnings");
}
