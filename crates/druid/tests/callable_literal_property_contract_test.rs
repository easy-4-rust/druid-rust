//! `java.sql.CallableStatement` 继承的字面 SQL、属性、资源 setter 与 OUT getter。
//!
//! 记录型 Callable 句柄断言 SQL/GeneratedKeys 转发、Statement 属性回读、
//! 资源描述符与 OUT 值读取；`getGeneratedKeys` 等默认入口断言精确错误。

use druid::core::DruidPooledConnection;
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
    ExecutionResult, FetchDirection, GeneratedKeys, ParameterIndex, PreparedInputParameter,
    PreparedStatement, Statement, StatementOptions,
};
use druid_rdbc::types::{RdbcObject, Value};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
enum Call {
    Query(String),
    Update(String, GeneratedKeys),
    Execute(String, GeneratedKeys),
    AddBatch(String),
    Batch,
}

#[derive(Debug, Default)]
struct State {
    calls: Mutex<Vec<Call>>,
    parameters: Mutex<Vec<(usize, PreparedInputParameter)>>,
    max_field_size: Mutex<i32>,
    fetch_direction: Mutex<FetchDirection>,
    poolable: Mutex<bool>,
    close_on_completion: Mutex<bool>,
    cursor_name: Mutex<Option<String>>,
    escape_processing: Mutex<bool>,
}

struct RecordingCallable {
    state: Arc<State>,
}

druid_rdbc::impl_standard_wrapper!(RecordingCallable => CallableStatement);

#[async_trait::async_trait]
impl Statement for RecordingCallable {
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    async fn execute_query(
        &mut self,
        sql: &str,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        self.record(Call::Query(sql.to_owned()));
        Ok(Box::new(druid_rdbc::extensions::BufferedRowSet::new(vec![
            druid_rdbc::types::Row::new(vec![druid_rdbc::types::Value::Int(1)]),
        ])))
    }

    async fn execute_update(&mut self, sql: &str, keys: GeneratedKeys) -> Result<i64, RdbcError> {
        self.record(Call::Update(sql.to_owned(), keys));
        Ok(1)
    }

    async fn execute(
        &mut self,
        sql: &str,
        keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.record(Call::Execute(sql.to_owned(), keys));
        Ok(vec![ExecutionResult::UpdateCount(1)])
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.record(Call::AddBatch(sql));
        Ok(())
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.record(Call::Batch);
        Ok(vec![1])
    }

    fn max_field_size(&self) -> Result<i32, RdbcError> {
        Ok(*self.state.max_field_size.lock().unwrap())
    }

    fn set_max_field_size(&mut self, max: i32) -> Result<(), RdbcError> {
        *self.state.max_field_size.lock().unwrap() = max;
        Ok(())
    }

    fn set_escape_processing(&mut self, enabled: bool) -> Result<(), RdbcError> {
        *self.state.escape_processing.lock().unwrap() = enabled;
        Ok(())
    }

    fn set_cursor_name(&mut self, name: &str) -> Result<(), RdbcError> {
        *self.state.cursor_name.lock().unwrap() = Some(name.to_owned());
        Ok(())
    }

    fn set_fetch_direction(&mut self, direction: FetchDirection) -> Result<(), RdbcError> {
        *self.state.fetch_direction.lock().unwrap() = direction;
        Ok(())
    }

    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        Ok(*self.state.fetch_direction.lock().unwrap())
    }

    fn is_poolable(&self) -> Result<bool, RdbcError> {
        Ok(*self.state.poolable.lock().unwrap())
    }

    fn set_poolable(&mut self, poolable: bool) -> Result<(), RdbcError> {
        *self.state.poolable.lock().unwrap() = poolable;
        Ok(())
    }

    fn close_on_completion(&mut self) -> Result<(), RdbcError> {
        *self.state.close_on_completion.lock().unwrap() = true;
        Ok(())
    }

    fn is_close_on_completion(&self) -> Result<bool, RdbcError> {
        Ok(*self.state.close_on_completion.lock().unwrap())
    }
}

