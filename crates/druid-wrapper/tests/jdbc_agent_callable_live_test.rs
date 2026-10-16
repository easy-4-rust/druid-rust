//! 真实 H2/HSQLDB 通过 RDBC 标准和 Druid 池化路径执行 CallableStatement。
#![cfg(feature = "jdbc-agent")]

use druid::pool::DruidPool;
use druid_rdbc::data_source::PooledConnection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CurrentResultDisposition,
    ExecutionResult, ParameterIndex, PreparedParameter, StatementOptions,
};
use druid_rdbc::types::Value;
use druid_wrapper::jdbc_agent::{JdbcAgentConnectionFactory, JdbcAgentOptions};
use std::path::PathBuf;
use std::sync::Arc;

fn options(driver: &str) -> Option<JdbcAgentOptions> {
    let agent = std::env::var_os("DRUID_JDBC_AGENT_JAR")?;
    let driver = std::env::var_os(driver)?;
    Some(
        JdbcAgentOptions::java("java", [PathBuf::from(agent), PathBuf::from(driver)])
            .expect("valid Agent classpath"),
    )
}

#[tokio::test]
async fn h2_callable_function_round_trip_and_pooled_owner_close() {
    let Some(options) = options("DRUID_JDBC_DRIVER_JAR") else {
        return;
    };
    let factory: Arc<dyn ConnectionFactory> = Arc::new(JdbcAgentConnectionFactory::new(
        "jdbc:h2:mem:callable_rust;DB_CLOSE_DELAY=-1",
        Some("SELECT 1".to_owned()),
        options.runtime_identity("callable-h2", "local", "isolated"),
    ));
    let pool = DruidPool::builder()
        .name("callable-h2")
        .driver_name("h2")
        .factory(factory)
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("H2 pool");
    let mut owner = pool
        .get_pooled_connection_owner()
        .await
        .expect("pooled owner");
    let mut connection = owner.connection().await.expect("logical connection");
    // H2 的 metadata 不声明完整 stored procedure，但 ABS 函数调用真实可执行。
    assert!(!connection.capabilities().callable_statements);
    let mut callable = connection
        .prepare_call("{? = call ABS(?)}", StatementOptions::default())
        .await
        .expect("H2 function call");
    callable
        .register_out_parameter(
            CallableParameter::by_index(1).unwrap(),
            CallableOutParameter::new(4),
        )
        .expect("OUT registration");
    callable
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(2).unwrap(),
            value: Value::Int(-7),
        })
        .expect("IN parameter");
    callable.execute_prepared().await.expect("execute ABS");
    assert_eq!(
        callable.out_value(ParameterIndex::new(1).unwrap()).unwrap(),
        Value::Int(7)
    );
    assert!(!callable.was_null().unwrap());
    callable
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(2).unwrap(),
            value: Value::Null,
        })
        .expect("null IN");
    callable.execute_prepared().await.expect("execute null ABS");
    assert_eq!(
        callable.out_value(ParameterIndex::new(1).unwrap()).unwrap(),
        Value::Null
    );
    assert!(callable.was_null().unwrap());
    callable
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(2).unwrap(),
            value: Value::String("not-a-number".to_owned()),
        })
        .expect("invalid IN can be staged");
    let failure = callable
        .execute_prepared()
        .await
        .expect_err("invalid numeric IN must retain the JDBC error");
    assert!(failure
        .sql_exception()
        .and_then(|error| error.sql_state())
        .is_some());
    assert!(callable.out_value(ParameterIndex::new(1).unwrap()).is_err());
    callable
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(2).unwrap(),
            value: Value::Int(-8),
        })
        .expect("retry IN");
    callable.execute_prepared().await.expect("retry ABS");
    assert_eq!(
        callable.out_value(ParameterIndex::new(1).unwrap()).unwrap(),
        Value::Int(8)
    );
    owner.close().await.expect("pooled owner close");
    assert!(callable.out_value(ParameterIndex::new(1).unwrap()).is_err());
    pool.close().await;
}

