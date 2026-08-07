//! Java Druid PreparedStatement pool 纵向契约。
//!
//! Java oracle：
//! - `DruidConnectionHolderTest4#test_toString`
//! - `PSCacheTest3#test_pscache`
//! - `DruidDataSourceTest_clearCache#test_clearStatementCache`

use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    AfterFilter, BatchExecContext, BeforeFilter, DruidError, DruidPooledConnection,
    DruidPooledPreparedStatement, DruidPooledPreparedStatementHandle, ExecContext, ExecOperation,
    ExecResult, FilterChain, JdbcCalendar, JdbcCalendarArgument, JdbcCharacterLength,
    JdbcInputStream, JdbcObject, JdbcReader, JdbcRowId, JdbcStreamLength, JdbcUrl,
    PhysicalConnection, PhysicalConnectionFactory, PhysicalPreparedStatement,
    PreparedInputParameter, PreparedStatementKey, PreparedTypeNameArgument, ResultSetStatement,
    Row, SqlTextPreparedStatement, StatementExecuteResult, StatementGeneratedKeys, Value, Wrapper,
    WrapperExt,
};
use druid::pool::DruidPool;
use druid::toasty::{ToastyConnectionFactory, ToastyPreparedStatement};
use std::any::TypeId;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct RejectNextPreparedBatch {
    reject_next: AtomicBool,
}