#[async_trait::async_trait]
impl PreparedStatement for RecordingCallable {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok("{call demo(?)}")
    }

    fn set_input_parameter(
        &mut self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.state
            .parameters
            .lock()
            .unwrap()
            .push((index.get(), parameter));
        Ok(())
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        Ok(1)
    }

    async fn execute_prepared_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        Ok(vec![1])
    }
}

#[async_trait::async_trait]
impl CallableStatement for RecordingCallable {
    fn register_out_parameter(
        &mut self,
        _parameter: CallableParameter,
        _out_parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        Ok(())
    }

    fn set_named_parameter(
        &mut self,
        _parameter_name: &str,
        _parameter: CallableInputParameter,
    ) -> Result<(), RdbcError> {
        Ok(())
    }

    fn out_parameter(&self, parameter: &CallableParameter) -> Result<RdbcObject, RdbcError> {
        Ok(match parameter {
            CallableParameter::Index(index) => match index.get() {
                1 => RdbcObject::Scalar(Value::Bool(true)),
                2 => RdbcObject::Scalar(Value::Bytes(vec![1, 2])),
                3 => RdbcObject::Scalar(Value::Float(2.5)),
                4 => RdbcObject::Scalar(Value::Float(1.5)),
                other => return Err(RdbcError::DriverError(format!("no OUT at {other}"))),
            },
            CallableParameter::Name(name) if name == "s" => RdbcObject::Scalar(Value::Int(7)),
            CallableParameter::Name(name) => {
                return Err(RdbcError::DriverError(format!("no OUT named {name}")))
            }
        })
    }
}

impl RecordingCallable {
    fn record(&self, call: Call) {
        self.state
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(call);
    }
}

struct RecordingConnection {
    state: Arc<State>,
}

druid_rdbc::impl_standard_wrapper!(RecordingConnection => Connection);

#[async_trait::async_trait]
impl Connection for RecordingConnection {
    async fn prepare_call(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        Ok(Box::new(RecordingCallable {
            state: Arc::clone(&self.state),
        }))
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("recording-callable-literal".to_owned())
    }
}

struct RecordingFactory {
    state: Arc<State>,
}

#[async_trait::async_trait]
impl ConnectionFactory for RecordingFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(RecordingConnection {
            state: Arc::clone(&self.state),
        }))
    }
}

