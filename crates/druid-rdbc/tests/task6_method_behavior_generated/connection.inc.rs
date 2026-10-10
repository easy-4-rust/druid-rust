// @generated receiver-exact group: Connection
#[async_trait::async_trait]
#[allow(unused_variables)]
impl Connection for StrictConnection {
    async fn abort(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn auto_commit(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    async fn begin(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn capabilities(&self) -> ConnectionCapabilities {
        ConnectionCapabilities::default()
    }
    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        self.trace.fail()
    }
    fn client_info(&self, _name: &str) -> Result<Option<String>, RdbcError> {
        self.trace.fail()
    }
    async fn commit(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn create_blob(&mut self) -> Result<Box<dyn Blob>, RdbcError> {
        self.trace.fail()
    }
    async fn create_clob(&mut self) -> Result<Box<dyn Clob>, RdbcError> {
        self.trace.fail()
    }
    async fn create_n_clob(&mut self) -> Result<Box<dyn NClob>, RdbcError> {
        self.trace.fail()
    }
    async fn database_meta_data(&mut self) -> Result<Box<dyn DatabaseMetaData>, RdbcError> {
        self.trace.fail()
    }
    async fn database_product_name(&mut self) -> Result<Option<String>, RdbcError> {
        self.trace.fail()
    }
    async fn database_product_version(&mut self) -> Result<Option<String>, RdbcError> {
        self.trace.fail()
    }
    fn driver_major_version(&self) -> Result<i32, RdbcError> {
        self.trace.fail()
    }
    fn driver_minor_version(&self) -> Result<i32, RdbcError> {
        self.trace.fail()
    }
    fn driver_name(&self) -> Result<String, RdbcError> {
        self.trace.fail()
    }
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        self.trace.fail()
    }
    async fn exec_batch(
        &mut self,
        batch: Vec<(String, Vec<Value>)>,
    ) -> Result<Vec<i64>, RdbcError> {
        self.trace.fail()
    }
    async fn exec_prepared_batch(
        &mut self,
        statement: &mut dyn PreparedStatement,
        parameter_sets: Vec<Vec<Value>>,
    ) -> Result<Vec<i64>, RdbcError> {
        self.trace.fail()
    }
    async fn exec_prepared_parameter_batch(
        &mut self,
        statement: &mut dyn PreparedStatement,
        parameter_sets: Vec<Vec<PreparedInputParameter>>,
    ) -> Result<Vec<i64>, RdbcError> {
        self.trace.fail()
    }
    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        self.trace.fail()
    }
    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        self.trace.fail()
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    async fn native_sql(&mut self, sql: &str) -> Result<String, RdbcError> {
        self.trace.fail()
    }
    fn network_timeout(&self) -> Result<Duration, RdbcError> {
        self.trace.fail()
    }
    async fn ping(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn prepare_call(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        self.trace.fail()
    }
    async fn prepare_statement(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        self.trace.fail()
    }
    fn read_only(&self) -> Result<bool, RdbcError> {
        self.trace.fail()
    }
    async fn release_savepoint(&mut self, _savepoint: &Savepoint) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn rollback(&mut self) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn rollback_to(&mut self, _savepoint: &Savepoint) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn schema(&self) -> Result<Option<String>, RdbcError> {
        self.trace.fail()
    }
    async fn set_auto_commit(&mut self, _enabled: bool) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn set_catalog(&mut self, _catalog: Option<&str>) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn set_client_info(
        &mut self,
        _name: &str,
        _value: Option<&str>,
    ) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn set_holdability(
        &mut self,
        _holdability: ResultSetHoldability,
    ) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn set_network_timeout(&mut self, _timeout: Duration) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn set_read_only(&mut self, _read_only: bool) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn set_savepoint(&mut self) -> Result<Savepoint, RdbcError> {
        self.trace.fail()
    }
    async fn set_savepoint_named(&mut self, _name: &str) -> Result<Savepoint, RdbcError> {
        self.trace.fail()
    }
    async fn set_schema(&mut self, _schema: Option<&str>) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn set_transaction_isolation(
        &mut self,
        _level: TransactionIsolation,
    ) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    async fn set_type_map(&mut self, _type_map: RdbcTypeMap) -> Result<(), RdbcError> {
        self.trace.fail()
    }
    fn transaction_isolation(&self) -> Result<TransactionIsolation, RdbcError> {
        self.trace.fail()
    }
    fn type_map(&self) -> Result<Option<RdbcTypeMap>, RdbcError> {
        self.trace.fail()
    }
}

#[test]
fn task6_method_behavior_connection_ext_get_client_info() {
    let expected = "druid_rdbc::connection::Connection::client_info";
    let target = StrictConnection::new(expected);
    let actual = Connection::client_info(&target, "task6");
    assert_trace(actual, "druid_rdbc::connection::Connection::client_info");
}

#[tokio::test]
async fn task6_method_behavior_connection_ext_get_database_product_name() {
    let expected = "druid_rdbc::connection::Connection::database_product_name";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::database_product_name(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::database_product_name",
    );
}

#[tokio::test]
async fn task6_method_behavior_connection_ext_get_database_product_version() {
    let expected = "druid_rdbc::connection::Connection::database_product_version";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::database_product_version(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::database_product_version",
    );
}

#[test]
fn task6_method_behavior_connection_ext_get_driver_major_version() {
    let expected = "druid_rdbc::connection::Connection::driver_major_version";
    let target = StrictConnection::new(expected);
    let actual = Connection::driver_major_version(&target);
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::driver_major_version",
    );
}

#[test]
fn task6_method_behavior_connection_ext_get_driver_minor_version() {
    let expected = "druid_rdbc::connection::Connection::driver_minor_version";
    let target = StrictConnection::new(expected);
    let actual = Connection::driver_minor_version(&target);
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::driver_minor_version",
    );
}