#[async_trait::async_trait]
impl BeforeFilter for RejectNextPreparedBatch {
    fn name(&self) -> &str {
        "reject_next_prepared_batch"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        if context.operation == ExecOperation::Batch
            && self.reject_next.swap(false, Ordering::AcqRel)
        {
            Err(DruidError::Other(
                "prepared batch rejected before physical execution".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

struct UnsupportedPreparedUnwrapType;

#[derive(Default)]
struct PreparedDescriptorRecorder {
    parameter_sets: Mutex<Vec<Vec<PreparedInputParameter>>>,
    batch_parameter_sets: Mutex<Vec<Vec<Vec<PreparedInputParameter>>>>,
}

#[async_trait::async_trait]
impl BeforeFilter for PreparedDescriptorRecorder {
    fn name(&self) -> &str {
        "prepared_descriptor_recorder"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        if let Some(parameters) = context.prepared_parameters {
            self.parameter_sets
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(parameters.to_vec());
        }
        Ok(())
    }

    async fn before_batch(&self, context: &mut BatchExecContext<'_>) -> Result<(), DruidError> {
        if let Some(parameter_sets) = context.prepared_parameter_sets {
            self.batch_parameter_sets
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(parameter_sets.to_vec());
        }
        Ok(())
    }
}

struct PreparedConnection {
    prepare_count: Arc<AtomicU64>,
    schema: String,
    closed: bool,
}

#[async_trait::async_trait]
impl PhysicalConnection for PreparedConnection {
    async fn exec(&mut self, sql: &str, _params: Vec<Value>) -> Result<ExecResult, DruidError> {
        if sql == "FAIL" {
            return Err(DruidError::DriverError("expected failure".to_string()));
        }
        Ok(ExecResult {
            rows_affected: 1,
            last_insert_id: None,
            row_count: None,
        })
    }

    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(vec![Row::new(vec![Value::Int(1)])])
    }

    async fn execute(
        &mut self,
        sql: &str,
        _params: Vec<Value>,
        _generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        match sql {
            "MULTI" => Ok(vec![
                StatementExecuteResult::ResultSet(vec![Row::new(vec![Value::Int(10)])]),
                StatementExecuteResult::Update(ExecResult {
                    rows_affected: 2,
                    last_insert_id: Some(99),
                    row_count: None,
                }),
                StatementExecuteResult::ResultSet(vec![Row::new(vec![Value::Int(20)])]),
            ]),
            "FAIL" => Err(DruidError::DriverError("expected failure".to_string())),
            _ => Ok(vec![StatementExecuteResult::Update(ExecResult {
                rows_affected: 1,
                last_insert_id: None,
                row_count: None,
            })]),
        }
    }

    async fn prepare_physical_statement(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        self.prepare_count.fetch_add(1, Ordering::Relaxed);
        Ok(Arc::new(SqlTextPreparedStatement::new(key.sql())))
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed
    }

    fn schema(&self) -> Option<&str> {
        Some(&self.schema)
    }

    async fn set_schema(&mut self, schema: &str) -> Result<(), DruidError> {
        self.schema = schema.to_string();
        Ok(())
    }
}

#[derive(Default)]
struct PreparedExecuteRecorder {
    events: Mutex<Vec<PreparedExecuteEvent>>,
}

type PreparedExecuteEvent = (String, ExecOperation, Vec<Value>, bool, Option<u64>);

#[async_trait::async_trait]
impl BeforeFilter for PreparedExecuteRecorder {
    fn name(&self) -> &str {
        "prepared_execute_recorder"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((
                context.sql.to_string(),
                context.operation,
                context.params.to_vec(),
                true,
                None,
            ));
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for PreparedExecuteRecorder {
    fn name(&self) -> &str {
        "prepared_execute_recorder"
    }

    async fn after(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((
                context.sql.to_string(),
                context.operation,
                context.params.to_vec(),
                false,
                result
                    .as_ref()
                    .ok()
                    .map(|execution| execution.row_count.unwrap_or(execution.rows_affected)),
            ));
        Ok(())
    }
}

struct PreparedFactory {
    prepare_count: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl druid::core::PhysicalConnectionFactory for PreparedFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        Ok(Box::new(PreparedConnection {
            prepare_count: self.prepare_count.clone(),
            schema: "main".to_string(),
            closed: false,
        }))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }

    async fn close(&self, connection: &mut Box<dyn PhysicalConnection>) -> Result<(), DruidError> {
        connection.close().await
    }
}

#[derive(Clone, Copy)]
struct PreparedPropertyValues {
    max_field_size: i32,
    max_rows: i32,
    query_timeout: i32,
    fetch_direction: i32,
    fetch_size: i32,
}

struct PreparedPropertyProbe {
    values: Mutex<PreparedPropertyValues>,
    events: Mutex<Vec<String>>,
    fail_restore_max_rows_once: AtomicBool,
    closed: AtomicBool,
}

impl PreparedPropertyProbe {
    fn new(fail_restore_max_rows_once: bool) -> Self {
        Self {
            values: Mutex::new(PreparedPropertyValues {
                max_field_size: 11,
                max_rows: 12,
                query_timeout: 13,
                fetch_direction: 1001,
                fetch_size: 14,
            }),
            events: Mutex::new(Vec::new()),
            fail_restore_max_rows_once: AtomicBool::new(fail_restore_max_rows_once),
            closed: AtomicBool::new(false),
        }
    }

    fn record(&self, event: impl Into<String>) {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(event.into());
    }

    fn events(&self) -> Vec<String> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

struct PreparedPropertyStatement {
    probe: Arc<PreparedPropertyProbe>,
}

impl PhysicalPreparedStatement for PreparedPropertyStatement {
    fn sql(&self) -> &str {
        "select 1"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn max_field_size(&self) -> Result<i32, DruidError> {
        self.probe.record("get_max_field_size");
        Ok(self
            .probe
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .max_field_size)
    }

    fn set_max_field_size(&self, max: i32) -> Result<(), DruidError> {
        self.probe.record(format!("set_max_field_size:{max}"));
        self.probe
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .max_field_size = max;
        Ok(())
    }

    fn max_rows(&self) -> Result<i32, DruidError> {
        self.probe.record("get_max_rows");
        Ok(self
            .probe
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .max_rows)
    }

    fn set_max_rows(&self, max: i32) -> Result<(), DruidError> {
        self.probe.record(format!("set_max_rows:{max}"));
        if max == 12
            && self
                .probe
                .fail_restore_max_rows_once
                .swap(false, Ordering::AcqRel)
        {
            return Err(DruidError::DriverError(
                "restore max rows failed".to_string(),
            ));
        }
        self.probe
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .max_rows = max;
        Ok(())
    }

    fn query_timeout(&self) -> Result<i32, DruidError> {
        self.probe.record("get_query_timeout");
        Ok(self
            .probe
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .query_timeout)
    }

    fn set_query_timeout(&self, seconds: i32) -> Result<(), DruidError> {
        self.probe.record(format!("set_query_timeout:{seconds}"));
        self.probe
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .query_timeout = seconds;
        Ok(())
    }

    fn fetch_direction(&self) -> Result<i32, DruidError> {
        self.probe.record("get_fetch_direction");
        Ok(self
            .probe
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fetch_direction)
    }

    fn set_fetch_direction(&self, direction: i32) -> Result<(), DruidError> {
        self.probe
            .record(format!("set_fetch_direction:{direction}"));
        self.probe
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fetch_direction = direction;
        Ok(())
    }

    fn fetch_size(&self) -> Result<i32, DruidError> {
        self.probe.record("get_fetch_size");
        Ok(self
            .probe
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fetch_size)
    }

    fn set_fetch_size(&self, rows: i32) -> Result<(), DruidError> {
        self.probe.record(format!("set_fetch_size:{rows}"));
        self.probe
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fetch_size = rows;
        Ok(())
    }

    fn clear_parameters(&self) -> Result<(), DruidError> {
        self.probe.record("clear_parameters");
        Ok(())
    }

    fn clear_batch(&self) -> Result<(), DruidError> {
        self.probe.record("clear_batch");
        Ok(())
    }

    fn close(&self) -> Result<(), DruidError> {
        self.probe.record("close");
        self.probe.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.probe.closed.load(Ordering::Acquire)
    }
}

struct PreparedPropertyConnection {
    probe: Arc<PreparedPropertyProbe>,
    prepare_count: Arc<AtomicU64>,
    closed: bool,
}

#[async_trait::async_trait]
impl PhysicalConnection for PreparedPropertyConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, DruidError> {
        Ok(ExecResult::default())
    }

    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(Vec::new())
    }

    async fn prepare_physical_statement(
        &mut self,
        _key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        self.prepare_count.fetch_add(1, Ordering::Relaxed);
        Ok(Arc::new(PreparedPropertyStatement {
            probe: Arc::clone(&self.probe),
        }))
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed
    }
}

struct PreparedPropertyFactory {
    probe: Arc<PreparedPropertyProbe>,
    prepare_count: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for PreparedPropertyFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        Ok(Box::new(PreparedPropertyConnection {
            probe: Arc::clone(&self.probe),
            prepare_count: Arc::clone(&self.prepare_count),
            closed: false,
        }))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }
}

async fn prepared_pool(max_statements: usize) -> (DruidPool, Arc<AtomicU64>) {
    let prepare_count = Arc::new(AtomicU64::new(0));
    let pool = DruidPool::builder()
        .name("prepared")
        .db_type_name("mysql")
        .factory(Arc::new(PreparedFactory {
            prepare_count: prepare_count.clone(),
        }))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(max_statements)
        .build()
        .await
        .unwrap();
    (pool, prepare_count)
}

#[tokio::test]
async fn prepared_close_restores_properties_in_java_order_and_stops_on_first_error() {
    let probe = Arc::new(PreparedPropertyProbe::new(true));
    let prepare_count = Arc::new(AtomicU64::new(0));
    let pool = DruidPool::builder()
        .name("prepared-property-order")
        .factory(Arc::new(PreparedPropertyFactory {
            probe: Arc::clone(&probe),
            prepare_count: Arc::clone(&prepare_count),
        }))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection.prepare_statement("select 1").await.unwrap();
    assert_eq!(
        probe.events(),
        [
            "get_max_field_size",
            "get_max_rows",
            "get_query_timeout",
            "get_fetch_direction",
            "get_fetch_size",
        ]
    );

    statement.set_max_field_size(&mut connection, 21).unwrap();
    statement.set_max_rows(&mut connection, 22).unwrap();
    statement.set_query_timeout(&mut connection, 23).unwrap();
    statement
        .set_fetch_direction(&mut connection, 1000)
        .unwrap();
    statement.set_fetch_size(&mut connection, 24).unwrap();

    let error = statement
        .close_with_connection(&mut connection)
        .expect_err("Java close 必须在首个属性恢复错误处停止");
    assert_eq!(
        error,
        DruidError::DriverError("restore max rows failed".to_string())
    );
    assert!(!statement.is_closed());
    assert_eq!(
        &probe.events()[10..],
        ["set_max_field_size:11", "set_max_rows:12"]
    );

    statement.close_with_connection(&mut connection).unwrap();
    assert!(statement.is_closed());
    assert_eq!(
        &probe.events()[12..],
        [
            "set_max_rows:12",
            "set_query_timeout:13",
            "set_fetch_direction:1001",
            "set_fetch_size:14",
            "clear_parameters",
            "clear_batch",
            "close",
        ]
    );
    assert_eq!(prepare_count.load(Ordering::Acquire), 1);

    // 首次恢复错误已经增加 exceptionCount，所以成功重试后也不得把该句柄放回缓存。
    let mut replacement = connection.prepare_statement("select 1").await.unwrap();
    assert_eq!(prepare_count.load(Ordering::Acquire), 2);
    replacement.close_with_connection(&mut connection).unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn pooled_prepared_statement_forwards_statement_properties_and_restores_real_sqlite_handle() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let pool = DruidPool::builder()
        .name("prepared-properties-sqlite")
        .driver_name("toasty-sqlite")
        .factory(Arc::new(factory))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .expect("真实 SQLite PreparedStatement 属性池必须创建成功");
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .prepare_statement_with_holdability("SELECT 1", 1004, 1008, 2)
        .await
        .unwrap();

    assert_eq!(statement.result_set_type(&connection).unwrap(), 1004);
    assert_eq!(statement.result_set_concurrency(&connection).unwrap(), 1008);
    assert_eq!(statement.result_set_holdability(&connection).unwrap(), 2);
    assert_eq!(statement.max_field_size(&mut connection).unwrap(), 0);
    assert_eq!(statement.max_rows(&mut connection).unwrap(), 0);
    assert_eq!(statement.query_timeout(&mut connection).unwrap(), 0);
    assert_eq!(statement.fetch_direction(&mut connection).unwrap(), 1000);
    assert_eq!(statement.fetch_size(&mut connection).unwrap(), 0);

    statement.set_max_field_size(&mut connection, 101).unwrap();
    statement.set_max_rows(&mut connection, 102).unwrap();
    statement.set_query_timeout(&mut connection, 103).unwrap();
    statement
        .set_fetch_direction(&mut connection, 1001)
        .unwrap();
    statement.set_fetch_size(&mut connection, 104).unwrap();
    statement
        .set_escape_processing(&mut connection, false)
        .unwrap();
    statement
        .set_cursor_name(&mut connection, "cursor")
        .unwrap();
    statement.cancel(&mut connection).unwrap();
    statement.set_poolable(&mut connection, true).unwrap();
    assert!(!statement.is_poolable());
    assert_eq!(
        statement.set_poolable(&mut connection, false).unwrap_err(),
        DruidError::UnsupportedOperation {
            operation: "statement_set_poolable_false"
        }
    );
    statement.close_on_completion(&mut connection).unwrap();
    assert!(statement.is_close_on_completion(&mut connection).unwrap());
    assert_eq!(statement.max_field_size(&mut connection).unwrap(), 101);
    assert_eq!(statement.max_rows(&mut connection).unwrap(), 102);
    assert_eq!(statement.query_timeout(&mut connection).unwrap(), 103);
    assert_eq!(statement.fetch_direction(&mut connection).unwrap(), 1001);
    assert_eq!(statement.fetch_size(&mut connection).unwrap(), 104);
    statement.close_with_connection(&mut connection).unwrap();

    let mut reused = connection
        .prepare_statement_with_holdability("SELECT 1", 1004, 1008, 2)
        .await
        .unwrap();
    assert_eq!(reused.max_field_size(&mut connection).unwrap(), 0);
    assert_eq!(reused.max_rows(&mut connection).unwrap(), 0);
    assert_eq!(reused.query_timeout(&mut connection).unwrap(), 0);
    assert_eq!(reused.fetch_direction(&mut connection).unwrap(), 1000);
    assert_eq!(reused.fetch_size(&mut connection).unwrap(), 0);
    let rows = reused.fetch(&mut connection, Vec::new()).await.unwrap();
    assert_eq!(rows, vec![Row::new(vec![Value::Int(1)])]);
    reused.close_with_connection(&mut connection).unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn prepared_setter_overloads_preserve_exact_descriptor_identity() {
    let (pool, _) = prepared_pool(4).await;
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection.prepare_statement("select 1").await.unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_nano_opt(11, 12, 13, 456_789_000).unwrap();
    let timestamp = NaiveDateTime::new(date, time);
    let calendar = JdbcCalendar::new("Asia/Shanghai").unwrap();
    let ascii = JdbcInputStream::from_bytes(b"ascii".to_vec());
    let binary = JdbcInputStream::from_bytes(vec![0, 1, 2]);
    let unicode = JdbcInputStream::from_bytes(b"unicode".to_vec());
    let blob_stream = JdbcInputStream::from_bytes(vec![3, 4, 5]);
    let character = JdbcReader::from_string("character");
    let national = JdbcReader::from_string("国家字符");
    let clob_reader = JdbcReader::from_string("clob");
    let n_clob_reader = JdbcReader::from_string("nclob");

    statement.set_null(&mut connection, 1, 4).unwrap();
    statement
        .set_null_with_type_name(&mut connection, 2, 2002, None)
        .unwrap();
    statement.set_boolean(&mut connection, 3, true).unwrap();
    statement.set_byte(&mut connection, 4, -8).unwrap();
    statement.set_short(&mut connection, 5, 16).unwrap();
    statement.set_int(&mut connection, 6, 32).unwrap();
    statement.set_long(&mut connection, 7, 64).unwrap();
    statement.set_float(&mut connection, 8, 1.25).unwrap();
    statement.set_double(&mut connection, 9, 2.5).unwrap();
    statement
        .set_big_decimal(
            &mut connection,
            10,
            Some(BigDecimal::from_str("123.4500").unwrap()),
        )
        .unwrap();
    statement
        .set_string(&mut connection, 11, Some("text".to_string()))
        .unwrap();
    statement
        .set_n_string(&mut connection, 12, Some("国字".to_string()))
        .unwrap();
    statement
        .set_bytes(&mut connection, 13, Some(vec![8, 9]))
        .unwrap();
    statement.set_date(&mut connection, 14, Some(date)).unwrap();
    statement
        .set_date_with_calendar(&mut connection, 15, Some(date), Some(calendar.clone()))
        .unwrap();
    statement.set_time(&mut connection, 16, Some(time)).unwrap();
    statement
        .set_time_with_calendar(&mut connection, 17, Some(time), None)
        .unwrap();
    statement
        .set_timestamp(&mut connection, 18, Some(timestamp))
        .unwrap();
    statement
        .set_timestamp_with_calendar(&mut connection, 19, Some(timestamp), Some(calendar.clone()))
        .unwrap();
    statement
        .set_object(
            &mut connection,
            20,
            Some(JdbcObject::String("object".to_string())),
        )
        .unwrap();
    statement
        .set_object_with_sql_type(&mut connection, 21, Some(JdbcObject::Integer(21)), 4)
        .unwrap();
    statement
        .set_object_with_sql_type_and_scale(
            &mut connection,
            22,
            Some(JdbcObject::BigDecimal(
                BigDecimal::from_str("22.50").unwrap(),
            )),
            3,
            2,
        )
        .unwrap();
    statement
        .set_ascii_stream(&mut connection, 23, Some(ascii.clone()))
        .unwrap();
    statement
        .set_ascii_stream_with_int_length(&mut connection, 24, None, -1)
        .unwrap();
    statement
        .set_ascii_stream_with_long_length(&mut connection, 25, None, i64::MAX)
        .unwrap();
    statement
        .set_unicode_stream(&mut connection, 26, Some(unicode.clone()), 7)
        .unwrap();
    statement
        .set_binary_stream(&mut connection, 27, Some(binary.clone()))
        .unwrap();
    statement
        .set_binary_stream_with_int_length(&mut connection, 28, None, -2)
        .unwrap();
    statement
        .set_binary_stream_with_long_length(&mut connection, 29, None, 29)
        .unwrap();
    statement
        .set_character_stream(&mut connection, 30, Some(character.clone()))
        .unwrap();
    statement
        .set_character_stream_with_int_length(&mut connection, 31, None, -3)
        .unwrap();
    statement
        .set_character_stream_with_long_length(&mut connection, 32, None, 32)
        .unwrap();
    statement
        .set_n_character_stream(&mut connection, 33, Some(national.clone()))
        .unwrap();
    statement
        .set_n_character_stream_with_long_length(&mut connection, 34, None, 34)
        .unwrap();
    statement.set_ref(&mut connection, 35, None).unwrap();
    statement.set_blob(&mut connection, 36, None).unwrap();
    statement
        .set_blob_stream(&mut connection, 37, Some(blob_stream.clone()))
        .unwrap();
    statement
        .set_blob_stream_with_long_length(&mut connection, 38, None, 38)
        .unwrap();
    statement.set_clob(&mut connection, 39, None).unwrap();
    statement
        .set_clob_reader(&mut connection, 40, Some(clob_reader.clone()))
        .unwrap();
    statement
        .set_clob_reader_with_long_length(&mut connection, 41, None, 41)
        .unwrap();
    statement.set_n_clob(&mut connection, 42, None).unwrap();
    statement
        .set_n_clob_reader(&mut connection, 43, Some(n_clob_reader.clone()))
        .unwrap();
    statement
        .set_n_clob_reader_with_long_length(&mut connection, 44, None, 44)
        .unwrap();
    statement.set_array(&mut connection, 45, None).unwrap();
    statement.set_url(&mut connection, 46, None).unwrap();
    statement.set_row_id(&mut connection, 47, None).unwrap();
    statement.set_sql_xml(&mut connection, 48, None).unwrap();

    assert_eq!(statement.parameter_slot_count(), 48);
    assert_eq!(
        statement.parameter(1),
        Some(PreparedInputParameter::Null {
            sql_type: 4,
            type_name: PreparedTypeNameArgument::Unspecified,
        })
    );
    assert_eq!(
        statement.parameter(2),
        Some(PreparedInputParameter::Null {
            sql_type: 2002,
            type_name: PreparedTypeNameArgument::Specified(None),
        })
    );
    assert_eq!(
        statement.parameter(15),
        Some(PreparedInputParameter::Date {
            value: Some(date),
            calendar: JdbcCalendarArgument::Specified(Some(calendar)),
        })
    );
    assert!(matches!(
        statement.parameter(22),
        Some(PreparedInputParameter::Object {
            target_sql_type: Some(3),
            scale_or_length: Some(2),
            ..
        })
    ));
    assert!(matches!(
        statement.parameter(24),
        Some(PreparedInputParameter::AsciiStream {
            length: JdbcStreamLength::Int(-1),
            ..
        })
    ));
    assert!(matches!(
        statement.parameter(31),
        Some(PreparedInputParameter::CharacterStream {
            length: JdbcCharacterLength::Int(-3),
            ..
        })
    ));
    assert!(matches!(
        statement.parameter(44),
        Some(PreparedInputParameter::NClobReader {
            length: JdbcCharacterLength::Long(44),
            ..
        })
    ));
    assert_eq!(ascii.read_to_end().unwrap(), b"ascii");
    assert_eq!(binary.read_to_end().unwrap(), vec![0, 1, 2]);
    assert_eq!(unicode.read_to_end().unwrap(), b"unicode");
    assert_eq!(blob_stream.read_to_end().unwrap(), vec![3, 4, 5]);
    assert_eq!(character.read_to_string().unwrap(), "character");
    assert_eq!(national.read_to_string().unwrap(), "国家字符");
    assert_eq!(clob_reader.read_to_string().unwrap(), "clob");
    assert_eq!(n_clob_reader.read_to_string().unwrap(), "nclob");

    statement.clear_parameters(&mut connection).unwrap();
    assert_eq!(statement.parameter_slot_count(), 0);
    assert!(statement.parameter(1).is_none());
    let error = statement.set_int(&mut connection, 0, 1).unwrap_err();
    assert!(matches!(error, DruidError::InvalidArgument(_)));
    assert_eq!(statement.parameter_slot_count(), 0);

    drop(statement);
    connection.close().await.unwrap();
    pool.close().await;
}

#[test]
fn prepared_input_parameter_converts_every_scalar_family_without_losing_type() {
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_nano_opt(1, 2, 3, 4).unwrap();
    let timestamp = NaiveDateTime::new(date, time);
    let decimal = BigDecimal::from_str("123.4500").unwrap();
    let url = JdbcUrl::new("https://example.com/path");
    let scalar_cases = vec![
        (PreparedInputParameter::null(4), Value::Null),
        (
            PreparedInputParameter::null_with_type_name(2002, None),
            Value::Null,
        ),
        (PreparedInputParameter::Boolean(true), Value::Bool(true)),
        (PreparedInputParameter::Byte(-8), Value::Int(-8)),
        (PreparedInputParameter::Short(16), Value::Int(16)),
        (PreparedInputParameter::Int(32), Value::Int(32)),
        (PreparedInputParameter::Long(64), Value::Int(64)),
        (PreparedInputParameter::Float(1.25), Value::Float(1.25)),
        (PreparedInputParameter::Double(2.5), Value::Float(2.5)),
        (
            PreparedInputParameter::BigDecimal(Some(decimal.clone())),
            Value::Decimal(decimal.clone()),
        ),
        (PreparedInputParameter::BigDecimal(None), Value::Null),
        (
            PreparedInputParameter::String(Some("text".to_string())),
            Value::String("text".to_string()),
        ),
        (PreparedInputParameter::String(None), Value::Null),
        (
            PreparedInputParameter::NString(Some("国家字符".to_string())),
            Value::String("国家字符".to_string()),
        ),
        (PreparedInputParameter::NString(None), Value::Null),
        (
            PreparedInputParameter::Bytes(Some(vec![0, 1, 2])),
            Value::Bytes(vec![0, 1, 2]),
        ),
        (PreparedInputParameter::Bytes(None), Value::Null),
        (
            PreparedInputParameter::Date {
                value: Some(date),
                calendar: JdbcCalendarArgument::Unspecified,
            },
            Value::Date(date),
        ),
        (
            PreparedInputParameter::Date {
                value: None,
                calendar: JdbcCalendarArgument::Unspecified,
            },
            Value::Null,
        ),
        (
            PreparedInputParameter::Time {
                value: Some(time),
                calendar: JdbcCalendarArgument::Unspecified,
            },
            Value::Time(time),
        ),
        (
            PreparedInputParameter::Time {
                value: None,
                calendar: JdbcCalendarArgument::Unspecified,
            },
            Value::Null,
        ),
        (
            PreparedInputParameter::Timestamp {
                value: Some(timestamp),
                calendar: JdbcCalendarArgument::Unspecified,
            },
            Value::Timestamp(timestamp),
        ),
        (
            PreparedInputParameter::Timestamp {
                value: None,
                calendar: JdbcCalendarArgument::Unspecified,
            },
            Value::Null,
        ),
        (PreparedInputParameter::object(None), Value::Null),
        (
            PreparedInputParameter::object(Some(JdbcObject::Scalar(Value::Int(1)))),
            Value::Int(1),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::String("s".to_string()))),
            Value::String("s".to_string()),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::NString("n".to_string()))),
            Value::String("n".to_string()),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::Boolean(true))),
            Value::Bool(true),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::Byte(-1))),
            Value::Int(-1),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::Short(2))),
            Value::Int(2),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::Integer(3))),
            Value::Int(3),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::Long(4))),
            Value::Int(4),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::Float(5.5))),
            Value::Float(5.5),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::Double(6.5))),
            Value::Float(6.5),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::Bytes(vec![7]))),
            Value::Bytes(vec![7]),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::BigDecimal(decimal.clone()))),
            Value::Decimal(decimal),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::Date(date))),
            Value::Date(date),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::Time(time))),
            Value::Time(time),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::Timestamp(timestamp))),
            Value::Timestamp(timestamp),
        ),
        (
            PreparedInputParameter::object(Some(JdbcObject::Url(url.clone()))),
            Value::String("https://example.com/path".to_string()),
        ),
        (
            PreparedInputParameter::Url(Some(url)),
            Value::String("https://example.com/path".to_string()),
        ),
        (PreparedInputParameter::Url(None), Value::Null),
        (PreparedInputParameter::Ref(None), Value::Null),
        (PreparedInputParameter::Array(None), Value::Null),
        (PreparedInputParameter::Blob(None), Value::Null),
        (PreparedInputParameter::Clob(None), Value::Null),
        (PreparedInputParameter::NClob(None), Value::Null),
        (PreparedInputParameter::RowId(None), Value::Null),
        (PreparedInputParameter::SqlXml(None), Value::Null),
    ];
    for (parameter, expected) in scalar_cases {
        assert_eq!(parameter.scalar_value().unwrap(), expected);
    }

    let typed = PreparedInputParameter::object_with_sql_type(Some(JdbcObject::Integer(8)), 4);
    assert_eq!(typed.scalar_value().unwrap(), Value::Int(8));
    let scaled = PreparedInputParameter::object_with_sql_type_and_scale(
        Some(JdbcObject::BigDecimal(BigDecimal::from(9))),
        3,
        2,
    );
    assert_eq!(
        scaled.scalar_value().unwrap(),
        Value::Decimal(BigDecimal::from(9))
    );

    let reader = JdbcReader::from_string("native-only");
    let object_error = PreparedInputParameter::object(Some(JdbcObject::CharacterStream(reader)))
        .scalar_value()
        .unwrap_err();
    assert_eq!(
        object_error,
        DruidError::UnsupportedOperation {
            operation: "prepared_object_requires_native_adapter"
        }
    );
    let stream_error = PreparedInputParameter::AsciiStream {
        stream: None,
        length: JdbcStreamLength::Unspecified,
    }
    .scalar_value()
    .unwrap_err();
    assert_eq!(
        stream_error,
        DruidError::UnsupportedOperation {
            operation: "prepared_parameter_requires_native_adapter"
        }
    );
}

