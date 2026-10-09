use super::ordinary_prepared_call_projection::OrdinaryPreparedCallProjection;
use super::standard_test_support::{TrackingConnection, TrackingState};
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{
    CallableParameter, CallableStatement, ParameterIndex, PreparedParameter, PreparedStatement,
    Statement, StatementOptions,
};
use druid_rdbc::types::Value;
use druid_rdbc::wrapper::Wrapper;
use std::any::TypeId;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// 验证准入控制持有真实普通句柄：角色、参数、执行和幂等关闭来自同一后端。
/// 对应 Java: DruidPooledConnection#prepareCall 非 Callable 句柄拒绝前的真实创建。
#[tokio::test]
async fn ordinary_prepared_projection_retains_backend_execution_and_close() {
    let state = TrackingState::configured();
    let mut connection = TrackingConnection::new(Arc::clone(&state));
    let raw = connection
        .prepare_statement("ordinary-handle", StatementOptions::default())
        .await
        .unwrap();
    let mut projected = OrdinaryPreparedCallProjection::new(raw);
    assert!(
        Wrapper::is_wrapper_for(&projected, Some(TypeId::of::<dyn PreparedStatement>())).unwrap()
    );
    assert!(Wrapper::is_wrapper_for(&projected, Some(TypeId::of::<dyn Statement>())).unwrap());
    assert!(
        !Wrapper::is_wrapper_for(&projected, Some(TypeId::of::<dyn CallableStatement>())).unwrap()
    );
    assert_eq!(state.prepared_close_count.load(Ordering::Acquire), 0);
    projected.set_query_timeout(9).unwrap();
    assert_eq!(projected.query_timeout().unwrap(), 9);
    let parameter = PreparedParameter {
        index: ParameterIndex::new(1).unwrap(),
        value: Value::Int(17),
    };
    projected.set_parameter(parameter.clone()).unwrap();
    let mut rows = projected.execute_prepared_query().await.unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(22)
    );
    rows.close().await.unwrap();
    assert_eq!(
        state
            .prepared_execution_parameters
            .lock()
            .unwrap()
            .as_slice(),
        &[vec![parameter]]
    );
    assert!(matches!(
        projected.out_parameter(&CallableParameter::by_index(1).unwrap()),
        Err(RdbcError::UnsupportedOperation {
            operation: "ordinary_prepared_out_parameter"
        })
    ));
    projected.close().await.unwrap();
    projected.close().await.unwrap();
    assert!(projected.is_closed().unwrap());
    assert_eq!(
        state.prepared_close_attempt_count.load(Ordering::Acquire),
        1
    );
    assert_eq!(state.prepared_close_count.load(Ordering::Acquire), 1);
    assert!(state
        .event_snapshot()
        .iter()
        .any(|event| event == "prepared.query:ordinary-handle"));
    connection.close().await.unwrap();
}
