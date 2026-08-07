//! RBDC Adapter 合同测试。

use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    ConnectionFactory, DruidError, DruidPooledConnection, DruidPooledPreparedStatement, JavaString,
    PhysicalArray, PhysicalBlob, PhysicalClob, PhysicalConnection, PhysicalNClob, PhysicalRef,
    PhysicalSqlXml, PreparedStatementKey, PreparedStatementMethodType, RdbcArray, RdbcBlob,
    RdbcClob, RdbcInputStream, RdbcNClob, RdbcObject, RdbcOutputStream, RdbcReader, RdbcRef,
    RdbcResultSet, RdbcRowId, RdbcSqlXml, RdbcTypeMap, RdbcUrl, RdbcWriter,
    RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource, Value,
};
use druid_wrapper::rbdc::RbdcConnectionFactory;
use futures::future::BoxFuture;
use futures::stream::{self, BoxStream, StreamExt};
use std::any::Any;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

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

fn unsupported_resource<T>() -> Result<T, DruidError> {
    Err(DruidError::UnsupportedOperation {
        operation: "test_resource_operation",
    })
}

#[derive(Debug)]
struct FixtureBlob {
    reported_length: i64,
    bytes: Vec<u8>,
}

impl PhysicalBlob for FixtureBlob {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn length(&self) -> Result<i64, DruidError> {
        Ok(self.reported_length)
    }

    fn get_bytes(&self, _position: i64, _length: i32) -> Result<Vec<u8>, DruidError> {
        Ok(self.bytes.clone())
    }

    fn get_binary_stream(&self) -> Result<RdbcInputStream, DruidError> {
        unsupported_resource()
    }

    fn position_bytes(&self, _pattern: &[u8], _start: i64) -> Result<Option<i64>, DruidError> {
        unsupported_resource()
    }

    fn position_blob(&self, _pattern: &RdbcBlob, _start: i64) -> Result<Option<i64>, DruidError> {
        unsupported_resource()
    }

    fn set_bytes(&self, _position: i64, _bytes: &[u8]) -> Result<i32, DruidError> {
        unsupported_resource()
    }

    fn set_bytes_range(
        &self,
        _position: i64,
        _bytes: &[u8],
        _offset: i32,
        _length: i32,
    ) -> Result<i32, DruidError> {
        unsupported_resource()
    }

    fn set_binary_stream(&self, _position: i64) -> Result<RdbcOutputStream, DruidError> {
        unsupported_resource()
    }

    fn truncate(&self, _length: i64) -> Result<(), DruidError> {
        unsupported_resource()
    }

    fn free(&self) -> Result<(), DruidError> {
        unsupported_resource()
    }

    fn is_freed(&self) -> bool {
        false
    }

    fn get_binary_stream_range(
        &self,
        _position: i64,
        _length: i64,
    ) -> Result<RdbcInputStream, DruidError> {
        unsupported_resource()
    }
}

#[derive(Debug)]
struct FixtureClob {
    reported_length: i64,
    value: JavaString,
}

impl PhysicalClob for FixtureClob {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn length(&self) -> Result<i64, DruidError> {
        Ok(self.reported_length)
    }

    fn get_sub_string(&self, _position: i64, _length: i32) -> Result<JavaString, DruidError> {
        Ok(self.value.clone())
    }

    fn get_character_stream(&self) -> Result<RdbcReader, DruidError> {
        unsupported_resource()
    }

    fn get_ascii_stream(&self) -> Result<RdbcInputStream, DruidError> {
        unsupported_resource()
    }

    fn position_string(
        &self,
        _pattern: &JavaString,
        _start: i64,
    ) -> Result<Option<i64>, DruidError> {
        unsupported_resource()
    }

    fn position_clob(&self, _pattern: &RdbcClob, _start: i64) -> Result<Option<i64>, DruidError> {
        unsupported_resource()
    }