#[derive(Clone, Copy)]
enum CleanupFailure {
    SetParameter,
    AddBatch,
    ClearParameters,
    ClearBatch,
    GetResultSet,
    GetUpdateCount,
    GetGeneratedKeys,
    GetMoreResults,
}

struct CleanupFailingStatement {
    failure: CleanupFailure,
    closed: AtomicBool,
}

impl CleanupFailingStatement {
    fn fatal_error() -> DruidError {
        DruidError::SqlException(Box::new(
            druid::core::SqlException::driver(1040, "too many connections").with_class_name(
                "com.mysql.cj.jdbc.exceptions.MySQLNonTransientConnectionException",
            ),
        ))
    }
}

impl PhysicalPreparedStatement for CleanupFailingStatement {
    fn sql(&self) -> &str {
        "select 1"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_parameter(
        &self,
        _parameter_index: usize,
        _parameter: &PreparedInputParameter,
    ) -> Result<(), DruidError> {
        match self.failure {
            CleanupFailure::SetParameter => Err(Self::fatal_error()),
            _ => Ok(()),
        }
    }

    fn clear_parameters(&self) -> Result<(), DruidError> {
        match self.failure {
            CleanupFailure::ClearParameters => Err(Self::fatal_error()),
            CleanupFailure::SetParameter
            | CleanupFailure::AddBatch
            | CleanupFailure::ClearBatch
            | CleanupFailure::GetResultSet
            | CleanupFailure::GetUpdateCount
            | CleanupFailure::GetGeneratedKeys
            | CleanupFailure::GetMoreResults => Ok(()),
        }
    }

    fn add_batch(&self, _params: &[Value]) -> Result<(), DruidError> {
        match self.failure {
            CleanupFailure::AddBatch => Err(Self::fatal_error()),
            CleanupFailure::SetParameter
            | CleanupFailure::ClearParameters
            | CleanupFailure::ClearBatch
            | CleanupFailure::GetResultSet
            | CleanupFailure::GetUpdateCount
            | CleanupFailure::GetGeneratedKeys
            | CleanupFailure::GetMoreResults => Ok(()),
        }
    }

    fn clear_batch(&self) -> Result<(), DruidError> {
        match self.failure {
            CleanupFailure::SetParameter
            | CleanupFailure::AddBatch
            | CleanupFailure::ClearParameters
            | CleanupFailure::GetResultSet
            | CleanupFailure::GetUpdateCount
            | CleanupFailure::GetGeneratedKeys
            | CleanupFailure::GetMoreResults => Ok(()),
            CleanupFailure::ClearBatch => Err(Self::fatal_error()),
        }
    }

    fn get_result_set(&self) -> Result<(), DruidError> {
        match self.failure {
            CleanupFailure::GetResultSet => Err(Self::fatal_error()),
            _ => Ok(()),
        }
    }

    fn get_update_count(&self) -> Result<(), DruidError> {
        match self.failure {
            CleanupFailure::GetUpdateCount => Err(Self::fatal_error()),
            _ => Ok(()),
        }
    }

    fn get_generated_keys(&self) -> Result<(), DruidError> {
        match self.failure {
            CleanupFailure::GetGeneratedKeys => Err(Self::fatal_error()),
            _ => Ok(()),
        }
    }

    fn get_more_results(&self, _current: Option<i32>) -> Result<(), DruidError> {
        match self.failure {
            CleanupFailure::GetMoreResults => Err(Self::fatal_error()),
            _ => Ok(()),
        }
    }

    fn close(&self) -> Result<(), DruidError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

struct CleanupFailingConnection {
    failure: CleanupFailure,
    discarded: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl PhysicalConnection for CleanupFailingConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, DruidError> {
        Ok(ExecResult::default())
    }

    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(vec![Row::new(vec![Value::Int(1)])])
    }

    async fn prepare_physical_statement(
        &mut self,
        _key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        Ok(Arc::new(CleanupFailingStatement {
            failure: self.failure,
            closed: AtomicBool::new(false),
        }))
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    fn mark_discarded(&mut self) {
        self.discarded.store(true, Ordering::Release);
    }

    fn is_discarded(&self) -> bool {
        self.discarded.load(Ordering::Acquire)
    }

    fn driver_name(&self) -> &str {
        "mysql"
    }
}

struct CleanupFailingFactory {
    failure: CleanupFailure,
    discarded: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl druid::core::PhysicalConnectionFactory for CleanupFailingFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        Ok(Box::new(CleanupFailingConnection {
            failure: self.failure,
            discarded: Arc::clone(&self.discarded),
        }))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }
}

async fn assert_cleanup_failure_discards_connection(failure: CleanupFailure) {
    let discarded = Arc::new(AtomicBool::new(false));
    let pool = DruidPool::builder()
        .name("prepared-cleanup-failure")
        .db_type_name("mysql")
        .factory(Arc::new(CleanupFailingFactory {
            failure,
            discarded: Arc::clone(&discarded),
        }))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .expect("cleanup failure pool must build");
    let mut connection = pool.get().await.expect("connection acquisition");
    let mut statement = connection
        .prepare_statement("select 1")
        .await
        .expect("physical statement preparation");

    let error = statement
        .close_with_connection(&mut connection)
        .expect_err("fatal cleanup error must be returned");
    assert!(matches!(error, DruidError::SqlException(_)));
    assert!(connection.is_discarded());
    assert!(discarded.load(Ordering::Acquire));

    drop(statement);
    connection
        .close()
        .await
        .expect("discarded connection close must complete");
    assert_eq!(pool.state().discard_count, 1);
    pool.close().await;
}

#[tokio::test]
async fn clear_parameters_failure_uses_connection_sorter_before_cache_return() {
    assert_cleanup_failure_discards_connection(CleanupFailure::ClearParameters).await;
}

#[tokio::test]
async fn clear_batch_failure_uses_connection_sorter_before_cache_return() {
    assert_cleanup_failure_discards_connection(CleanupFailure::ClearBatch).await;
}

async fn assert_result_set_statement_handle_reports_cleanup_failure(failure: CleanupFailure) {
    let discarded = Arc::new(AtomicBool::new(false));
    let pool = DruidPool::builder()
        .name("prepared-result-set-handle-cleanup-failure")
        .db_type_name("mysql")
        .factory(Arc::new(CleanupFailingFactory {
            failure,
            discarded: Arc::clone(&discarded),
        }))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .expect("cleanup failure pool must build");
    let mut connection = pool.get().await.expect("connection acquisition");
    let mut statement = connection
        .prepare_statement("select 1")
        .await
        .expect("physical statement preparation");
    let result_set = statement
        .fetch_result_set(&mut connection, Vec::new())
        .await
        .expect("query must expose a ResultSet handle");
    let handle = result_set
        .prepared_statement()
        .expect("PreparedStatement ResultSet must retain its statement");

    let error = handle
        .close()
        .expect_err("physical cleanup failure must be returned");
    assert!(matches!(error, DruidError::SqlException(_)));
    assert!(!handle.is_closed());

    drop(result_set);
    drop(statement);
    connection
        .close()
        .await
        .expect("connection close must complete");
    pool.close().await;
}

#[tokio::test]
async fn result_set_statement_handle_reports_clear_parameters_failure() {
    assert_result_set_statement_handle_reports_cleanup_failure(CleanupFailure::ClearParameters)
        .await;
}

#[tokio::test]
async fn result_set_statement_handle_reports_clear_batch_failure() {
    assert_result_set_statement_handle_reports_cleanup_failure(CleanupFailure::ClearBatch).await;
}

async fn assert_direct_prepared_method_failure_discards_connection(failure: CleanupFailure) {
    let discarded = Arc::new(AtomicBool::new(false));
    let pool = DruidPool::builder()
        .name("prepared-direct-method-failure")
        .db_type_name("mysql")
        .factory(Arc::new(CleanupFailingFactory {
            failure,
            discarded: Arc::clone(&discarded),
        }))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection.prepare_statement("select 1").await.unwrap();

    let error = match failure {
        CleanupFailure::SetParameter => statement.set_int(&mut connection, 1, 1).unwrap_err(),
        CleanupFailure::AddBatch => statement
            .add_batch(&mut connection, vec![Value::Int(1)])
            .unwrap_err(),
        CleanupFailure::ClearParameters => statement.clear_parameters(&mut connection).unwrap_err(),
        CleanupFailure::ClearBatch => statement.clear_batch(&mut connection).unwrap_err(),
        CleanupFailure::GetResultSet
        | CleanupFailure::GetUpdateCount
        | CleanupFailure::GetGeneratedKeys
        | CleanupFailure::GetMoreResults => unreachable!("getter variants use a dedicated test"),
    };
    assert!(matches!(error, DruidError::SqlException(_)));
    assert!(connection.is_discarded());
    assert!(discarded.load(Ordering::Acquire));
    assert_eq!(statement.batch_size(), 0);

    drop(statement);
    connection.close().await.unwrap();
    assert_eq!(pool.state().discard_count, 1);
    pool.close().await;
}

#[tokio::test]
async fn direct_prepared_setter_and_batch_methods_route_failures_through_connection_sorter() {
    for failure in [
        CleanupFailure::SetParameter,
        CleanupFailure::AddBatch,
        CleanupFailure::ClearParameters,
        CleanupFailure::ClearBatch,
    ] {
        assert_direct_prepared_method_failure_discards_connection(failure).await;
    }
}

async fn assert_prepared_getter_failure_discards_connection(failure: CleanupFailure) {
    let discarded = Arc::new(AtomicBool::new(false));
    let pool = DruidPool::builder()
        .name("prepared-getter-failure")
        .db_type_name("mysql")
        .factory(Arc::new(CleanupFailingFactory {
            failure,
            discarded: Arc::clone(&discarded),
        }))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection.prepare_statement("select 1").await.unwrap();

    let error = match failure {
        CleanupFailure::GetResultSet => {
            statement.fetch(&mut connection, Vec::new()).await.unwrap();
            statement.result_set(&mut connection).unwrap_err()
        }
        CleanupFailure::GetUpdateCount => {
            statement.exec(&mut connection, Vec::new()).await.unwrap();
            statement.update_count(&mut connection).unwrap_err()
        }
        CleanupFailure::GetGeneratedKeys => {
            statement.exec(&mut connection, Vec::new()).await.unwrap();
            statement.generated_keys(&mut connection).unwrap_err()
        }
        CleanupFailure::GetMoreResults => {
            statement.fetch(&mut connection, Vec::new()).await.unwrap();
            let result_set = statement.result_set(&mut connection).unwrap().unwrap();
            let error = statement.more_results(&mut connection).unwrap_err();
            assert!(
                result_set.is_closed(),
                "fatal 驱动错误会立即 discard 连接并级联关闭旧 ResultSet wrapper"
            );
            error
        }
        CleanupFailure::SetParameter
        | CleanupFailure::AddBatch
        | CleanupFailure::ClearParameters
        | CleanupFailure::ClearBatch => {
            unreachable!("cleanup variants use dedicated tests")
        }
    };
    assert!(matches!(error, DruidError::SqlException(_)));
    assert!(connection.is_discarded());
    assert!(discarded.load(Ordering::Acquire));

    drop(statement);
    connection.close().await.unwrap();
    assert_eq!(pool.state().discard_count, 1);
    pool.close().await;
}

#[tokio::test]
async fn prepared_result_getters_route_driver_failures_through_connection_sorter() {
    for failure in [
        CleanupFailure::GetResultSet,
        CleanupFailure::GetUpdateCount,
        CleanupFailure::GetGeneratedKeys,
        CleanupFailure::GetMoreResults,
    ] {
        assert_prepared_getter_failure_discards_connection(failure).await;
    }
}

#[tokio::test]
async fn holder_statement_pool_is_lazy_and_debug_does_not_initialize_it() {
    let (pool, _) = prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    assert!(connection
        .connection_holder()
        .unwrap()
        .statement_pool_direct()
        .is_none());

    let _ = format!("{:?}", connection.connection_holder().unwrap());
    assert!(connection
        .connection_holder()
        .unwrap()
        .statement_pool_direct()
        .is_none());

    let statement_pool = connection.connection_holder_mut().unwrap().statement_pool();
    assert_eq!(
        statement_pool
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .size(),
        0
    );
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn prepared_statement_reuses_physical_handle_and_updates_java_stats() {
    let (pool, prepare_count) = prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();

    let mut first = connection.prepare_statement("select 1").await.unwrap();
    assert!(!first.is_wrapper_for(None));
    assert!(first.unwrap(None).is_none());
    assert!(first.is_wrapper_for_type::<DruidPooledPreparedStatement>());
    assert!(first.unwrap_ref::<DruidPooledPreparedStatement>().is_some());
    assert!(first.is_wrapper_for_type::<dyn PhysicalPreparedStatement>());
    assert!(first
        .unwrap(Some(TypeId::of::<dyn PhysicalPreparedStatement>()))
        .and_then(|value| value.prepared_statement())
        .is_some());
    assert!(first.is_wrapper_for_type::<SqlTextPreparedStatement>());
    assert!(first.unwrap_ref::<SqlTextPreparedStatement>().is_some());
    assert!(!first.is_wrapper_for_type::<UnsupportedPreparedUnwrapType>());
    assert!(first
        .unwrap_ref::<UnsupportedPreparedUnwrapType>()
        .is_none());
    assert!(first.prepared_statement_holder().is_in_use());
    assert!(!first.prepared_statement_holder().is_pooling());
    assert_eq!(first.fetch(&mut connection, vec![]).await.unwrap().len(), 1);
    first.close().unwrap();
    assert!(first.is_wrapper_for_type::<dyn PhysicalPreparedStatement>());
    assert!(first.unwrap_ref::<SqlTextPreparedStatement>().is_some());
    assert!(!first.prepared_statement_holder().is_in_use());
    assert!(first.prepared_statement_holder().is_pooling());
    drop(first);

    let mut second = connection.prepare_statement("select 1").await.unwrap();
    assert_eq!(prepare_count.load(Ordering::Relaxed), 1);
    assert_eq!(second.prepared_statement_holder().hit_count(), 1);
    second.exec(&mut connection, vec![]).await.unwrap();
    second.close().unwrap();
    drop(second);

    let state = pool.state();
    assert_eq!(state.prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_hit_count, 1);
    assert_eq!(state.cached_prepared_statement_miss_count, 1);
    assert_eq!(state.cached_prepared_statement_access_count, 2);

    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn concurrent_logical_statements_preserve_java_in_use_and_lru_semantics() {
    let (pool, _) = prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();

    let mut first = connection.prepare_statement("select 0").await.unwrap();
    first.close().unwrap();
    drop(first);

    let mut active = connection.prepare_statement("select 0").await.unwrap();
    assert!(active.prepared_statement_holder().is_pooling());
    assert!(active.prepared_statement_holder().is_in_use());

    for sql in ["select 1", "select 2", "select 3", "select 4"] {
        let mut statement = connection.prepare_statement(sql).await.unwrap();
        statement.close().unwrap();
    }
    let active_holder = active.prepared_statement_holder() as *const _;
    assert!(!active.prepared_statement_holder().is_pooling());
    active.close().unwrap();
    assert!(active.prepared_statement_holder().is_pooling());
    assert_eq!(
        active.prepared_statement_holder() as *const _,
        active_holder
    );
    drop(active);

    let statement_pool = connection
        .connection_holder()
        .unwrap()
        .statement_pool_direct()
        .unwrap();
    assert_eq!(
        statement_pool
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keys_in_lru_order()
            .iter()
            .map(PreparedStatementKey::sql)
            .collect::<Vec<_>>(),
        vec!["select 3", "select 4", "select 0"]
    );

    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn mysql_schema_change_clears_existing_statement_cache() {
    let (pool, _) = prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection.prepare_statement("select 1").await.unwrap();
    statement.close().unwrap();
    drop(statement);
    assert_eq!(pool.state().cached_prepared_statement_count, 1);

    connection.set_schema("tenant").await.unwrap();
    let state = pool.state();
    assert_eq!(state.cached_prepared_statement_count, 0);
    assert_eq!(state.cached_prepared_statement_delete_count, 1);
    assert_eq!(state.closed_prepared_statement_count, 1);

    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn statement_execution_error_removes_handle_instead_of_reusing_it() {
    let (pool, _) = prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection.prepare_statement("FAIL").await.unwrap();
    assert!(statement.exec(&mut connection, vec![]).await.is_err());
    statement.close().unwrap();
    assert!(!statement.prepared_statement_holder().is_pooling());
    drop(statement);

    let state = pool.state();
    // Java closePreapredStatement 对尚未入缓存的失败语句也无条件递减。
    assert_eq!(state.cached_prepared_statement_count, -1);
    assert_eq!(state.cached_prepared_statement_delete_count, 1);
    assert_eq!(state.closed_prepared_statement_count, 1);

    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn prepared_statement_cannot_cross_connection_lease_boundary() {
    let (pool, prepare_count) = prepared_pool(3).await;
    let mut first_connection = pool.get().await.unwrap();

    let mut seed = first_connection
        .prepare_statement("select 1")
        .await
        .unwrap();
    seed.close().unwrap();
    let mut leaked = first_connection
        .prepare_statement("select 1")
        .await
        .unwrap();
    assert_eq!(leaked.prepared_statement_holder().hit_count(), 1);
    first_connection.close().await.unwrap();

    let mut second_connection = pool.get().await.unwrap();
    assert!(leaked.exec(&mut second_connection, vec![]).await.is_err());
    let mut replacement = second_connection
        .prepare_statement("select 1")
        .await
        .unwrap();
    replacement.close().unwrap();
    // 旧逻辑 wrapper 已被第一租约 close，但底层缓存语句可由同一物理连接的
    // 下一租约安全复用；跨租约禁止的是逻辑对象，而不是物理缓存条目。
    assert_eq!(prepare_count.load(Ordering::Relaxed), 1);

    drop(leaked);
    let state = pool.state();
    assert_eq!(state.cached_prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_delete_count, 0);
    assert_eq!(state.closed_prepared_statement_count, 0);

    second_connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn prepared_batch_snapshots_parameters_and_executes_against_real_sqlite() {
    let reject_filter = Arc::new(RejectNextPreparedBatch {
        reject_next: AtomicBool::new(false),
    });
    let mut filter_chain = FilterChain::new();
    filter_chain.add_before(Arc::clone(&reject_filter) as Arc<dyn BeforeFilter>);
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 SQLite 物理连接");
    let mut connection = DruidPooledConnection::with_context(
        physical,
        91,
        "prepared-batch-sqlite".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    connection
        .exec(
            "CREATE TABLE prepared_batch_item(
                id INTEGER PRIMARY KEY,
                value TEXT NOT NULL
            )",
            Vec::new(),
        )
        .await
        .expect("真实 SQLite 建表必须成功");

    let sql = "INSERT INTO prepared_batch_item(id, value) VALUES (?1, ?2)";
    let mut statement = connection
        .prepare_statement(sql)
        .await
        .expect("必须创建池化 PreparedStatement");
    statement
        .add_batch(
            &mut connection,
            vec![Value::Int(1), Value::String("first".to_string())],
        )
        .unwrap();
    statement
        .add_batch(
            &mut connection,
            vec![Value::Int(2), Value::String("second".to_string())],
        )
        .unwrap();
    statement.clear_parameters(&mut connection).unwrap();
    assert_eq!(statement.batch_size(), 2);
    assert_eq!(
        statement.execute_batch(&mut connection).await.unwrap(),
        [1, 1]
    );
    assert_eq!(statement.batch_size(), 0);
    assert_eq!(
        statement.execute_batch(&mut connection).await.unwrap(),
        Vec::<i32>::new(),
        "SQLite JDBC oracle 在 executeBatch 后消费参数批次"
    );

    let rows = connection
        .fetch(
            "SELECT id, value FROM prepared_batch_item ORDER BY id",
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| row.values.clone())
            .collect::<Vec<_>>(),
        [
            vec![Value::Int(1), Value::String("first".to_string())],
            vec![Value::Int(2), Value::String("second".to_string())],
        ]
    );

    statement
        .add_batch(
            &mut connection,
            vec![Value::Int(3), Value::String("third".to_string())],
        )
        .unwrap();
    statement
        .add_batch(
            &mut connection,
            vec![Value::Int(1), Value::String("duplicate".to_string())],
        )
        .unwrap();
    statement
        .add_batch(
            &mut connection,
            vec![Value::Int(4), Value::String("fourth".to_string())],
        )
        .unwrap();
    let partial_error = statement
        .execute_batch(&mut connection)
        .await
        .expect_err("唯一键冲突必须保留部分成功信息");
    assert_eq!(partial_error.batch_update_counts(), Some([1].as_slice()));
    assert!(partial_error.sql_exception().is_some());
    assert_eq!(statement.batch_size(), 0);

    statement
        .add_batch(
            &mut connection,
            vec![Value::Int(5), Value::String("retry".to_string())],
        )
        .unwrap();
    reject_filter.reject_next.store(true, Ordering::Release);
    assert_eq!(
        statement.execute_batch(&mut connection).await.unwrap_err(),
        DruidError::Other("prepared batch rejected before physical execution".to_string())
    );
    assert_eq!(
        statement.batch_size(),
        1,
        "before Filter 短路发生在物理驱动消费批次之前"
    );
    assert_eq!(statement.execute_batch(&mut connection).await.unwrap(), [1]);
    assert_eq!(statement.batch_size(), 0);

    let ids = connection
        .fetch("SELECT id FROM prepared_batch_item ORDER BY id", Vec::new())
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.values[0].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        [Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(5)]
    );

    statement.close_with_connection(&mut connection).unwrap();
    statement.clear_batch(&mut connection).unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn prepared_generic_execute_matches_real_sqlite_result_and_generated_key_semantics() {
    let recorder = Arc::new(PreparedExecuteRecorder::default());
    let mut filter_chain = FilterChain::new();
    filter_chain.add_before(Arc::clone(&recorder) as Arc<dyn BeforeFilter>);
    filter_chain.add_after(Arc::clone(&recorder) as Arc<dyn AfterFilter>);
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 SQLite 物理连接");
    let mut connection = DruidPooledConnection::with_context(
        physical,
        92,
        "prepared-execute-sqlite".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    connection
        .exec(
            "CREATE TABLE prepared_execute_item(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                value TEXT NOT NULL
            )",
            Vec::new(),
        )
        .await
        .unwrap();
    recorder
        .events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();

    let mut query = connection
        .prepare_statement("SELECT ?1 AS value")
        .await
        .unwrap();
    assert!(query
        .execute(&mut connection, vec![Value::Int(7)])
        .await
        .unwrap());
    assert_eq!(query.update_count(&mut connection).unwrap(), -1);
    let mut rows = query.result_set(&mut connection).unwrap().unwrap();
    let prepared_identity = rows
        .prepared_statement()
        .expect("PreparedStatement ResultSet 必须恢复具体 statement 身份");
    assert!(prepared_identity.is_same_statement(&query));
    assert_eq!(prepared_identity.key(), query.key());
    assert!(std::ptr::eq(
        prepared_identity.pooled_statement().statement(),
        query.pooled_statement().statement()
    ));
    assert!(prepared_identity
        .unwrap(Some(TypeId::of::<dyn PhysicalPreparedStatement>()))
        .and_then(|value| value.prepared_statement())
        .is_some());
    assert!(rows.next(&mut connection).unwrap());
    assert_eq!(rows.object(&mut connection, 1).unwrap(), Value::Int(7));
    assert!(!query.more_results(&mut connection).unwrap());
    assert!(rows.is_closed());

    let query_events = recorder
        .events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(
        query_events,
        [
            (
                "SELECT ?1 AS value".to_string(),
                ExecOperation::Execute,
                vec![Value::Int(7)],
                true,
                None,
            ),
            (
                "SELECT ?1 AS value".to_string(),
                ExecOperation::Execute,
                vec![Value::Int(7)],
                false,
                Some(1),
            ),
        ],
        "PreparedStatement generic execute 只进入/退出 Filter 各一次"
    );

    assert!(query
        .execute(&mut connection, vec![Value::Int(8)])
        .await
        .unwrap());
    let invalid_result = query.result_set(&mut connection).unwrap().unwrap();
    assert!(query
        .more_results_with_current(&mut connection, 999)
        .is_err());
    assert!(
        !invalid_result.is_closed(),
        "Java 在无效 current 失败时不关闭旧 ResultSet wrapper"
    );
    drop(invalid_result);
    query.close_with_connection(&mut connection).unwrap();

    let mut plain_insert = connection
        .prepare_statement("INSERT INTO prepared_execute_item(value) VALUES (?1)")
        .await
        .unwrap();
    assert!(!plain_insert
        .execute(&mut connection, vec![Value::String("plain".to_string())])
        .await
        .unwrap());
    assert_eq!(plain_insert.update_count(&mut connection).unwrap(), 1);
    let mut plain_key = plain_insert.generated_keys(&mut connection).unwrap();
    assert!(plain_key
        .prepared_statement()
        .is_some_and(|identity| identity.is_same_statement(&plain_insert)));
    assert!(plain_key.next(&mut connection).unwrap());
    assert_eq!(plain_key.object(&mut connection, 1).unwrap(), Value::Int(1));
    plain_key.close_with_connection(&mut connection).unwrap();
    plain_insert.close_with_connection(&mut connection).unwrap();

    let mut auto_insert = connection
        .prepare_statement_with_auto_generated_keys(
            "INSERT INTO prepared_execute_item(value) VALUES (?1)",
            1,
        )
        .await
        .unwrap();
    assert!(!auto_insert
        .execute(&mut connection, vec![Value::String("auto".to_string())])
        .await
        .unwrap());
    let mut auto_key = auto_insert.generated_keys(&mut connection).unwrap();
    assert!(auto_key
        .prepared_statement()
        .is_some_and(|identity| identity.is_same_statement(&auto_insert)));
    assert!(auto_key.next(&mut connection).unwrap());
    assert_eq!(auto_key.object(&mut connection, 1).unwrap(), Value::Int(2));
    auto_key.close_with_connection(&mut connection).unwrap();
    auto_insert.close_with_connection(&mut connection).unwrap();

    let mut indexed_insert = connection
        .prepare_statement_with_column_indexes(
            "INSERT INTO prepared_execute_item(value) VALUES (?1)",
            vec![1],
        )
        .await
        .unwrap();
    assert!(!indexed_insert
        .execute(&mut connection, vec![Value::String("indexed".to_string())],)
        .await
        .unwrap());
    let mut indexed_key = indexed_insert.generated_keys(&mut connection).unwrap();
    assert!(indexed_key.next(&mut connection).unwrap());
    assert_eq!(
        indexed_key.object(&mut connection, 1).unwrap(),
        Value::Int(3)
    );
    indexed_key.close_with_connection(&mut connection).unwrap();
    indexed_insert
        .close_with_connection(&mut connection)
        .unwrap();

    let mut named_insert = connection
        .prepare_statement_with_column_names(
            "INSERT INTO prepared_execute_item(value) VALUES (?1)",
            vec!["id".to_string()],
        )
        .await
        .unwrap();
    assert!(!named_insert
        .execute(&mut connection, vec![Value::String("named".to_string())])
        .await
        .unwrap());
    let mut named_key = named_insert.generated_keys(&mut connection).unwrap();
    assert!(named_key.next(&mut connection).unwrap());
    assert_eq!(named_key.object(&mut connection, 1).unwrap(), Value::Int(4));
    named_key.close_with_connection(&mut connection).unwrap();
    named_insert.close_with_connection(&mut connection).unwrap();

    connection.close().await.unwrap();
}

#[tokio::test]
async fn prepared_result_set_keeps_the_same_statement_alive_and_can_close_it() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 SQLite 物理连接");
    let mut connection = DruidPooledConnection::new(physical, 93, Box::new(|_, _| {}));
    let mut statement = connection
        .prepare_statement("SELECT ?1 AS value")
        .await
        .unwrap();
    let expected_key = statement.key().clone();
    let mut result_set = statement
        .fetch_result_set(&mut connection, vec![Value::Int(11)])
        .await
        .unwrap();

    let identity = result_set
        .prepared_statement()
        .expect("PreparedStatement 结果集必须持有具体身份");
    assert!(
        result_set.callable_statement().is_none(),
        "普通 PreparedStatement 结果集不能伪装成 CallableStatement"
    );
    assert!(identity.is_same_statement(&statement));
    assert_eq!(identity.key(), &expected_key);
    assert!(!identity.is_closed());
    assert!(!identity.is_wrapper_for(None));
    assert!(identity.unwrap(None).is_none());
    assert!(identity.is_wrapper_for_type::<DruidPooledPreparedStatementHandle>());
    assert!(identity
        .unwrap_ref::<DruidPooledPreparedStatementHandle>()
        .is_some());
    assert!(identity.is_wrapper_for_type::<dyn PhysicalPreparedStatement>());
    assert!(identity
        .unwrap(Some(TypeId::of::<dyn PhysicalPreparedStatement>()))
        .and_then(|value| value.prepared_statement())
        .is_some());
    assert!(identity.is_wrapper_for_type::<ToastyPreparedStatement>());
    assert!(identity.unwrap_ref::<ToastyPreparedStatement>().is_some());
    assert!(!identity.is_wrapper_for_type::<UnsupportedPreparedUnwrapType>());
    assert!(identity
        .unwrap_ref::<UnsupportedPreparedUnwrapType>()
        .is_none());
    assert!(identity.as_any().is::<DruidPooledPreparedStatementHandle>());
    let statement_object = result_set.statement_object(&mut connection).unwrap();
    assert!(matches!(statement_object, ResultSetStatement::Prepared(_)));
    assert!(statement_object
        .prepared_statement()
        .expect("动态平台对象必须保留 PreparedStatement 身份")
        .is_same_statement(&statement));
    assert!(statement_object
        .pooled_statement()
        .is_same_statement(statement.pooled_statement()));
    assert!(statement_object.callable_statement().is_none());
    assert!(!statement_object.is_closed());

    drop(statement);
    assert!(
        !result_set.prepared_statement().unwrap().is_closed(),
        "ResultSet 对同一 Java statement 的强引用必须阻止提前回收"
    );
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(
        result_set.object(&mut connection, 1).unwrap(),
        Value::Int(11)
    );

    result_set.prepared_statement().unwrap().close().unwrap();
    result_set.prepared_statement().unwrap().close().unwrap();
    assert!(statement_object.is_closed());
    assert!(result_set.prepared_statement().unwrap().is_closed());
    assert!(
        result_set.is_closed(),
        "通过 ResultSet#getStatement 返回的句柄关闭语句必须级联关闭 ResultSet"
    );
    connection.close().await.unwrap();
}

#[tokio::test]
async fn prepared_bound_setters_execute_and_batch_against_real_sqlite() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 SQLite 物理连接");
    let mut connection = DruidPooledConnection::new(physical, 94, Box::new(|_, _| {}));
    connection
        .exec(
            "CREATE TABLE prepared_bound_item(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bool_value BOOLEAN,
                byte_value INTEGER,
                short_value INTEGER,
                int_value INTEGER,
                long_value INTEGER,
                float_value REAL,
                double_value REAL,
                decimal_value DECIMAL,
                string_value TEXT,
                n_string_value TEXT,
                bytes_value BLOB,
                date_value DATE,
                time_value TIME,
                timestamp_value DATETIME,
                null_value TEXT
            )",
            Vec::new(),
        )
        .await
        .unwrap();

    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_nano_opt(12, 13, 14, 123_456_789).unwrap();
    let timestamp = NaiveDateTime::new(date, time);
    let decimal = BigDecimal::from_str("123.4500").unwrap();
    let mut insert = connection
        .prepare_statement(
            "INSERT INTO prepared_bound_item(
                bool_value, byte_value, short_value, int_value, long_value,
                float_value, double_value, decimal_value, string_value,
                n_string_value, bytes_value, date_value, time_value,
                timestamp_value, null_value
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
             )",
        )
        .await
        .unwrap();
    insert.set_boolean(&mut connection, 1, true).unwrap();
    insert.set_byte(&mut connection, 2, -8).unwrap();
    insert.set_short(&mut connection, 3, 16).unwrap();
    insert.set_int(&mut connection, 4, 31).unwrap();
    insert.set_int(&mut connection, 4, 32).unwrap();
    insert.set_long(&mut connection, 5, i64::MAX).unwrap();
    insert.set_float(&mut connection, 6, 1.25).unwrap();
    insert.set_double(&mut connection, 7, 2.5).unwrap();
    insert
        .set_big_decimal(&mut connection, 8, Some(decimal.clone()))
        .unwrap();
    insert
        .set_string(&mut connection, 9, Some("text".to_string()))
        .unwrap();
    insert
        .set_n_string(&mut connection, 10, Some("国家字符".to_string()))
        .unwrap();
    insert
        .set_bytes(&mut connection, 11, Some(vec![0, 1, 2]))
        .unwrap();
    insert.set_date(&mut connection, 12, Some(date)).unwrap();
    insert.set_time(&mut connection, 13, Some(time)).unwrap();
    insert
        .set_timestamp(&mut connection, 14, Some(timestamp))
        .unwrap();
    insert.set_null(&mut connection, 15, 12).unwrap();
    assert_eq!(
        insert.parameter(1).unwrap().scalar_value().unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        insert.parameter(8).unwrap().scalar_value().unwrap(),
        Value::Decimal(decimal.clone())
    );

    assert_eq!(
        insert
            .execute_update_bound(&mut connection)
            .await
            .unwrap()
            .rows_affected,
        1
    );
    let rows = connection
        .fetch(
            "SELECT bool_value, byte_value, short_value, int_value, long_value,
                    float_value, double_value, decimal_value, string_value,
                    n_string_value, bytes_value, date_value, time_value,
                    timestamp_value, null_value
             FROM prepared_bound_item",
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        &rows[0].values,
        &[
            // SQLite 没有独立 BOOLEAN 存储类，真实读取按 INTEGER 返回。
            Value::Int(1),
            Value::Int(-8),
            Value::Int(16),
            Value::Int(32),
            Value::Int(i64::MAX),
            Value::Float(1.25),
            Value::Float(2.5),
            // SQLite NUMERIC affinity 会规范化无意义的小数末尾零。
            Value::Decimal(BigDecimal::from_str("123.45").unwrap()),
            Value::String("text".to_string()),
            Value::String("国家字符".to_string()),
            Value::Bytes(vec![0, 1, 2]),
            Value::Date(date),
            Value::Time(time),
            Value::Timestamp(timestamp),
            Value::Null,
        ]
    );

    insert.clear_parameters(&mut connection).unwrap();
    assert_eq!(insert.parameter_slot_count(), 0);
    insert.close_with_connection(&mut connection).unwrap();

    let mut batch = connection
        .prepare_statement("INSERT INTO prepared_bound_item(string_value) VALUES (?1)")
        .await
        .unwrap();
    batch
        .set_string(&mut connection, 1, Some("first".to_string()))
        .unwrap();
    batch.add_bound_batch(&mut connection).unwrap();
    batch
        .set_string(&mut connection, 1, Some("second".to_string()))
        .unwrap();
    batch.add_bound_batch(&mut connection).unwrap();
    assert_eq!(
        batch.execute_batch(&mut connection).await.unwrap(),
        vec![1, 1]
    );
    batch.close_with_connection(&mut connection).unwrap();

    let values = connection
        .fetch(
            "SELECT string_value FROM prepared_bound_item WHERE string_value IS NOT NULL ORDER BY id",
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        values
            .iter()
            .map(|row| row.get(0).cloned().unwrap())
            .collect::<Vec<_>>(),
        vec![
            Value::String("text".to_string()),
            Value::String("first".to_string()),
            Value::String("second".to_string()),
        ]
    );

    let mut query_result_set = connection.prepare_statement("SELECT ?1").await.unwrap();
    query_result_set.set_int(&mut connection, 1, 41).unwrap();
    let mut result_set = query_result_set
        .execute_query_bound(&mut connection)
        .await
        .unwrap();
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(
        result_set.object(&mut connection, 1).unwrap(),
        Value::Int(41)
    );
    result_set.close_with_connection(&mut connection).unwrap();
    query_result_set
        .close_with_connection(&mut connection)
        .unwrap();

    let mut eager_query = connection.prepare_statement("SELECT ?1").await.unwrap();
    eager_query.set_long(&mut connection, 1, 42).unwrap();
    assert_eq!(
        eager_query.fetch_bound(&mut connection).await.unwrap(),
        vec![Row::new(vec![Value::Int(42)])]
    );
    eager_query.close_with_connection(&mut connection).unwrap();

    let mut generic_query = connection.prepare_statement("SELECT ?1").await.unwrap();
    generic_query.set_short(&mut connection, 1, 43).unwrap();
    assert!(generic_query.execute_bound(&mut connection).await.unwrap());
    let mut generic_result_set = generic_query.result_set(&mut connection).unwrap().unwrap();
    assert!(generic_result_set.next(&mut connection).unwrap());
    assert_eq!(
        generic_result_set.object(&mut connection, 1).unwrap(),
        Value::Int(43)
    );
    generic_result_set
        .close_with_connection(&mut connection)
        .unwrap();
    generic_query
        .close_with_connection(&mut connection)
        .unwrap();

    let mut gap = connection
        .prepare_statement("INSERT INTO prepared_bound_item(int_value) VALUES (?1)")
        .await
        .unwrap();
    gap.set_int(&mut connection, 2, 2).unwrap();
    assert!(matches!(
        gap.execute_update_bound(&mut connection).await.unwrap_err(),
        DruidError::InvalidArgument(_)
    ));

    let stream = JdbcInputStream::from_bytes(b"not-eagerly-read".to_vec());
    let mut native_only = connection
        .prepare_statement("INSERT INTO prepared_bound_item(bytes_value) VALUES (?1)")
        .await
        .unwrap();
    native_only
        .set_binary_stream(&mut connection, 1, Some(stream.clone()))
        .unwrap();
    assert_eq!(
        native_only
            .execute_update_bound(&mut connection)
            .await
            .unwrap()
            .rows_affected,
        1
    );
    assert!(stream.read_to_end().unwrap().is_empty());

    drop(gap);
    drop(native_only);
    connection.close().await.unwrap();
}

#[tokio::test]
async fn prepared_generic_execute_preserves_ordered_multi_results() {
    let (pool, _) = prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection.prepare_statement("MULTI").await.unwrap();

    assert!(statement
        .execute(&mut connection, Vec::new())
        .await
        .unwrap());
    assert_eq!(statement.update_count(&mut connection).unwrap(), -1);
    let mut first = statement.result_set(&mut connection).unwrap().unwrap();
    assert!(first.next(&mut connection).unwrap());
    assert_eq!(first.object(&mut connection, 1).unwrap(), Value::Int(10));

    assert!(!statement.more_results(&mut connection).unwrap());
    assert!(first.is_closed());
    assert_eq!(statement.update_count(&mut connection).unwrap(), 2);
    let mut generated_keys = statement.generated_keys(&mut connection).unwrap();
    assert!(generated_keys.next(&mut connection).unwrap());
    assert_eq!(
        generated_keys.object(&mut connection, 1).unwrap(),
        Value::Int(99)
    );
    generated_keys
        .close_with_connection(&mut connection)
        .unwrap();

    assert!(statement.more_results(&mut connection).unwrap());
    let mut third = statement.result_set(&mut connection).unwrap().unwrap();
    assert!(third.next(&mut connection).unwrap());
    assert_eq!(third.object(&mut connection, 1).unwrap(), Value::Int(20));
    assert!(!statement.more_results(&mut connection).unwrap());
    assert!(third.is_closed());
    assert_eq!(statement.update_count(&mut connection).unwrap(), -1);

    statement.close_with_connection(&mut connection).unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn prepared_stream_reader_lob_url_rowid_and_null_bind_against_real_sqlite() {
    let recorder = Arc::new(PreparedDescriptorRecorder::default());
    let mut filter_chain = FilterChain::new();
    filter_chain.add_before(Arc::clone(&recorder) as Arc<dyn BeforeFilter>);
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 SQLite 物理连接");
    let mut connection = DruidPooledConnection::with_context(
        physical,
        95,
        "prepared-resource-sqlite".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    connection
        .exec(
            "CREATE TABLE prepared_resource_item(
                ascii_value TEXT,
                binary_value BLOB,
                character_value TEXT,
                n_character_value TEXT,
                blob_stream_value BLOB,
                clob_reader_value TEXT,
                nclob_reader_value TEXT,
                url_value TEXT,
                row_id_value BLOB,
                null_blob_value BLOB
            )",
            Vec::new(),
        )
        .await
        .unwrap();

    let short_stream = JdbcInputStream::from_bytes(vec![1, 2]);
    let mut invalid = connection.prepare_statement("SELECT ?1").await.unwrap();
    assert!(matches!(
        invalid.set_binary_stream_with_int_length(
            &mut connection,
            1,
            Some(short_stream.clone()),
            3,
        ),
        Err(DruidError::DriverError(_))
    ));
    assert!(short_stream.read_to_end().unwrap().is_empty());
    assert!(matches!(
        invalid.set_binary_stream_with_int_length(
            &mut connection,
            1,
            Some(JdbcInputStream::from_bytes(vec![1])),
            -1,
        ),
        Err(DruidError::InvalidArgument(_))
    ));
    invalid.close_with_connection(&mut connection).unwrap();

    let ascii = JdbcInputStream::from_bytes(b"abcdef".to_vec());
    let binary = JdbcInputStream::from_bytes(vec![0, 1, 2, 3]);
    let character = JdbcReader::from_string("hello");
    let national = JdbcReader::from_string("国家字符");
    let blob_stream = JdbcInputStream::from_bytes(vec![9, 8, 7]);
    let clob_reader = JdbcReader::from_string("clob-tail");
    let nclob_reader = JdbcReader::from_string("国字大对象");
    let mut statement = connection
        .prepare_statement(
            "INSERT INTO prepared_resource_item(
                ascii_value, binary_value, character_value, n_character_value,
                blob_stream_value, clob_reader_value, nclob_reader_value,
                url_value, row_id_value, null_blob_value
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .await
        .unwrap();
    statement
        .set_ascii_stream_with_int_length(&mut connection, 1, Some(ascii.clone()), 3)
        .unwrap();
    statement
        .set_binary_stream_with_long_length(&mut connection, 2, Some(binary.clone()), 3)
        .unwrap();
    statement
        .set_character_stream_with_int_length(&mut connection, 3, Some(character.clone()), 3)
        .unwrap();
    statement
        .set_n_character_stream(&mut connection, 4, Some(national.clone()))
        .unwrap();
    statement
        .set_blob_stream_with_long_length(&mut connection, 5, Some(blob_stream.clone()), 2)
        .unwrap();
    statement
        .set_clob_reader_with_long_length(&mut connection, 6, Some(clob_reader.clone()), 4)
        .unwrap();
    statement
        .set_n_clob_reader(&mut connection, 7, Some(nclob_reader.clone()))
        .unwrap();
    statement
        .set_url(
            &mut connection,
            8,
            Some(JdbcUrl::new("https://example.com/路径")),
        )
        .unwrap();
    statement
        .set_row_id(&mut connection, 9, Some(JdbcRowId::new(vec![4, 5, 6])))
        .unwrap();
    statement.set_blob(&mut connection, 10, None).unwrap();

    // Java SQLite oracle 在 setBinaryStream 时立即读取并校验资源。Toasty
    // 物理 PreparedStatement 同样必须在 setter 返回前推进游标；execute
    // 只能使用已经物化的参数，不能再次读取这些剩余内容。
    assert_eq!(ascii.read_to_end().unwrap(), b"def");
    assert_eq!(binary.read_to_end().unwrap(), vec![3]);
    assert_eq!(character.read_to_string().unwrap(), "lo");
    assert!(national.read_to_string().unwrap().is_empty());
    assert_eq!(blob_stream.read_to_end().unwrap(), vec![7]);
    assert_eq!(clob_reader.read_to_string().unwrap(), "-tail");
    assert!(nclob_reader.read_to_string().unwrap().is_empty());

    assert_eq!(
        statement
            .execute_update_bound(&mut connection)
            .await
            .unwrap()
            .rows_affected,
        1
    );

    let rows = connection
        .fetch(
            "SELECT ascii_value, binary_value, character_value, n_character_value,
                    blob_stream_value, clob_reader_value, nclob_reader_value,
                    url_value, row_id_value, null_blob_value
             FROM prepared_resource_item",
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        rows,
        vec![Row::new(vec![
            Value::String("abc".to_string()),
            Value::Bytes(vec![0, 1, 2]),
            Value::String("hel".to_string()),
            Value::String("国家字符".to_string()),
            Value::Bytes(vec![9, 8]),
            Value::String("clob".to_string()),
            Value::String("国字大对象".to_string()),
            Value::String("https://example.com/路径".to_string()),
            Value::Bytes(vec![4, 5, 6]),
            Value::Null,
        ])]
    );

    let mut query = connection.prepare_statement("SELECT ?1, ?2").await.unwrap();
    query
        .set_binary_stream(
            &mut connection,
            1,
            Some(JdbcInputStream::from_bytes(vec![1, 3, 5])),
        )
        .unwrap();
    query
        .set_character_stream(
            &mut connection,
            2,
            Some(JdbcReader::from_string("query-reader")),
        )
        .unwrap();
    let mut query_rows = query.execute_query_bound(&mut connection).await.unwrap();
    assert!(query_rows.next(&mut connection).unwrap());
    assert_eq!(
        query_rows.object(&mut connection, 1).unwrap(),
        Value::Bytes(vec![1, 3, 5])
    );
    assert_eq!(
        query_rows.object(&mut connection, 2).unwrap(),
        Value::String("query-reader".to_string())
    );
    query_rows.close_with_connection(&mut connection).unwrap();
    query.close_with_connection(&mut connection).unwrap();

    let mut generic = connection.prepare_statement("SELECT ?1").await.unwrap();
    generic
        .set_blob_stream(
            &mut connection,
            1,
            Some(JdbcInputStream::from_bytes(vec![2, 4, 6])),
        )
        .unwrap();
    assert!(generic.execute_bound(&mut connection).await.unwrap());
    let mut generic_rows = generic.result_set(&mut connection).unwrap().unwrap();
    assert!(generic_rows.next(&mut connection).unwrap());
    assert_eq!(
        generic_rows.object(&mut connection, 1).unwrap(),
        Value::Bytes(vec![2, 4, 6])
    );
    generic_rows.close_with_connection(&mut connection).unwrap();
    generic.close_with_connection(&mut connection).unwrap();

    let first_batch_stream = JdbcInputStream::from_bytes(vec![10, 11, 12]);
    let second_batch_reader = JdbcReader::from_string("第二批");
    let mut batch = connection
        .prepare_statement(
            "INSERT INTO prepared_resource_item(binary_value, character_value) VALUES (?1, ?2)",
        )
        .await
        .unwrap();
    batch
        .set_binary_stream_with_int_length(&mut connection, 1, Some(first_batch_stream.clone()), 2)
        .unwrap();
    batch
        .set_character_stream(&mut connection, 2, Some(JdbcReader::from_string("first")))
        .unwrap();
    batch.add_bound_batch(&mut connection).unwrap();
    batch
        .set_binary_stream(
            &mut connection,
            1,
            Some(JdbcInputStream::from_bytes(vec![20, 21])),
        )
        .unwrap();
    batch
        .set_character_stream(&mut connection, 2, Some(second_batch_reader.clone()))
        .unwrap();
    batch.add_bound_batch(&mut connection).unwrap();
    assert_eq!(
        batch.execute_batch(&mut connection).await.unwrap(),
        vec![1, 1]
    );
    assert_eq!(first_batch_stream.read_to_end().unwrap(), vec![12]);
    assert!(second_batch_reader.read_to_string().unwrap().is_empty());
    batch.close_with_connection(&mut connection).unwrap();

    let batch_rows = connection
        .fetch(
            "SELECT binary_value, character_value
             FROM prepared_resource_item
             WHERE binary_value IS NOT NULL
             ORDER BY rowid
             LIMIT 2 OFFSET 1",
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        batch_rows,
        vec![
            Row::new(vec![
                Value::Bytes(vec![10, 11]),
                Value::String("first".to_string()),
            ]),
            Row::new(vec![
                Value::Bytes(vec![20, 21]),
                Value::String("第二批".to_string()),
            ]),
        ]
    );

    {
        let recorded = recorder
            .parameter_sets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(recorded.len(), 3);
        assert_eq!(recorded[0].len(), 10);
        assert!(matches!(
            recorded[0][0],
            PreparedInputParameter::AsciiStream {
                length: JdbcStreamLength::Int(3),
                ..
            }
        ));
        assert!(matches!(recorded[0][9], PreparedInputParameter::Blob(None)));
    }
    {
        let recorded_batches = recorder
            .batch_parameter_sets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(recorded_batches.len(), 1);
        assert_eq!(recorded_batches[0].len(), 2);
        assert!(matches!(
            recorded_batches[0][0][0],
            PreparedInputParameter::BinaryStream {
                length: JdbcStreamLength::Int(2),
                ..
            }
        ));
        assert!(matches!(
            recorded_batches[0][1][1],
            PreparedInputParameter::CharacterStream {
                length: JdbcCharacterLength::Unspecified,
                ..
            }
        ));
    }

    statement.close_with_connection(&mut connection).unwrap();
    connection.close().await.unwrap();
}