#[test]
fn task6_method_behavior_connection_ext_get_holdability() {
    let expected = "druid_rdbc::connection::Connection::holdability";
    let target = StrictConnection::new(expected);
    let actual = Connection::holdability(&target);
    assert_trace(actual, "druid_rdbc::connection::Connection::holdability");
}

#[tokio::test]
async fn task6_method_behavior_connection_ext_get_meta_data() {
    let expected = "druid_rdbc::connection::Connection::database_meta_data";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::database_meta_data(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::database_meta_data",
    );
}

#[test]
fn task6_method_behavior_connection_ext_get_network_timeout() {
    let expected = "druid_rdbc::connection::Connection::network_timeout";
    let target = StrictConnection::new(expected);
    let actual = Connection::network_timeout(&target);
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::network_timeout",
    );
}

#[test]
fn task6_method_behavior_connection_ext_get_type_map() {
    let expected = "druid_rdbc::connection::Connection::type_map";
    let target = StrictConnection::new(expected);
    let actual = Connection::type_map(&target);
    assert_trace(actual, "druid_rdbc::connection::Connection::type_map");
}

#[tokio::test]
async fn task6_method_behavior_connection_ext_native_sql() {
    let expected = "druid_rdbc::connection::Connection::native_sql";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::native_sql(&mut target, "task6").await;
    assert_trace(actual, "druid_rdbc::connection::Connection::native_sql");
}

#[tokio::test]
async fn task6_method_behavior_connection_ext_prepare_call() {
    let expected = "druid_rdbc::connection::Connection::prepare_call";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::prepare_call(&mut target, "task6", StatementOptions::default()).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::prepare_call");
}

#[tokio::test]
async fn task6_method_behavior_connection_ext_prepare_statement() {
    let expected = "druid_rdbc::connection::Connection::prepare_statement";
    let mut target = StrictConnection::new(expected);
    let actual =
        Connection::prepare_statement(&mut target, "task6", StatementOptions::default()).await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::prepare_statement",
    );
}

#[tokio::test]
async fn task6_method_behavior_connection_ext_set_client_info() {
    let expected = "druid_rdbc::connection::Connection::set_client_info";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::set_client_info(&mut target, "task6", Some("task6")).await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::set_client_info",
    );
}

#[tokio::test]
async fn task6_method_behavior_connection_ext_set_holdability() {
    let expected = "druid_rdbc::connection::Connection::set_holdability";
    let mut target = StrictConnection::new(expected);
    let actual =
        Connection::set_holdability(&mut target, ResultSetHoldability::CloseAtCommit).await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::set_holdability",
    );
}

