//! RBDC Adapter 合同测试。

use druid::core::{RdbcError, Value};
use druid_wrapper::rbdc::RbdcConnectionFactory;
use futures::future::BoxFuture;
use futures::stream::{self, BoxStream, StreamExt};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn rbdc_standard_factory_emits_typed_runtime_trace() {
    use druid_rdbc::connection::Connection;

    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut factory = factory(Arc::clone(&observed));
    let (trace_probe, trace_expectation) = factory.enable_task4_runtime_trace();
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
        .await
        .expect("standard RBDC connection");
    let mut statement = connection
        .create_statement(druid_rdbc::statement::StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(
        statement
            .execute_update(
                "UPDATE item SET value = 1",
                druid_rdbc::statement::GeneratedKeys::None,
            )
            .await
            .unwrap(),
        1
    );
    let mut prepared = connection
        .prepare_statement(
            "SELECT ?",
            druid_rdbc::statement::StatementOptions::default(),
        )
        .await
        .expect("owned RBDC prepared statement");
    prepared
        .set_parameter(druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).unwrap(),
            value: Value::String("standard-rbdc".to_owned()),
        })
        .expect("standard RBDC setter");
    let mut rows = prepared.execute_prepared_query().await.unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.meta_data()
            .unwrap()
            .column(druid_rdbc::result_set::ColumnIndex::new(1).unwrap())
            .unwrap()
            .label(),
        "column_0"
    );
    assert_eq!(
        *observed.lock().unwrap(),
        [rbs::Value::String("standard-rbdc".to_owned())]
    );
    let mut metadata = connection.database_meta_data().await.unwrap();
    assert!(matches!(
        metadata.database_product_name().await,
        Err(RdbcError::UnsupportedOperation {
            operation: "rbdc_database_metadata_product_name"
        })
    ));
    Connection::close(connection.as_mut()).await.unwrap();
    assert!(prepared.clear_parameters().is_err());
    assert!(metadata.database_product_name().await.is_err());
    Connection::close(connection.as_mut()).await.unwrap();
    druid_wrapper::task4_runtime_trace::verify_and_emit(trace_probe, trace_expectation).unwrap();
}

#[derive(Debug)]
struct FakeMetaData {
    column_count: usize,
}

impl rbdc::db::MetaData for FakeMetaData {
    fn column_len(&self) -> usize {
        self.column_count
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
        Box::new(FakeMetaData {
            column_count: self.values.len(),
        })
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
    observed_history: Arc<Mutex<Vec<Vec<rbs::Value>>>>,
    closed: bool,
}

impl rbdc::db::Connection for FakeRbdcConnection {
    fn exec_rows(
        &mut self,
        sql: &str,
        params: Vec<rbs::Value>,
    ) -> BoxFuture<
        '_,
        Result<BoxStream<'_, Result<Box<dyn rbdc::db::Row>, rbdc::Error>>, rbdc::Error>,
    > {
        *self.observed_params.lock().expect("params lock poisoned") = params;
        self.observed_history
            .lock()
            .expect("history lock poisoned")
            .push(
                self.observed_params
                    .lock()
                    .expect("params lock poisoned")
                    .clone(),
            );
        if sql == "FAIL" {
            return Box::pin(async { Err(rbdc::Error::from("vendor database failure")) });
        }
        let strong = sql == "STRONG";
        Box::pin(async move {
            let values = if strong {
                vec![
                    rbs::Value::Ext(
                        "Decimal",
                        Box::new(rbs::Value::String("123456789.125".to_string())),
                    ),
                    rbs::Value::Ext(
                        "Date",
                        Box::new(rbs::Value::String("2026-07-29".to_string())),
                    ),
                    rbs::Value::Ext(
                        "Time",
                        Box::new(rbs::Value::String("11:12:13.456789".to_string())),
                    ),
                    rbs::Value::Ext(
                        "DateTime",
                        Box::new(rbs::Value::String("2026-07-29 11:12:13.456789".to_string())),
                    ),
                ]
            } else {
                vec![
                    rbs::Value::Null,
                    rbs::Value::Bool(true),
                    rbs::Value::I64(7),
                    rbs::Value::F64(2.5),
                    rbs::Value::String("rbdc".to_string()),
                    rbs::Value::Binary(vec![1, 2]),
                ]
            };
            let row = FakeRow { values };
            let rows: BoxStream<'_, Result<Box<dyn rbdc::db::Row>, rbdc::Error>> =
                stream::iter(vec![Ok(Box::new(row) as Box<dyn rbdc::db::Row>)]).boxed();
            Ok(rows)
        })
    }

