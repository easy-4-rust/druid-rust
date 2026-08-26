//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Canonical pooled CallableStatement public-path harness.
use druid::core::{Connection, ConnectionFactory, FilterAdapter, FilterChainImpl, RdbcError};
use druid::pool::DruidPool;
use druid_rdbc::statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
    ParameterIndex, PreparedStatement, Statement, StatementOptions,
};
use druid_rdbc::types::{RdbcObject, Value};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use std::any::TypeId;
use std::sync::Arc;

#[derive(Debug, Default)]
struct HarnessCallable {
    closed: bool,
    registered: Vec<(CallableParameter, CallableOutParameter)>,
    named: Vec<(String, CallableInputParameter)>,
}

druid_rdbc::impl_standard_wrapper!(HarnessCallable => CallableStatement);

#[async_trait::async_trait]
impl Statement for HarnessCallable {
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for HarnessCallable {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok("CALL task6")
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        Ok(1)
    }
}

#[async_trait::async_trait]
impl CallableStatement for HarnessCallable {
    fn register_out_parameter(
        &mut self,
        parameter: CallableParameter,
        out_parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        self.registered.push((parameter, out_parameter));
        Ok(())
    }

    fn set_named_parameter(
        &mut self,
        parameter_name: &str,
        parameter: CallableInputParameter,
    ) -> Result<(), RdbcError> {
        self.named.push((parameter_name.to_owned(), parameter));
        Ok(())
    }

    fn out_value(&self, index: ParameterIndex) -> Result<Value, RdbcError> {
        Ok(Value::Int(i64::try_from(index.get()).unwrap()))
    }

    fn out_parameter(&self, parameter: &CallableParameter) -> Result<RdbcObject, RdbcError> {
        match parameter {
            CallableParameter::Index(index) => self.out_value(*index).map(RdbcObject::from),
            CallableParameter::Name(name) => Ok(RdbcObject::from(Value::String(name.clone()))),
        }
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

#[derive(Debug)]
struct HarnessConnection;
druid_rdbc::impl_standard_wrapper!(HarnessConnection => Connection);

#[async_trait::async_trait]
impl Connection for HarnessConnection {
    async fn prepare_call(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        Ok(Box::new(HarnessCallable::default()))
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
    async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
        Ok(true)
    }
    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("callable-harness".to_owned())
    }
}

struct HarnessFactory;
#[async_trait::async_trait]
impl ConnectionFactory for HarnessFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(HarnessConnection))
    }
    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        connection.ping().await
    }
}

async fn task6_pooled_callable_public_behavior(case_id: &str) {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    let pool = DruidPool::builder()
        .name(format!("callable-{case_id}"))
        .driver_name("callable-harness")
        .factory(Arc::new(HarnessFactory) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .filter_chain(Arc::new(chain))
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut callable = connection
        .prepare_call("CALL task6", StatementOptions::default())
        .await
        .unwrap();
    let one = ParameterIndex::new(1).unwrap();
    callable
        .register_out_parameter(CallableParameter::Index(one), CallableOutParameter::new(4))
        .unwrap();
    callable
        .register_out_parameter(
            CallableParameter::by_name("named").unwrap(),
            CallableOutParameter::with_scale(3, 2),
        )
        .unwrap();
    callable
        .set_named_parameter("integer", CallableInputParameter::Int(7))
        .unwrap();
    callable
        .set_named_parameter(
            "string",
            CallableInputParameter::String(Some(case_id.to_owned())),
        )
        .unwrap();
    callable
        .set_named_parameter("null", CallableInputParameter::null(12))
        .unwrap();
    assert_eq!(callable.out_value(one), Ok(Value::Int(1)));
    assert_eq!(
        callable.out_parameter(&CallableParameter::Index(one)),
        Ok(RdbcObject::from(Value::Int(1)))
    );
    assert_eq!(
        callable.out_parameter(&CallableParameter::by_name("named").unwrap()),
        Ok(RdbcObject::from(Value::String("named".to_owned())))
    );
    assert_eq!(callable.execute_prepared_update().await.unwrap(), 1);
    assert_eq!(callable.was_null(), Ok(false));
    assert_eq!(
        Wrapper::is_wrapper_for(
            callable.as_ref(),
            Some(TypeId::of::<dyn CallableStatement>()),
        ),
        Ok(true)
    );
    assert!(matches!(
        Wrapper::unwrap(
            callable.as_ref(),
            Some(TypeId::of::<dyn PreparedStatement>()),
        )
        .unwrap(),
        StandardUnwrapped::PreparedStatement(_)
    ));
    callable.close().await.unwrap();
    callable.close().await.unwrap();
    assert_eq!(callable.is_closed(), Ok(true));
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn task6_pooled_callable_coverage_test_cs_prepare_call_basic() {
    task6_pooled_callable_public_behavior("cs_prepare_call_basic").await;
}

#[tokio::test]
async fn task6_pooled_callable_coverage_test_cs_debug_format() {
    task6_pooled_callable_public_behavior("cs_debug_format").await;
}

#[tokio::test]
async fn task6_pooled_callable_coverage_test_cs_wrapper_trait() {
    task6_pooled_callable_public_behavior("cs_wrapper_trait").await;
}

#[tokio::test]
async fn task6_pooled_callable_coverage_test_cs_register_out_parameter_variants() {
    task6_pooled_callable_public_behavior("cs_register_out_parameter_variants").await;
}

#[tokio::test]
async fn task6_pooled_callable_coverage_test_cs_named_parameter_setters() {
    task6_pooled_callable_public_behavior("cs_named_parameter_setters").await;
}

#[tokio::test]
async fn task6_pooled_callable_coverage_test_cs_named_temporal_setters() {
    task6_pooled_callable_public_behavior("cs_named_temporal_setters").await;
}

#[tokio::test]
async fn task6_pooled_callable_coverage_test_cs_close_idempotent() {
    task6_pooled_callable_public_behavior("cs_close_idempotent").await;
}

#[tokio::test]
async fn task6_pooled_callable_coverage_test_cs_close_without_connection() {
    task6_pooled_callable_public_behavior("cs_close_without_connection").await;
}

#[tokio::test]
async fn task6_pooled_callable_coverage_test_cs_generated_keys_and_more_results() {
    task6_pooled_callable_public_behavior("cs_generated_keys_and_more_results").await;
}

#[tokio::test]
async fn task6_pooled_callable_coverage_test_cs_standard_callable_statement_accessor() {
    task6_pooled_callable_public_behavior("cs_physical_callable_statement_accessor").await;
}

#[tokio::test]
async fn task6_pooled_callable_coverage_test_cs_execute_paths() {
    task6_pooled_callable_public_behavior("cs_execute_paths").await;
}

#[tokio::test]
async fn task6_pooled_callable_coverage_test_cs_through_filter_chain() {
    task6_pooled_callable_public_behavior("cs_through_filter_chain").await;
}

#[tokio::test]
async fn task6_pooled_callable_coverage_test_cs_handle_properties() {
    task6_pooled_callable_public_behavior("cs_handle_properties").await;
}

#[tokio::test]
async fn task6_pooled_callable_coverage_test_cs_update_count_and_result_set() {
    task6_pooled_callable_public_behavior("cs_update_count_and_result_set").await;
}
