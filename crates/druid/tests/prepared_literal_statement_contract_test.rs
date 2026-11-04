//! `java.sql.PreparedStatement` 继承的字面 SQL 重载、batch 与结果状态。
//!
//! 通过记录型 PreparedStatement 句柄，断言 SQL 与 `GeneratedKeys` 变体成对转发、
//! `addBatch(String)` 进入 driver batch、`getResultSet` 返回当前状态，并覆盖
//! `getMetaData` 的精确默认不支持错误与 `setBigDecimal` 描述符。

use bigdecimal::BigDecimal;
use druid::core::DruidPooledConnection;
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{
    ExecutionResult, GeneratedKeys, ParameterIndex, PreparedInputParameter, PreparedStatement,
    Statement, StatementOptions,
};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
enum Call {
    Query(String),
    Update(String, GeneratedKeys),
    Execute(String, GeneratedKeys),
    AddBatch(String),
}

#[derive(Debug, Default)]
struct State {
    calls: Mutex<Vec<Call>>,
    parameters: Mutex<Vec<(usize, PreparedInputParameter)>>,
}

struct RecordingPrepared {
    sql: String,
    state: Arc<State>,
}

druid_rdbc::impl_standard_wrapper!(RecordingPrepared => PreparedStatement);

#[async_trait::async_trait]
impl Statement for RecordingPrepared {
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
        self.state
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Call::Query(sql.to_owned()));
        Ok(Box::new(druid_rdbc::extensions::BufferedRowSet::new(vec![
            druid_rdbc::types::Row::new(vec![druid_rdbc::types::Value::Int(1)]),
        ])))
    }

    async fn execute_update(&mut self, sql: &str, keys: GeneratedKeys) -> Result<i64, RdbcError> {
        self.state
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Call::Update(sql.to_owned(), keys));
        Ok(1)
    }

    async fn execute(
        &mut self,
        sql: &str,
        keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.state
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Call::Execute(sql.to_owned(), keys));
        Ok(vec![ExecutionResult::UpdateCount(1)])
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.state
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Call::AddBatch(sql));
        Ok(())
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        Ok(vec![1])
    }
}

#[async_trait::async_trait]
impl PreparedStatement for RecordingPrepared {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok(&self.sql)
    }

    fn set_input_parameter(
        &mut self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.state
            .parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((index.get(), parameter));
        Ok(())
    }

    fn result_set_meta_data(
        &self,
    ) -> Result<Arc<dyn druid_rdbc::metadata::ResultSetMetaData>, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "prepared_statement_result_set_meta_data",
        ))
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        Ok(1)
    }
}

struct RecordingConnection {
    state: Arc<State>,
}

druid_rdbc::impl_standard_wrapper!(RecordingConnection => Connection);

