#![cfg(feature = "jdbc-agent")]

use druid::core::{PhysicalConnectionFactory, Value};
use druid_wrapper::jdbc_agent::{JdbcAgentConnectionFactory, JdbcAgentOptions};
use std::path::PathBuf;

#[tokio::test]
async fn jdbc_agent_h2_contract_when_configured() {
    let Some(agent_jar) = std::env::var_os("DRUID_JDBC_AGENT_JAR") else {
        return;
    };
    let Some(driver_jar) = std::env::var_os("DRUID_JDBC_DRIVER_JAR") else {
        return;
    };
    let options = JdbcAgentOptions::java(
        "java",
        [PathBuf::from(agent_jar), PathBuf::from(driver_jar)],
    )
    .expect("必须构造跨平台 Java classpath");
    let factory = JdbcAgentConnectionFactory::new(
        "jdbc:h2:mem:druid_rust_agent;DB_CLOSE_DELAY=-1",
        Some("SELECT 1".to_owned()),
        options,
    );
    let mut connection = factory.create().await.expect("必须建立 H2 JDBC 连接");
    let mut shared_runtime_connection = factory
        .create()
        .await
        .expect("第二个 session 必须复用共享 Agent 进程");

    connection
        .exec(
            "CREATE TABLE sample(id BIGINT PRIMARY KEY, name VARCHAR(32))",
            Vec::new(),
        )
        .await
        .expect("必须建表");
    connection
        .exec(
            "INSERT INTO sample(id, name) VALUES (?, ?)",
            vec![Value::Int(1), Value::String("druid".to_owned())],
        )
        .await
        .expect("必须绑定并执行 PreparedStatement");
    let rows = connection
        .fetch("SELECT id, name FROM sample", Vec::new())
        .await
        .expect("必须查询");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values[0], Value::Int(1));
    assert_eq!(rows[0].values[1], Value::String("druid".to_owned()));
    let shared_rows = shared_runtime_connection
        .fetch("SELECT id, name FROM sample", Vec::new())
        .await
        .expect("第二 session 必须访问同一 Agent 进程中的 H2 内存库");
    assert_eq!(shared_rows.len(), 1);

    connection.begin().await.expect("必须开始事务");
    connection
        .exec("DELETE FROM sample", Vec::new())
        .await
        .expect("事务内更新必须成功");
    connection.rollback().await.expect("必须回滚事务");
    assert_eq!(
        connection
            .fetch("SELECT id FROM sample", Vec::new())
            .await
            .expect("回滚后必须可查询")
            .len(),
        1
    );
    connection.ping().await.expect("必须验证连接");
    shared_runtime_connection
        .close()
        .await
        .expect("必须关闭第二个 session");
    connection.close().await.expect("必须关闭连接 session");
}
