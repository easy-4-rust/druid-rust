//! RBDC Adapter 合同测试。

use druid_core::{ConnectionFactory, PreparedStatementKey, PreparedStatementMethodType, Value};
use druid_rbdc::RbdcConnectionFactory;
use futures::future::BoxFuture;
use futures::stream::{self, BoxStream, StreamExt};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct FakeMetaData;

impl rbdc::db::MetaData for FakeMetaData {
    fn column_len(&self) -> usize {
        6
    }

    fn column_name(&self, index: usize) -> String {
        format!("column_{index}")
    }

    fn column_type(&self, _index: usize) -> String {
        "test".to_string()
    }
}

#[derive(Debug)]
struct FakeRow {
    values: Vec<rbs::Value>,
}

impl rbdc::db::Row for FakeRow {
    fn meta_data(&self) -> Box<dyn rbdc::db::MetaData> {
        Box::new(FakeMetaData)
    }

    fn get(&mut self, index: usize) -> Result<rbs::Value, rbdc::Error> {
        self.values
            .get(index)
            .cloned()
            .ok_or_else(|| rbdc::Error::from("column index out of bounds"))
    }
}

#[derive(Debug)]
struct FakeRbdcConnection {
    observed_params: Arc<Mutex<Vec<rbs::Value>>>,
    closed: bool,
}

impl rbdc::db::Connection for FakeRbdcConnection {
    fn exec_rows(
        &mut self,
        _sql: &str,
        params: Vec<rbs::Value>,
    ) -> BoxFuture<
        '_,
        Result<BoxStream<'_, Result<Box<dyn rbdc::db::Row>, rbdc::Error>>, rbdc::Error>,
    > {
        *self.observed_params.lock().expect("params lock poisoned") = params;
        Box::pin(async {
            let row = FakeRow {
                values: vec![
                    rbs::Value::Null,
                    rbs::Value::Bool(true),
                    rbs::Value::I64(7),
                    rbs::Value::F64(2.5),
                    rbs::Value::String("rbdc".to_string()),
                    rbs::Value::Binary(vec![1, 2]),
                ],
            };
            let rows: BoxStream<'_, Result<Box<dyn rbdc::db::Row>, rbdc::Error>> =
                stream::iter(vec![Ok(Box::new(row) as Box<dyn rbdc::db::Row>)]).boxed();
            Ok(rows)
        })
    }

    fn exec(
        &mut self,
        _sql: &str,
        params: Vec<rbs::Value>,
    ) -> BoxFuture<'_, Result<rbdc::db::ExecResult, rbdc::Error>> {
        *self.observed_params.lock().expect("params lock poisoned") = params;
        Box::pin(async {
            Ok(rbdc::db::ExecResult {
                rows_affected: 1,
                last_insert_id: rbs::Value::I64(9),
            })
        })
    }

    fn ping(&mut self) -> BoxFuture<'_, Result<(), rbdc::Error>> {
        Box::pin(async { Ok(()) })
    }

    fn close(&mut self) -> BoxFuture<'_, Result<(), rbdc::Error>> {
        self.closed = true;
        Box::pin(async { Ok(()) })
    }
}

#[derive(Debug)]
struct FakeConnectOptions;

impl rbdc::db::ConnectOptions for FakeConnectOptions {
    fn connect(&self) -> BoxFuture<'_, Result<Box<dyn rbdc::db::Connection>, rbdc::Error>> {
        Box::pin(async {
            Ok(Box::new(FakeRbdcConnection {
                observed_params: Arc::new(Mutex::new(Vec::new())),
                closed: false,
            }) as Box<dyn rbdc::db::Connection>)
        })
    }

    fn set_uri(&mut self, _uri: &str) -> Result<(), rbdc::Error> {
        Ok(())
    }
}

#[derive(Debug)]
struct FakeDriver {
    observed_params: Arc<Mutex<Vec<rbs::Value>>>,
}

impl rbdc::db::Driver for FakeDriver {
    fn name(&self) -> &str {
        "fake-rbdc"
    }

    fn connect(
        &self,
        _url: &str,
    ) -> BoxFuture<'_, Result<Box<dyn rbdc::db::Connection>, rbdc::Error>> {
        let observed_params = self.observed_params.clone();
        Box::pin(async move {
            Ok(Box::new(FakeRbdcConnection {
                observed_params,
                closed: false,
            }) as Box<dyn rbdc::db::Connection>)
        })
    }

    fn connect_opt<'a>(
        &'a self,
        _options: &'a dyn rbdc::db::ConnectOptions,
    ) -> BoxFuture<'a, Result<Box<dyn rbdc::db::Connection>, rbdc::Error>> {
        self.connect("fake://options")
    }

    fn default_option(&self) -> Box<dyn rbdc::db::ConnectOptions> {
        Box::new(FakeConnectOptions)
    }
}

