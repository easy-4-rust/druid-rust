#![cfg(feature = "jdbc-agent")]

use druid_core::core::{
    PhysicalConnectionFactory, PreparedStatementKey, PreparedStatementMethodType, Value,
};
use druid_wrapper::jdbc_agent::{
    JdbcAgentConnectionFactory, JdbcAgentOptions, JdbcAgentRuntimeMetrics,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
    let metrics_before = JdbcAgentRuntimeMetrics::snapshot();
    let factory = JdbcAgentConnectionFactory::new(
        "jdbc:h2:mem:druid_rust_agent;DB_CLOSE_DELAY=-1",
        Some("SELECT 1".to_owned()),
        options.clone(),
    );
    let mut connection = factory.create().await.expect("必须建立 H2 JDBC 连接");
    let mut shared_runtime_connection = factory
        .create()
        .await
        .expect("第二个 session 必须复用共享 Agent 进程");
    let metrics_open = JdbcAgentRuntimeMetrics::snapshot();
    assert!(metrics_open.process_count() > metrics_before.process_count());
    assert!(metrics_open.active_sessions() >= metrics_before.active_sessions() + 2);

    connection
        .exec(
            "CREATE TABLE sample(id BIGINT PRIMARY KEY, name VARCHAR(32))",
            Vec::new(),
        )
        .await
        .expect("必须建表");
    connection
        .exec(
            "CREATE ALIAS IF NOT EXISTS SLEEP FOR 'java.lang.Thread.sleep(long)'",
            Vec::new(),
        )
        .await
        .expect("必须创建可取消的 H2 慢查询");
    let statement_key = PreparedStatementKey::new(
        Some("INSERT INTO sample(id, name) VALUES (?, ?)".to_owned()),
        connection.catalog().map(ToOwned::to_owned),
        PreparedStatementMethodType::M1,
    )
    .expect("必须创建 PreparedStatement key");
    let statement = connection
        .prepare_physical_statement(&statement_key)
        .await
        .expect("必须在 Agent 端创建真实 PreparedStatement");
    connection
        .exec_prepared(
            statement.as_ref(),
            vec![Value::Int(1), Value::String("druid".to_owned())],
        )
        .await
        .expect("必须按 statementId 绑定并执行远程 PreparedStatement");
    statement.close().expect("必须关闭远程 PreparedStatement");
    connection
        .exec(
            "INSERT INTO sample SELECT X, 'name-' || X FROM SYSTEM_RANGE(2, 1200)",
            Vec::new(),
        )
        .await
        .expect("必须创建跨三页的数据集");
    let rows = connection
        .fetch("SELECT id, name FROM sample ORDER BY id", Vec::new())
        .await
        .expect("必须通过 fetch_page 收集完整查询");
    assert_eq!(rows.len(), 1200);
    assert_eq!(rows[0].values[0], Value::Int(1));
    assert_eq!(rows[0].values[1], Value::String("druid".to_owned()));
    let shared_rows = shared_runtime_connection
        .fetch("SELECT id, name FROM sample", Vec::new())
        .await
        .expect("第二 session 必须访问同一 Agent 进程中的 H2 内存库");
    assert_eq!(shared_rows.len(), 1200);

    assert!(connection.auto_commit());
    connection
        .set_auto_commit(false)
        .await
        .expect("必须更新并缓存 autoCommit");
    assert!(!connection.auto_commit());
    connection
        .set_auto_commit(true)
        .await
        .expect("必须恢复 autoCommit");
    if connection.capabilities().schema {
        connection
            .set_schema("PUBLIC")
            .await
            .expect("H2 必须设置 schema");
        assert_eq!(connection.schema(), Some("PUBLIC"));
    }

    {
        let mut metadata = connection
            .database_meta_data()
            .expect("JDBC Agent 必须暴露握手捕获的真实数据库元数据");
        assert_eq!(
            metadata
                .get_database_product_name()
                .await
                .expect("必须读取数据库产品名")
                .as_deref(),
            Some("H2")
        );
        assert!(metadata
            .get_driver_name()
            .await
            .expect("必须读取 JDBC driver 名称")
            .is_some_and(|name| !name.is_empty()));
    }

    if connection.capabilities().savepoints {
        connection.begin().await.expect("保存点合同必须开始事务");
        let savepoint = connection
            .set_savepoint_named("druid_h2_contract")
            .await
            .expect("H2 必须创建远程命名保存点");
        connection
            .exec("DELETE FROM sample WHERE id = 1", Vec::new())
            .await
            .expect("保存点后的更新必须成功");
        connection
            .rollback_to(&savepoint)
            .await
            .expect("必须按远程 savepointId 回滚");
        connection
            .release_savepoint(&savepoint)
            .await
            .expect("必须释放 Agent session 中的保存点");
        connection.commit().await.expect("保存点事务必须提交");
        connection
            .set_auto_commit(true)
            .await
            .expect("保存点事务后必须恢复 autoCommit");
        assert_eq!(
            connection
                .fetch("SELECT id FROM sample WHERE id = 1", Vec::new())
                .await
                .expect("回滚到保存点后必须可查询")
                .len(),
            1
        );
    }

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
        1200
    );
    connection
        .set_auto_commit(true)
        .await
        .expect("事务完成后必须恢复 autoCommit");
    connection.ping().await.expect("必须验证连接");
    let timeout_factory = JdbcAgentConnectionFactory::new(
        "jdbc:h2:mem:druid_rust_agent;DB_CLOSE_DELAY=-1",
        Some("SELECT 1".to_owned()),
        options.request_timeout(Duration::from_millis(300)),
    );
    let mut timeout_connection = timeout_factory
        .create()
        .await
        .expect("必须为超时契约打开独立 session");
    let metrics_before_timeout = JdbcAgentRuntimeMetrics::snapshot();
    let started = Instant::now();
    let timeout_error = timeout_connection
        .fetch("CALL SLEEP(1000)", Vec::new())
        .await
        .expect_err("慢查询必须触发有界超时和 cancel");
    assert!(started.elapsed() < Duration::from_millis(800));
    assert!(timeout_error.to_string().contains("cancellation requested"));
    let metrics_after_timeout = JdbcAgentRuntimeMetrics::snapshot();
    assert!(metrics_after_timeout.timeout_count() > metrics_before_timeout.timeout_count());
    assert!(
        metrics_after_timeout.cancellation_count() > metrics_before_timeout.cancellation_count()
    );
    tokio::time::sleep(Duration::from_millis(1_050)).await;
    timeout_connection
        .ping()
        .await
        .expect("取消完成后非事务 session 必须仍可验证");
    timeout_connection
        .close()
        .await
        .expect("必须关闭超时测试 session");
    let metrics_after_operations = JdbcAgentRuntimeMetrics::snapshot();
    assert!(metrics_after_operations.rpc_count() > metrics_before.rpc_count());
    assert!(metrics_after_operations.rpc_latency_micros_total() > 0);

    connection
        .exec(
            "CREATE ALIAS IF NOT EXISTS AGENT_EXIT FOR 'java.lang.System.exit(int)'",
            Vec::new(),
        )
        .await
        .expect("必须创建 Agent 崩溃探针");
    let crashes_before = JdbcAgentRuntimeMetrics::snapshot().crash_count();
    connection
        .fetch("CALL AGENT_EXIT(23)", Vec::new())
        .await
        .expect_err("Agent 进程退出必须让当前请求失败");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(connection.is_discarded());
    assert!(shared_runtime_connection.is_discarded());
    shared_runtime_connection
        .fetch("SELECT 1", Vec::new())
        .await
        .expect_err("Agent 崩溃后禁止透明重连现有 session");
    assert!(JdbcAgentRuntimeMetrics::snapshot().crash_count() > crashes_before);
    let _ = shared_runtime_connection.close().await;
    let _ = connection.close().await;
    assert_eq!(
        JdbcAgentRuntimeMetrics::snapshot().active_sessions(),
        metrics_before.active_sessions()
    );
}