    fn exec(
        &mut self,
        sql: &str,
        params: Vec<rbs::Value>,
    ) -> BoxFuture<'_, Result<rbdc::db::ExecResult, rbdc::Error>> {
        let batch_command = params.first().and_then(rbs::Value::as_i64);
        *self.observed_params.lock().expect("params lock poisoned") = params;
        self.observed_history
            .lock()
            .expect("history lock poisoned")
            .push(
                self.observed_params
                    .lock()
                    .expect("params lock poisoned")
                    .clone(),
            );
        if sql == "FAIL" || batch_command == Some(2) {
            return Box::pin(async { Err(rbdc::Error::from("vendor database failure")) });
        }
        let rows_affected = if batch_command == Some(1) {
            u64::try_from(i64::from(i32::MAX) + 7).expect("fixture count fits u64")
        } else {
            1
        };
        Box::pin(async move {
            Ok(rbdc::db::ExecResult {
                rows_affected,
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
                observed_history: Arc::new(Mutex::new(Vec::new())),
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
    observed_history: Arc<Mutex<Vec<Vec<rbs::Value>>>>,
}

impl rbdc::db::Driver for FakeDriver {
    fn name(&self) -> &'static str {
        "fake-rbdc"
    }

    fn connect(
        &self,
        _url: &str,
    ) -> BoxFuture<'_, Result<Box<dyn rbdc::db::Connection>, rbdc::Error>> {
        let observed_params = self.observed_params.clone();
        let observed_history = self.observed_history.clone();
        Box::pin(async move {
            Ok(Box::new(FakeRbdcConnection {
                observed_params,
                observed_history,
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
    factory_with_history(observed_params, Arc::new(Mutex::new(Vec::new())))
}

fn factory_with_history(
    observed_params: Arc<Mutex<Vec<rbs::Value>>>,
    observed_history: Arc<Mutex<Vec<Vec<rbs::Value>>>>,
) -> RbdcConnectionFactory {
    RbdcConnectionFactory::new(
        Arc::new(FakeDriver {
            observed_params,
            observed_history,
        }),
        "fake://database",
    )
}

#[tokio::test]
async fn rbdc_connection_first_close_rejects_prepared_and_live_metadata() {
    let factory = factory(Arc::new(Mutex::new(Vec::new())));
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
        .await
        .unwrap();
    let mut prepared = connection
        .prepare_statement(
            "SELECT value FROM item WHERE id = ?",
            druid_rdbc::statement::StatementOptions::default(),
        )
        .await
        .unwrap();
    let mut metadata = connection.database_meta_data().await.unwrap();
    druid_rdbc::connection::Connection::close(connection.as_mut())
        .await
        .unwrap();
    assert!(prepared
        .set_parameter(druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).unwrap(),
            value: Value::Int(1),
        })
        .is_err());
    assert!(prepared.clear_parameters().is_err());
    assert!(prepared.execute_prepared_query().await.is_err());
    assert!(metadata.database_product_name().await.is_err());
    assert!(matches!(
        metadata
            .tables(druid_rdbc::metadata::TablesQuery::default())
            .await,
        Err(RdbcError::ConnectionDiscarded)
    ));

    let mut second = druid_rdbc::driver::ConnectionFactory::create(&factory)
        .await
        .unwrap();
    let mut statement = second
        .create_statement(druid_rdbc::statement::StatementOptions::default())
        .await
        .unwrap();
    statement.close().await.unwrap();
    druid_rdbc::connection::Connection::close(second.as_mut())
        .await
        .unwrap();
    druid_rdbc::connection::Connection::close(second.as_mut())
        .await
        .unwrap();
}
