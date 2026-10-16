//! 真实 H2/HSQLDB 通过 RDBC 标准和 Druid 池化路径执行 CallableStatement。
#![cfg(feature = "jdbc-agent")]

use druid::pool::DruidPool;
use druid_rdbc::data_source::PooledConnection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::statement::{
    CallableOutParameter, CallableParameter, ParameterIndex, PreparedParameter, StatementOptions,
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
    connection.close().await.expect("close HSQLDB session");
}