async fn recording_callable() -> (
    Arc<State>,
    DruidPooledConnection,
    Box<dyn CallableStatement>,
) {
    let state = Arc::new(State::default());
    let pool = DruidPool::builder()
        .name("callable-literal")
        .driver_name("recording-callable-literal")
        .factory(Arc::new(RecordingFactory {
            state: Arc::clone(&state),
        }) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("recording pool");
    let mut connection = pool.get().await.expect("pooled connection");
    let callable = connection
        .prepare_call("{call demo(?)}", StatementOptions::default())
        .await
        .expect("callable handle");
    (state, connection, callable)
}

fn calls(state: &State) -> Vec<Call> {
    state.calls.lock().unwrap().clone()
}

#[tokio::test]
async fn callable_literal_overloads_forward_sql_and_generated_keys() {
    let (state, _connection, mut callable) = recording_callable().await;
    let sql = "UPDATE t SET v = 1";
    for keys in [
        GeneratedKeys::None,
        GeneratedKeys::ReturnGeneratedKeys,
        GeneratedKeys::ColumnIndexes(vec![druid_rdbc::result_set::ColumnIndex::new(1).unwrap()]),
        GeneratedKeys::ColumnNames(vec!["id".to_owned()]),
    ] {
        assert_eq!(
            Statement::execute_update(callable.as_mut(), sql, keys.clone())
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            Statement::execute(callable.as_mut(), sql, keys.clone())
                .await
                .unwrap()
                .len(),
            1
        );
    }
    assert_eq!(
        Statement::execute_query(callable.as_mut(), "SELECT 1")
            .await
            .unwrap()
            .known_row_count(),
        Some(1)
    );
    Statement::add_batch(callable.as_mut(), "UPDATE a".to_owned()).unwrap();
    assert_eq!(
        Statement::execute_batch(callable.as_mut()).await.unwrap(),
        vec![1]
    );
    assert_eq!(
        PreparedStatement::execute_prepared_update(callable.as_mut())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        PreparedStatement::execute_prepared_batch(callable.as_mut())
            .await
            .unwrap(),
        vec![1]
    );
    assert_eq!(calls(&state).len(), 11);
    let mut current = callable
        .get_result_set()
        .await
        .unwrap()
        .expect("query result remains current");
    assert!(current.next().await.unwrap());
    current.close().await.unwrap();
    callable.clear_parameters().unwrap();
}

#[tokio::test]
async fn callable_statement_properties_round_trip_through_the_handle() {
    let (_state, _connection, mut callable) = recording_callable().await;

    callable.set_max_field_size(2048).unwrap();
    assert_eq!(callable.max_field_size().unwrap(), 2048);
    callable
        .set_fetch_direction(FetchDirection::Reverse)
        .unwrap();
    assert_eq!(callable.fetch_direction().unwrap(), FetchDirection::Reverse);
    callable.set_poolable(true).unwrap();
    assert!(callable.is_poolable().unwrap());
    callable.close_on_completion().unwrap();
    assert!(callable.is_close_on_completion().unwrap());
    callable.set_cursor_name("cursor").unwrap();
    callable.set_escape_processing(false).unwrap();
}

#[tokio::test]
async fn callable_resource_setters_and_out_getters_use_exact_descriptors() {
    let (state, _connection, mut callable) = recording_callable().await;

    callable
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::Array(None),
        )
        .unwrap();
    callable
        .set_input_parameter(
            ParameterIndex::new(2).unwrap(),
            PreparedInputParameter::Ref(None),
        )
        .unwrap();
    callable
        .set_input_parameter(
            ParameterIndex::new(3).unwrap(),
            PreparedInputParameter::RowId(None),
        )
        .unwrap();
    callable
        .set_input_parameter(
            ParameterIndex::new(4).unwrap(),
            PreparedInputParameter::SqlXml(None),
        )
        .unwrap();
    callable
        .set_input_parameter(
            ParameterIndex::new(5).unwrap(),
            PreparedInputParameter::Url(None),
        )
        .unwrap();
    callable
        .set_input_parameter(
            ParameterIndex::new(6).unwrap(),
            PreparedInputParameter::UnicodeStream {
                stream: None,
                length: 3,
            },
        )
        .unwrap();

    assert_eq!(
        callable
            .boolean_out_parameter(&CallableParameter::by_index(1).unwrap())
            .unwrap(),
        true
    );
    assert_eq!(
        callable
            .bytes_out_parameter(&CallableParameter::by_index(2).unwrap())
            .unwrap(),
        Some(vec![1, 2])
    );
    assert_eq!(
        callable
            .double_out_parameter(&CallableParameter::by_index(3).unwrap())
            .unwrap(),
        2.5
    );
    assert_eq!(
        callable
            .float_out_parameter(&CallableParameter::by_index(4).unwrap())
            .unwrap(),
        1.5
    );
    assert_eq!(
        callable
            .short_out_parameter(&CallableParameter::by_name("s").unwrap())
            .unwrap(),
        7
    );

    let recorded = state.parameters.lock().unwrap().clone();
    assert_eq!(recorded.len(), 6);
    assert_eq!(recorded[0].1, PreparedInputParameter::Array(None));
    assert_eq!(recorded[1].1, PreparedInputParameter::Ref(None));
    assert_eq!(recorded[2].1, PreparedInputParameter::RowId(None));
    assert_eq!(recorded[3].1, PreparedInputParameter::SqlXml(None));
    assert_eq!(recorded[4].1, PreparedInputParameter::Url(None));
    assert_eq!(
        recorded[5].1,
        PreparedInputParameter::UnicodeStream {
            stream: None,
            length: 3
        }
    );
}