#[tokio::test]
async fn task6_method_behavior_connection_ext_set_network_timeout() {
    let expected = "druid_rdbc::connection::Connection::set_network_timeout";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::set_network_timeout(&mut target, Duration::from_millis(1)).await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::set_network_timeout",
    );
}

#[tokio::test]
async fn task6_method_behavior_connection_ext_set_type_map() {
    let expected = "druid_rdbc::connection::Connection::set_type_map";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::set_type_map(&mut target, RdbcTypeMap::new()).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::set_type_map");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_abort() {
    let expected = "druid_rdbc::connection::Connection::abort";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::abort(&mut target).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::abort");
}

#[test]
fn task6_method_behavior_physical_connection_auto_commit() {
    let expected = "druid_rdbc::connection::Connection::auto_commit";
    let target = StrictConnection::new(expected);
    let actual = Connection::auto_commit(&target);
    assert_trace(actual, "druid_rdbc::connection::Connection::auto_commit");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_begin() {
    let expected = "druid_rdbc::connection::Connection::begin";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::begin(&mut target).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::begin");
}

#[test]
fn task6_method_behavior_physical_connection_capabilities() {
    let expected = "druid_rdbc::connection::Connection::capabilities";
    let target = StrictConnection::new(expected);
    let actual = Connection::capabilities(&target);
    assert_eq!(actual, ConnectionCapabilities::default());
}

#[test]
fn task6_method_behavior_physical_connection_catalog() {
    let expected = "druid_rdbc::connection::Connection::catalog";
    let target = StrictConnection::new(expected);
    let actual = Connection::catalog(&target);
    assert_trace(actual, "druid_rdbc::connection::Connection::catalog");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_commit() {
    let expected = "druid_rdbc::connection::Connection::commit";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::commit(&mut target).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::commit");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_create_blob() {
    let expected = "druid_rdbc::connection::Connection::create_blob";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::create_blob(&mut target).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::create_blob");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_create_clob() {
    let expected = "druid_rdbc::connection::Connection::create_clob";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::create_clob(&mut target).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::create_clob");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_create_n_clob() {
    let expected = "druid_rdbc::connection::Connection::create_n_clob";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::create_n_clob(&mut target).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::create_n_clob");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_database_meta_data() {
    let expected = "druid_rdbc::connection::Connection::database_meta_data";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::database_meta_data(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::database_meta_data",
    );
}

#[test]
fn task6_method_behavior_physical_connection_driver_name() {
    let expected = "druid_rdbc::connection::Connection::driver_name";
    let target = StrictConnection::new(expected);
    let actual = Connection::driver_name(&target);
    assert_trace(actual, "druid_rdbc::connection::Connection::driver_name");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_exec() {
    let expected = "druid_rdbc::connection::Connection::exec";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::exec(&mut target, "task6", vec![Value::Int(1)]).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::exec");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_exec_batch() {
    let expected = "druid_rdbc::connection::Connection::exec_batch";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::exec_batch(&mut target, vec![("SELECT 1".to_owned(), vec![])]).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::exec_batch");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_exec_prepared_batch() {
    let expected = "druid_rdbc::connection::Connection::exec_prepared_batch";
    let mut target = StrictConnection::new(expected);
    let mut prepared = StrictPreparedStatement::new(expected);
    let actual =
        Connection::exec_prepared_batch(&mut target, &mut prepared, vec![vec![Value::Int(1)]])
            .await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::exec_prepared_batch",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_exec_prepared_parameter_batch() {
    let expected = "druid_rdbc::connection::Connection::exec_prepared_parameter_batch";
    let mut target = StrictConnection::new(expected);
    let mut prepared = StrictPreparedStatement::new(expected);
    let actual = Connection::exec_prepared_parameter_batch(
        &mut target,
        &mut prepared,
        vec![vec![PreparedInputParameter::RustValue(Value::Int(1))]],
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::exec_prepared_parameter_batch",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_fetch() {
    let expected = "druid_rdbc::connection::Connection::fetch";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::fetch(&mut target, "task6", vec![Value::Int(1)]).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::fetch");
}

#[test]
fn task6_method_behavior_physical_connection_holdability() {
    let expected = "druid_rdbc::connection::Connection::holdability";
    let target = StrictConnection::new(expected);
    let actual = Connection::holdability(&target);
    assert_trace(actual, "druid_rdbc::connection::Connection::holdability");
}

#[test]
fn task6_method_behavior_physical_connection_is_closed() {
    let expected = "druid_rdbc::connection::Connection::is_closed";
    let target = StrictConnection::new(expected);
    let actual = Connection::is_closed(&target);
    assert_trace(actual, "druid_rdbc::connection::Connection::is_closed");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_ping() {
    let expected = "druid_rdbc::connection::Connection::ping";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::ping(&mut target).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::ping");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_prepare_physical_call() {
    let expected = "druid_rdbc::connection::Connection::prepare_call";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::prepare_call(&mut target, "task6", StatementOptions::default()).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::prepare_call");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_prepare_physical_statement() {
    let expected = "druid_rdbc::connection::Connection::prepare_statement";
    let mut target = StrictConnection::new(expected);
    let actual =
        Connection::prepare_statement(&mut target, "task6", StatementOptions::default()).await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::prepare_statement",
    );
}

#[test]
fn task6_method_behavior_physical_connection_read_only() {
    let expected = "druid_rdbc::connection::Connection::read_only";
    let target = StrictConnection::new(expected);
    let actual = Connection::read_only(&target);
    assert_trace(actual, "druid_rdbc::connection::Connection::read_only");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_release_savepoint() {
    let expected = "druid_rdbc::connection::Connection::release_savepoint";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::release_savepoint(&mut target, &Savepoint { id: 1, name: None }).await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::release_savepoint",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_rollback() {
    let expected = "druid_rdbc::connection::Connection::rollback";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::rollback(&mut target).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::rollback");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_rollback_to() {
    let expected = "druid_rdbc::connection::Connection::rollback_to";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::rollback_to(&mut target, &Savepoint { id: 1, name: None }).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::rollback_to");
}

#[test]
fn task6_method_behavior_physical_connection_schema() {
    let expected = "druid_rdbc::connection::Connection::schema";
    let target = StrictConnection::new(expected);
    let actual = Connection::schema(&target);
    assert_trace(actual, "druid_rdbc::connection::Connection::schema");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_set_auto_commit() {
    let expected = "druid_rdbc::connection::Connection::set_auto_commit";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::set_auto_commit(&mut target, true).await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::set_auto_commit",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_set_catalog() {
    let expected = "druid_rdbc::connection::Connection::set_catalog";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::set_catalog(&mut target, Some("task6")).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::set_catalog");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_set_holdability() {
    let expected = "druid_rdbc::connection::Connection::set_holdability";
    let mut target = StrictConnection::new(expected);
    let actual =
        Connection::set_holdability(&mut target, ResultSetHoldability::CloseAtCommit).await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::set_holdability",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_set_read_only() {
    let expected = "druid_rdbc::connection::Connection::set_read_only";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::set_read_only(&mut target, true).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::set_read_only");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_set_savepoint() {
    let expected = "druid_rdbc::connection::Connection::set_savepoint";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::set_savepoint(&mut target).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::set_savepoint");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_set_savepoint_named() {
    let expected = "druid_rdbc::connection::Connection::set_savepoint_named";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::set_savepoint_named(&mut target, "task6").await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::set_savepoint_named",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_set_schema() {
    let expected = "druid_rdbc::connection::Connection::set_schema";
    let mut target = StrictConnection::new(expected);
    let actual = Connection::set_schema(&mut target, Some("task6")).await;
    assert_trace(actual, "druid_rdbc::connection::Connection::set_schema");
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_set_transaction_isolation() {
    let expected = "druid_rdbc::connection::Connection::set_transaction_isolation";
    let mut target = StrictConnection::new(expected);
    let actual =
        Connection::set_transaction_isolation(&mut target, TransactionIsolation::READ_COMMITTED)
            .await;
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::set_transaction_isolation",
    );
}

#[test]
fn task6_method_behavior_physical_connection_transaction_isolation() {
    let expected = "druid_rdbc::connection::Connection::transaction_isolation";
    let target = StrictConnection::new(expected);
    let actual = Connection::transaction_isolation(&target);
    assert_trace(
        actual,
        "druid_rdbc::connection::Connection::transaction_isolation",
    );
}