fn factory(observed_params: Arc<Mutex<Vec<rbs::Value>>>) -> RbdcConnectionFactory {
    RbdcConnectionFactory::new(Arc::new(FakeDriver { observed_params }), "fake://database")
}

#[tokio::test]
async fn rbdc_factory_creates_real_physical_adapter() {
    let observed_params = Arc::new(Mutex::new(Vec::new()));
    let factory = factory(observed_params.clone());
    assert_eq!(factory.url(), "fake://database");
    assert_eq!(factory.driver_name(), "fake-rbdc");

    let mut connection = factory.create().await.expect("factory create must succeed");
    assert_eq!(connection.driver_name(), "fake-rbdc");
    factory
        .validate(&mut connection)
        .await
        .expect("factory validate must ping");

    let result = connection
        .exec(
            "INSERT INTO item VALUES (?, ?, ?, ?, ?, ?)",
            vec![
                Value::Null,
                Value::Bool(true),
                Value::Int(7),
                Value::Float(2.5),
                Value::String("rbdc".to_string()),
                Value::Bytes(vec![1, 2]),
            ],
        )
        .await
        .expect("RBDC exec must succeed");
    assert_eq!(result.rows_affected, 1);
    assert_eq!(result.last_insert_id, Some(9));
    assert_eq!(
        *observed_params.lock().expect("params lock poisoned"),
        vec![
            rbs::Value::Null,
            rbs::Value::Bool(true),
            rbs::Value::I64(7),
            rbs::Value::F64(2.5),
            rbs::Value::String("rbdc".to_string()),
            rbs::Value::Binary(vec![1, 2]),
        ]
    );
}

#[tokio::test]
async fn rbdc_adapter_fetch_transaction_and_savepoint_semantics() {
    let factory = factory(Arc::new(Mutex::new(Vec::new())));
    let mut connection = factory.create().await.expect("factory create must succeed");
    let rows = connection
        .fetch("SELECT value FROM item", vec![Value::Int(1)])
        .await
        .expect("RBDC fetch must succeed");
    assert_eq!(
        rows[0].values,
        vec![
            Value::Null,
            Value::Bool(true),
            Value::Int(7),
            Value::Float(2.5),
            Value::String("rbdc".to_string()),
            Value::Bytes(vec![1, 2]),
        ]
    );

    connection.begin().await.expect("transaction must begin");
    assert!(!connection.auto_commit());
    let savepoint = connection
        .set_savepoint_named("rbdc_contract")
        .await
        .expect("savepoint must be created");
    connection
        .rollback_to(&savepoint)
        .await
        .expect("rollback to savepoint must succeed");
    connection
        .release_savepoint(&savepoint)
        .await
        .expect("savepoint release must succeed");
    connection.commit().await.expect("transaction must commit");
    assert!(connection.auto_commit());

    assert!(connection.set_savepoint_named("unsafe;name").await.is_err());
    connection.close().await.expect("close must succeed");
    connection
        .close()
        .await
        .expect("duplicate close must succeed");
    assert!(connection.is_closed());
}

#[tokio::test]
async fn rbdc_adapter_maps_prepared_statement_to_driver_exec_contract() {
    let observed_params = Arc::new(Mutex::new(Vec::new()));
    let factory = factory(observed_params.clone());
    let mut connection = factory.create().await.expect("factory create must succeed");
    let key = PreparedStatementKey::new(
        Some("INSERT INTO item VALUES (?)".to_string()),
        None,
        PreparedStatementMethodType::M1,
    )
    .expect("prepared key must build");
    let statement = connection
        .prepare_physical_statement(&key)
        .await
        .expect("RBDC prepared token must build");

    let result = connection
        .exec_prepared(
            statement.as_ref(),
            vec![Value::String("prepared".to_string())],
        )
        .await
        .expect("RBDC prepared execution must delegate to exec");
    assert_eq!(result.rows_affected, 1);
    assert_eq!(
        *observed_params.lock().expect("params lock poisoned"),
        vec![rbs::Value::String("prepared".to_string())]
    );

    connection
        .close_prepared_statement(statement.clone())
        .await
        .expect("RBDC prepared token must close");
    assert!(connection
        .exec_prepared(statement.as_ref(), vec![])
        .await
        .is_err());
}