#[tokio::test]
async fn hsqldb_callable_procedure_round_trip_and_explicit_close() {
    let Some(options) = options("DRUID_HSQLDB_DRIVER_JAR") else {
        return;
    };
    let factory = JdbcAgentConnectionFactory::new(
        "jdbc:hsqldb:mem:callable_rust_hsqldb",
        Some("VALUES (1)".to_owned()),
        options.runtime_identity("callable-hsqldb", "local", "isolated"),
    )
    .user_name("SA")
    .password("");
    let mut connection = factory.create().await.expect("HSQLDB session");
    assert!(connection.capabilities().callable_statements);
    connection.exec(
        "CREATE PROCEDURE absolute_value(IN p_input INT, OUT p_output INT) READS SQL DATA BEGIN ATOMIC SET p_output = ABS(p_input); END",
        Vec::new(),
    ).await.expect("create real stored procedure");
    let mut callable = connection
        .prepare_call("CALL absolute_value(?, ?)", StatementOptions::default())
        .await
        .expect("HSQLDB call");
    callable
        .register_out_parameter(
            CallableParameter::by_index(2).unwrap(),
            CallableOutParameter::new(4),
        )
        .expect("OUT registration");
    callable
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::Int(-11),
        })
        .expect("IN parameter");
    callable
        .execute_prepared()
        .await
        .expect("execute procedure");
    assert_eq!(
        callable.out_value(ParameterIndex::new(2).unwrap()).unwrap(),
        Value::Int(11)
    );
    callable.close().await.expect("explicit close");
    assert!(callable.out_value(ParameterIndex::new(2).unwrap()).is_err());
    let mut named = connection
        .prepare_call("CALL absolute_value(?, ?)", StatementOptions::default())
        .await
        .expect("named HSQLDB call");
    named
        .register_out_parameter(
            CallableParameter::by_name("p_output").unwrap(),
            CallableOutParameter::new(4),
        )
        .expect("named OUT registration");
    named
        .set_named_parameter("p_input", CallableInputParameter::Int(-12))
        .expect("named IN parameter");
    named.execute_prepared().await.expect("execute named call");
    assert_eq!(
        named
            .out_parameter(&CallableParameter::by_name("p_output").unwrap())
            .unwrap(),
        druid_rdbc::types::RdbcObject::Scalar(Value::Int(12))
    );
    named.close().await.expect("close named call");
    connection.close().await.expect("close HSQLDB session");
}

#[tokio::test]
async fn hsqldb_callable_preserves_ordered_update_and_two_result_sets() {
    let Some(options) = options("DRUID_HSQLDB_DRIVER_JAR") else {
        return;
    };
    let factory: Arc<dyn ConnectionFactory> = Arc::new(
        JdbcAgentConnectionFactory::new(
            "jdbc:hsqldb:mem:callable_multi_rust",
            Some("VALUES (1)".to_owned()),
            options.runtime_identity("callable-multi-hsqldb", "local", "isolated"),
        )
        .user_name("SA")
        .password(""),
    );
    let pool = DruidPool::builder()
        .name("callable-multi-hsqldb")
        .driver_name("hsqldb")
        .factory(factory)
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("HSQLDB pool");
    let mut owner = pool
        .get_pooled_connection_owner()
        .await
        .expect("pooled owner");
    let mut connection = owner.connection().await.expect("logical connection");
    connection
        .exec("CREATE TABLE callable_values(value INTEGER)", Vec::new())
        .await
        .expect("table");
    connection
        .exec("INSERT INTO callable_values VALUES(7)", Vec::new())
        .await
        .expect("row");
    connection.exec(
        "CREATE PROCEDURE two_results() READS SQL DATA DYNAMIC RESULT SETS 2 BEGIN ATOMIC \
         DECLARE first_result CURSOR WITH RETURN FOR SELECT value FROM callable_values FOR READ ONLY; \
         DECLARE second_result CURSOR WITH RETURN FOR SELECT value + 1 FROM callable_values FOR READ ONLY; \
         OPEN first_result; OPEN second_result; END",
        Vec::new(),
    ).await.expect("two-result procedure");
    let mut callable = connection
        .prepare_call("CALL two_results()", StatementOptions::default())
        .await
        .expect("callable");
    let mut results = callable.execute_prepared().await.expect("execute");
    assert_eq!(results.len(), 3);
    assert!(matches!(results.remove(0), ExecutionResult::UpdateCount(0)));
    assert!(matches!(results.remove(0), ExecutionResult::ResultSet(_)));
    assert!(matches!(results.remove(0), ExecutionResult::ResultSet(_)));
    assert_eq!(callable.get_update_count().unwrap(), Some(0));
    for expected in [7, 8] {
        assert!(callable
            .get_more_results(CurrentResultDisposition::Default)
            .await
            .unwrap());
        let mut current = callable.get_result_set().await.unwrap().unwrap();
        assert!(current.next().await.unwrap());
        assert_eq!(current.int(ColumnIndex::new(1).unwrap()).unwrap(), expected);
    }
    assert!(!callable
        .get_more_results(CurrentResultDisposition::Default)
        .await
        .unwrap());
    assert_eq!(callable.get_update_count().unwrap(), None);
    callable.close().await.expect("close callable");
    owner.close().await.expect("close pooled owner");
    pool.close().await;
}