    fn set_string(&self, _position: i64, _value: &JavaString) -> Result<i32, DruidError> {
        unsupported_resource()
    }

    fn set_string_range(
        &self,
        _position: i64,
        _value: &JavaString,
        _offset: i32,
        _length: i32,
    ) -> Result<i32, DruidError> {
        unsupported_resource()
    }

    fn set_ascii_stream(&self, _position: i64) -> Result<RdbcOutputStream, DruidError> {
        unsupported_resource()
    }

    fn set_character_stream(&self, _position: i64) -> Result<RdbcWriter, DruidError> {
        unsupported_resource()
    }

    fn truncate(&self, _length: i64) -> Result<(), DruidError> {
        unsupported_resource()
    }

    fn free(&self) -> Result<(), DruidError> {
        unsupported_resource()
    }

    fn is_freed(&self) -> bool {
        false
    }

    fn get_character_stream_range(
        &self,
        _position: i64,
        _length: i64,
    ) -> Result<RdbcReader, DruidError> {
        unsupported_resource()
    }
}

impl PhysicalNClob for FixtureClob {}

#[derive(Debug)]
struct FixtureSqlXml {
    value: JavaString,
    fail_string: bool,
}

impl PhysicalSqlXml for FixtureSqlXml {
    fn free(&self) -> Result<(), DruidError> {
        unsupported_resource()
    }

    fn is_freed(&self) -> bool {
        false
    }

    fn binary_stream(&self) -> Result<RdbcInputStream, DruidError> {
        unsupported_resource()
    }

    fn set_binary_stream(&self) -> Result<RdbcOutputStream, DruidError> {
        unsupported_resource()
    }

    fn character_stream(&self) -> Result<RdbcReader, DruidError> {
        unsupported_resource()
    }

    fn set_character_stream(&self) -> Result<RdbcWriter, DruidError> {
        unsupported_resource()
    }

    fn string(&self) -> Result<JavaString, DruidError> {
        if self.fail_string {
            return Err(DruidError::DriverError(
                "injected SQLXML string failure".to_string(),
            ));
        }
        Ok(self.value.clone())
    }

    fn set_string(&self, _value: &JavaString) -> Result<(), DruidError> {
        unsupported_resource()
    }