#[async_trait::async_trait]
impl Connection for RecordingConnection {
    async fn prepare_statement(
        &mut self,
        sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        Ok(Box::new(RecordingPrepared {
            sql: sql.to_owned(),
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
        Ok("recording-prepared-literal".to_owned())
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

async fn recording_prepared() -> (
    Arc<State>,
    DruidPooledConnection,
    Box<dyn PreparedStatement>,
) {
    let state = Arc::new(State::default());
    let pool = DruidPool::builder()
        .name("prepared-literal")
        .driver_name("recording-prepared-literal")
        .factory(Arc::new(RecordingFactory {
            state: Arc::clone(&state),
        }) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("recording pool");
    let mut connection = pool.get().await.expect("pooled connection");
    let prepared = connection
        .prepare_statement("SELECT ?", StatementOptions::default())
        .await
        .expect("prepared handle");
    (state, connection, prepared)
}

fn calls(state: &State) -> Vec<Call> {
    state
        .calls
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

#[tokio::test]
async fn prepared_literal_update_overloads_forward_sql_and_generated_keys() {
    let (state, _connection, mut prepared) = recording_prepared().await;
    let sql = "UPDATE t SET v = 1";

    assert_eq!(
        Statement::execute_update(prepared.as_mut(), sql, GeneratedKeys::None)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        Statement::execute_update(prepared.as_mut(), sql, GeneratedKeys::ReturnGeneratedKeys)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        Statement::execute_update(
            prepared.as_mut(),
            sql,
            GeneratedKeys::ColumnIndexes(
                vec![druid_rdbc::result_set::ColumnIndex::new(1).unwrap()]
            )
        )
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        Statement::execute_update(
            prepared.as_mut(),
            sql,
            GeneratedKeys::ColumnNames(vec!["id".to_owned()])
        )
        .await
        .unwrap(),
        1
    );

    assert_eq!(
        calls(&state),
        vec![
            Call::Update(sql.to_owned(), GeneratedKeys::None),
            Call::Update(sql.to_owned(), GeneratedKeys::ReturnGeneratedKeys),
            Call::Update(
                sql.to_owned(),
                GeneratedKeys::ColumnIndexes(vec![
                    druid_rdbc::result_set::ColumnIndex::new(1).unwrap()
                ])
            ),
            Call::Update(
                sql.to_owned(),
                GeneratedKeys::ColumnNames(vec!["id".to_owned()])
            ),
        ]
    );
}

#[tokio::test]
async fn prepared_literal_execute_query_and_batch_reach_the_driver() {
    let (state, _connection, mut prepared) = recording_prepared().await;

    let mut result = Statement::execute_query(prepared.as_mut(), "SELECT 42")
        .await
        .unwrap();
    assert!(result.next().await.unwrap());
    result.close().await.unwrap();

    Statement::add_batch(prepared.as_mut(), "UPDATE a".to_owned()).unwrap();
    assert_eq!(
        Statement::execute_batch(prepared.as_mut()).await.unwrap(),
        vec![1]
    );

    for keys in [
        GeneratedKeys::None,
        GeneratedKeys::ReturnGeneratedKeys,
        GeneratedKeys::ColumnIndexes(vec![druid_rdbc::result_set::ColumnIndex::new(1).unwrap()]),
        GeneratedKeys::ColumnNames(vec!["id".to_owned()]),
    ] {
        let generic = Statement::execute(prepared.as_mut(), "CALL p()", keys.clone())
            .await
            .unwrap();
        assert_eq!(generic.len(), 1);
        assert!(matches!(generic[0], ExecutionResult::UpdateCount(1)));
    }
    assert!(prepared.get_result_set().await.unwrap().is_none());

    assert_eq!(
        calls(&state),
        vec![
            Call::Query("SELECT 42".to_owned()),
            Call::AddBatch("UPDATE a".to_owned()),
            Call::Execute("CALL p()".to_owned(), GeneratedKeys::None),
            Call::Execute("CALL p()".to_owned(), GeneratedKeys::ReturnGeneratedKeys),
            Call::Execute(
                "CALL p()".to_owned(),
                GeneratedKeys::ColumnIndexes(vec![
                    druid_rdbc::result_set::ColumnIndex::new(1).unwrap()
                ])
            ),
            Call::Execute(
                "CALL p()".to_owned(),
                GeneratedKeys::ColumnNames(vec!["id".to_owned()])
            ),
        ]
    );
}

#[tokio::test]
async fn prepared_result_set_meta_data_and_big_decimal_are_observable() {
    let (state, _connection, mut prepared) = recording_prepared().await;

    assert!(matches!(
        prepared.result_set_meta_data(),
        Err(RdbcError::SqlException(ref exception))
            if exception.sql_state() == Some("0A000")
                && exception.class_name() == "java.sql.SQLFeatureNotSupportedException"
                && exception
                    .message()
                    .is_some_and(|message| message.contains("prepared_statement_result_set_meta_data"))
    ));

    prepared
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BigDecimal(Some(BigDecimal::from_str("123.45").unwrap())),
        )
        .unwrap();
    let recorded = state
        .parameters
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].0, 1);
    assert_eq!(
        recorded[0].1,
        PreparedInputParameter::BigDecimal(Some(BigDecimal::from_str("123.45").unwrap()))
    );
}