    fn source(
        &self,
        _representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlSource, DruidError> {
        unsupported_resource()
    }

    fn result(
        &self,
        _representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlResult, DruidError> {
        unsupported_resource()
    }
}

#[derive(Debug)]
struct FixtureRef;

impl PhysicalRef for FixtureRef {
    fn base_type_name(&self) -> Result<String, DruidError> {
        unsupported_resource()
    }

    fn object(&self) -> Result<RdbcObject, DruidError> {
        unsupported_resource()
    }

    fn object_with_type_map(&self, _type_map: &RdbcTypeMap) -> Result<RdbcObject, DruidError> {
        unsupported_resource()
    }

    fn set_object(&self, _value: RdbcObject) -> Result<(), DruidError> {
        unsupported_resource()
    }
}

#[derive(Debug)]
struct FixtureArray;

impl PhysicalArray for FixtureArray {
    fn base_type_name(&self) -> Result<String, DruidError> {
        unsupported_resource()
    }

    fn base_type(&self) -> Result<i32, DruidError> {
        unsupported_resource()
    }

    fn values(&self) -> Result<Vec<RdbcObject>, DruidError> {
        unsupported_resource()
    }

    fn values_with_type_map(&self, _type_map: &RdbcTypeMap) -> Result<Vec<RdbcObject>, DruidError> {
        unsupported_resource()
    }

    fn values_range(&self, _index: i64, _count: i32) -> Result<Vec<RdbcObject>, DruidError> {
        unsupported_resource()
    }

    fn values_range_with_type_map(
        &self,
        _index: i64,
        _count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, DruidError> {
        unsupported_resource()
    }

    fn result_set(&self) -> Result<RdbcResultSet, DruidError> {
        unsupported_resource()
    }

    fn result_set_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcResultSet, DruidError> {
        unsupported_resource()
    }

    fn result_set_range(&self, _index: i64, _count: i32) -> Result<RdbcResultSet, DruidError> {
        unsupported_resource()
    }

    fn result_set_range_with_type_map(
        &self,
        _index: i64,
        _count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcResultSet, DruidError> {
        unsupported_resource()
    }

    fn free(&self) -> Result<(), DruidError> {
        unsupported_resource()
    }

    fn is_freed(&self) -> bool {
        false
    }
}

#[tokio::test]
async fn rbdc_adapter_preserves_decimal_and_temporal_extension_types() {
    let observed_params = Arc::new(Mutex::new(Vec::new()));
    let factory = factory(observed_params.clone());
    let mut connection = factory.create().await.expect("factory create must succeed");
    let decimal = BigDecimal::from_str("123456789.125").unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_micro_opt(11, 12, 13, 456_789).unwrap();
    let timestamp = NaiveDateTime::new(date, time);

    connection
        .exec(
            "INSERT_STRONG",
            vec![
                Value::Decimal(decimal.clone()),
                Value::Date(date),
                Value::Time(time),
                Value::Timestamp(timestamp),
            ],
        )
        .await
        .expect("RBDC strong parameters must be encoded");
    assert_eq!(
        *observed_params.lock().expect("params lock poisoned"),
        vec![
            rbs::Value::Ext("Decimal", Box::new(rbs::Value::String(decimal.to_string()))),
            rbs::Value::Ext("Date", Box::new(rbs::Value::String(date.to_string()))),
            rbs::Value::Ext("Time", Box::new(rbs::Value::String(time.to_string()))),
            rbs::Value::Ext(
                "DateTime",
                Box::new(rbs::Value::String(timestamp.to_string()))
            ),
        ]
    );

    let rows = connection
        .fetch("STRONG", Vec::new())
        .await
        .expect("RBDC strong results must be decoded");
    assert_eq!(
        rows[0].values,
        vec![
            Value::Decimal(decimal),
            Value::Date(date),
            Value::Time(time),
            Value::Timestamp(timestamp),
        ]
    );
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
    fn name(&self) -> &str {
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
async fn rbdc_pooled_result_set_preserves_driver_column_labels() {
    let factory = factory(Arc::new(Mutex::new(Vec::new())));
    let physical = factory.create().await.expect("factory create must succeed");
    let mut connection = DruidPooledConnection::new(physical, 702, Box::new(|_, _| {}));
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "STRONG")
        .await
        .unwrap();

    let meta_data = result_set.meta_data(&mut connection).unwrap();
    assert_eq!(meta_data.column_count().unwrap(), 4);
    assert_eq!(meta_data.column_label(1).unwrap(), "column_0");
    assert_eq!(meta_data.column_label(4).unwrap(), "column_3");
    assert_eq!(
        result_set.find_column(&mut connection, "COLUMN_3").unwrap(),
        4
    );
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(
        result_set
            .string_by_label(&mut connection, "column_1")
            .unwrap()
            .as_deref(),
        Some("2026-07-29")
    );
    result_set.close_with_connection(&mut connection).unwrap();
}

#[tokio::test]
async fn rbdc_adapter_preserves_warning_and_connection_state_semantics() {
    let factory = factory(Arc::new(Mutex::new(Vec::new())));
    let mut connection = factory.create().await.expect("factory create must succeed");

    assert!(connection.capabilities().clear_warnings);
    assert_eq!(
        connection
            .warnings()
            .await
            .expect("live RBDC connection must expose warning semantics"),
        None
    );
    connection
        .clear_warnings()
        .await
        .expect("live RBDC connection must clear warning state");

    connection.mark_discarded();
    assert!(matches!(
        connection.warnings().await,
        Err(DruidError::ConnectionDiscarded)
    ));
    assert!(matches!(
        connection.clear_warnings().await,
        Err(DruidError::ConnectionDiscarded)
    ));
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

#[tokio::test]
async fn rbdc_prepared_resources_preserve_setter_timing_and_batch_snapshots() {
    let observed_params = Arc::new(Mutex::new(Vec::new()));
    let observed_history = Arc::new(Mutex::new(Vec::new()));
    let factory = factory_with_history(observed_params, observed_history.clone());
    let physical = factory.create().await.expect("factory create must succeed");
    let mut connection = DruidPooledConnection::new(physical, 700, Box::new(|_, _| {}));

    let mut invalid = connection.prepare_statement("INVALID ?").await.unwrap();
    let short = RdbcInputStream::from_bytes(vec![1, 2]);
    assert!(matches!(
        invalid.set_binary_stream_with_int_length(&mut connection, 1, Some(short.clone()), 3,),
        Err(DruidError::DriverError(_))
    ));
    assert!(short.read_to_end().unwrap().is_empty());
    invalid.close_with_connection(&mut connection).unwrap();

    let binary = RdbcInputStream::from_bytes(vec![1, 2, 3]);
    let reader = RdbcReader::from_string("rbdc-reader");
    let mut statement = connection
        .prepare_statement("INSERT_RESOURCE ?, ?, ?, ?")
        .await
        .unwrap();
    statement
        .set_binary_stream_with_int_length(&mut connection, 1, Some(binary.clone()), 2)
        .unwrap();
    statement
        .set_character_stream_with_int_length(&mut connection, 2, Some(reader.clone()), 4)
        .unwrap();
    statement
        .set_url(
            &mut connection,
            3,
            Some(RdbcUrl::new("https://example.com/rbdc")),
        )
        .unwrap();
    statement
        .set_row_id(&mut connection, 4, Some(RdbcRowId::new(vec![8, 9])))
        .unwrap();
    assert_eq!(binary.read_to_end().unwrap(), vec![3]);
    assert_eq!(reader.read_to_string().unwrap(), "-reader");
    assert_eq!(
        statement
            .execute_update_bound(&mut connection)
            .await
            .unwrap()
            .rows_affected,
        1
    );

    let mut batch = connection
        .prepare_statement("INSERT_BATCH ?, ?")
        .await
        .unwrap();
    batch
        .set_blob_stream(
            &mut connection,
            1,
            Some(RdbcInputStream::from_bytes(vec![10, 11])),
        )
        .unwrap();
    batch
        .set_clob_reader(&mut connection, 2, Some(RdbcReader::from_string("first")))
        .unwrap();
    batch.add_bound_batch(&mut connection).unwrap();
    batch
        .set_binary_stream(
            &mut connection,
            1,
            Some(RdbcInputStream::from_bytes(vec![20, 21])),
        )
        .unwrap();
    batch
        .set_n_clob_reader(&mut connection, 2, Some(RdbcReader::from_string("第二")))
        .unwrap();
    batch.add_bound_batch(&mut connection).unwrap();
    assert_eq!(
        batch.execute_batch(&mut connection).await.unwrap(),
        vec![1, 1]
    );

    let mut query = connection
        .prepare_statement("QUERY_RESOURCE ?")
        .await
        .unwrap();
    query
        .set_binary_stream(
            &mut connection,
            1,
            Some(RdbcInputStream::from_bytes(vec![30, 31])),
        )
        .unwrap();
    let mut rows = query.execute_query_bound(&mut connection).await.unwrap();
    let meta_data = rows.meta_data(&mut connection).unwrap();
    assert_eq!(meta_data.column_count().unwrap(), 6);
    assert_eq!(meta_data.column_label(1).unwrap(), "column_0");
    assert_eq!(meta_data.column_label(6).unwrap(), "column_5");
    assert_eq!(rows.find_column(&mut connection, "COLUMN_2").unwrap(), 3);
    assert!(rows.next(&mut connection).unwrap());
    rows.close_with_connection(&mut connection).unwrap();

    let history = observed_history
        .lock()
        .expect("history lock poisoned")
        .clone();
    assert_eq!(
        history,
        vec![
            vec![
                rbs::Value::Binary(vec![1, 2]),
                rbs::Value::String("rbdc".to_string()),
                rbs::Value::String("https://example.com/rbdc".to_string()),
                rbs::Value::Binary(vec![8, 9]),
            ],
            vec![
                rbs::Value::Binary(vec![10, 11]),
                rbs::Value::String("first".to_string()),
            ],
            vec![
                rbs::Value::Binary(vec![20, 21]),
                rbs::Value::String("第二".to_string()),
            ],
            vec![rbs::Value::Binary(vec![30, 31])],
        ]
    );

    query.close_with_connection(&mut connection).unwrap();
    batch.close_with_connection(&mut connection).unwrap();
    statement.close_with_connection(&mut connection).unwrap();
    connection.close().await.unwrap();
}

fn assert_invalid_resource_setters(
    connection: &mut DruidPooledConnection,
    invalid: &mut DruidPooledPreparedStatement,
) {
    let oversized = i64::from(i32::MAX) + 1;
    assert!(matches!(
        invalid.set_blob(
            connection,
            1,
            Some(RdbcBlob::new(Arc::new(FixtureBlob {
                reported_length: oversized,
                bytes: Vec::new(),
            }))),
        ),
        Err(DruidError::InvalidArgument(_))
    ));
    assert!(matches!(
        invalid.set_clob(
            connection,
            2,
            Some(RdbcClob::new(Arc::new(FixtureClob {
                reported_length: oversized,
                value: JavaString::from(""),
            }))),
        ),
        Err(DruidError::InvalidArgument(_))
    ));
    assert!(matches!(
        invalid.set_n_clob(
            connection,
            3,
            Some(RdbcNClob::new(Arc::new(FixtureClob {
                reported_length: oversized,
                value: JavaString::from(""),
            }))),
        ),
        Err(DruidError::InvalidArgument(_))
    ));
    assert!(matches!(
        invalid.set_sql_xml(
            connection,
            4,
            Some(RdbcSqlXml::new(Arc::new(FixtureSqlXml {
                value: JavaString::from("<x/>"),
                fail_string: true,
            }))),
        ),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        invalid.set_sql_xml(
            connection,
            5,
            Some(RdbcSqlXml::new(Arc::new(FixtureSqlXml {
                value: JavaString::from_utf16([0xd800]),
                fail_string: false,
            }))),
        ),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        invalid.set_ascii_stream(connection, 1, Some(RdbcInputStream::from_bytes([0xff])),),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        invalid.set_ref(connection, 1, Some(RdbcRef::new(Arc::new(FixtureRef))),),
        Err(DruidError::UnsupportedOperation { .. })
    ));
    assert!(matches!(
        invalid.set_array(connection, 1, Some(RdbcArray::new(Arc::new(FixtureArray))),),
        Err(DruidError::UnsupportedOperation { .. })
    ));
}

fn bind_valid_resource_objects(
    connection: &mut DruidPooledConnection,
    statement: &mut DruidPooledPreparedStatement,
) {
    let blob = RdbcBlob::new(Arc::new(FixtureBlob {
        reported_length: 2,
        bytes: vec![40, 41],
    }));
    let clob = RdbcClob::new(Arc::new(FixtureClob {
        reported_length: 4,
        value: JavaString::from("clob"),
    }));
    let n_clob = RdbcNClob::new(Arc::new(FixtureClob {
        reported_length: 1,
        value: JavaString::from("国"),
    }));
    let sql_xml = RdbcSqlXml::new(Arc::new(FixtureSqlXml {
        value: JavaString::from("<x/>"),
        fail_string: false,
    }));

    statement
        .set_blob(connection, 1, Some(blob.clone()))
        .unwrap();
    statement
        .set_clob(connection, 2, Some(clob.clone()))
        .unwrap();
    statement
        .set_n_clob(connection, 3, Some(n_clob.clone()))
        .unwrap();
    statement
        .set_sql_xml(connection, 4, Some(sql_xml.clone()))
        .unwrap();
    statement
        .set_object(
            connection,
            5,
            Some(RdbcObject::RowId(RdbcRowId::new([1, 2]))),
        )
        .unwrap();
    statement
        .set_object(connection, 6, Some(RdbcObject::SqlXml(sql_xml)))
        .unwrap();
    statement
        .set_object(connection, 7, Some(RdbcObject::Blob(blob)))
        .unwrap();
    statement
        .set_object(connection, 8, Some(RdbcObject::Clob(clob)))
        .unwrap();
    statement
        .set_object(connection, 9, Some(RdbcObject::NClob(n_clob)))
        .unwrap();
    statement
        .set_object(
            connection,
            10,
            Some(RdbcObject::CharacterStream(RdbcReader::from_string(
                "reader",
            ))),
        )
        .unwrap();
    statement
        .set_object(
            connection,
            11,
            Some(RdbcObject::NCharacterStream(RdbcReader::from_string(
                "国字",
            ))),
        )
        .unwrap();
    statement
        .set_object(
            connection,
            12,
            Some(RdbcObject::String("scalar".to_string())),
        )
        .unwrap();
}

#[tokio::test]
async fn rbdc_prepared_lob_sqlxml_and_object_resources_materialize_at_setter_boundary() {
    let observed_params = Arc::new(Mutex::new(Vec::new()));
    let observed_history = Arc::new(Mutex::new(Vec::new()));
    let factory = factory_with_history(observed_params, observed_history.clone());
    let physical = factory.create().await.expect("factory create must succeed");
    let mut connection = DruidPooledConnection::new(physical, 701, Box::new(|_, _| {}));

    let mut invalid = connection
        .prepare_statement("INVALID_LOB ?, ?, ?, ?, ?")
        .await
        .unwrap();
    assert_invalid_resource_setters(&mut connection, &mut invalid);
    invalid.close_with_connection(&mut connection).unwrap();

    let mut statement = connection
        .prepare_statement("INSERT_OBJECTS ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?")
        .await
        .unwrap();
    bind_valid_resource_objects(&mut connection, &mut statement);

    assert_eq!(
        statement
            .execute_update_bound(&mut connection)
            .await
            .unwrap()
            .rows_affected,
        1
    );
    assert_eq!(
        observed_history
            .lock()
            .expect("history lock poisoned")
            .as_slice(),
        &[vec![
            rbs::Value::Binary(vec![40, 41]),
            rbs::Value::String("clob".to_string()),
            rbs::Value::String("国".to_string()),
            rbs::Value::String("<x/>".to_string()),
            rbs::Value::Binary(vec![1, 2]),
            rbs::Value::String("<x/>".to_string()),
            rbs::Value::Binary(vec![40, 41]),
            rbs::Value::String("clob".to_string()),
            rbs::Value::String("国".to_string()),
            rbs::Value::String("reader".to_string()),
            rbs::Value::String("国字".to_string()),
            rbs::Value::String("scalar".to_string()),
        ]]
    );

    statement.close_with_connection(&mut connection).unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn rbdc_driver_failure_preserves_the_publicly_available_structure() {
    let factory = factory(Arc::new(Mutex::new(Vec::new())));
    let mut connection = factory.create().await.expect("factory create must succeed");
    let error = connection
        .exec("FAIL", Vec::new())
        .await
        .expect_err("fake RBDC driver must return its database error");

    let DruidError::SqlException(exception) = error else {
        panic!("RBDC database failure 必须映射为结构化 SqlException");
    };
    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(exception.class_name(), "rbdc::Error");
    assert_eq!(exception.message(), Some("vendor database failure"));
}
