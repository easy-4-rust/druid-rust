// Generated from the pinned Task 6 BASE test denominator.

async fn verify_timeout_and_cancel_if_configured(
    profile: &druid_wrapper::driver::DatabaseProfile,
    url: &str,
) -> bool {
    let long_query = match std::env::var("DRUID_CONTRACT_LONG_QUERY")
        .ok()
        .filter(|value| !value.is_empty())
    {
        Some(value) => value,
        None if std::env::var_os("DRUID_CONTRACT_EVIDENCE_PATH").is_some() => {
            panic!("生成认证证据必须设置 DRUID_CONTRACT_LONG_QUERY")
        }
        None => return false,
    };
    let registry = configured_registry(profile.runtime_mode());
    let config = configured_connection_config(profile.id().as_str(), url);
    let resolved = registry.resolve(&config).expect("合同档案必须可解析");

    let mut timeout_connection = resolved
        .factory()
        .create()
        .await
        .expect("timeout 合同必须建立物理连接");
    let mut timeout_statement = timeout_connection
        .prepare_statement(&long_query, StatementOptions::default())
        .await
        .expect("timeout 合同必须 prepare");
    timeout_statement
        .set_query_timeout(1)
        .expect("canonical PreparedStatement 必须接受 query timeout");
    let timeout_error = match tokio::time::timeout(
        Duration::from_secs(15),
        timeout_statement.execute_prepared_query(),
    )
    .await
    .expect("驱动 query timeout 必须在合同截止时间内完成")
    {
        Ok(_) => panic!("长查询必须被 query timeout 中断"),
        Err(error) => error,
    };
    assert!(
        matches!(timeout_error, RdbcError::SqlException(ref exception)
            if matches!(exception.sql_state(), Some("HYT00" | "HY008" | "57014"))),
        "timeout 必须保留标准 SQLState，实际为 {timeout_error:?}"
    );

    let mut cancel_connection = resolved
        .factory()
        .create()
        .await
        .expect("cancel 合同必须建立独立物理连接");
    let mut cancel_statement = cancel_connection
        .prepare_statement(&long_query, StatementOptions::default())
        .await
        .expect("cancel 合同必须 prepare");
    cancel_statement
        .cancel()
        .expect("显式 cancel 请求必须可发送");
    let task = tokio::spawn(async move { cancel_statement.execute_prepared_query().await });
    let cancel_error = match tokio::time::timeout(Duration::from_secs(15), task)
        .await
        .expect("显式 cancel 必须在合同截止时间内完成")
        .expect("cancel 合同任务不得 panic")
    {
        Ok(_) => panic!("长查询必须被显式 cancel 中断"),
        Err(error) => error,
    };
    assert!(
        matches!(cancel_error, RdbcError::SqlException(ref exception)
            if matches!(exception.sql_state(), Some("HY008" | "57014" | "HYT00")))
            || matches!(cancel_error, RdbcError::ConnectionDiscarded),
        "cancel 必须产生可分类异常或明确丢弃，实际为 {cancel_error:?}"
    );
    true
}

fn configured_registry(runtime_mode: DriverRuntimeMode) -> DruidDriverRegistry {
    #[cfg(feature = "jdbc-agent")]
    {
        let registry = DruidDriverRegistry::builtin().expect("内置数据库档案必须可解析");
        if runtime_mode == DriverRuntimeMode::JdbcAgent {
            return registry.with_jdbc_agent(configured_jdbc_agent_options());
        }
        registry
    }
    #[cfg(not(feature = "jdbc-agent"))]
    {
        assert_ne!(
            runtime_mode,
            DriverRuntimeMode::JdbcAgent,
            "JDBC 产品合同必须启用 jdbc-agent feature"
        );
        DruidDriverRegistry::builtin().expect("内置数据库档案必须可解析")
    }
}

#[cfg(feature = "jdbc-agent")]
async fn verify_jdbc_agent_failures_if_required(
    profile: &druid_wrapper::driver::DatabaseProfile,
    url: &str,
) -> bool {
    if profile.runtime_mode() != DriverRuntimeMode::JdbcAgent {
        return true;
    }
    let connect = || {
        JdbcAgentConnection::connect(
            url,
            profile.validation_query(),
            configured_properties(),
            configured_jdbc_agent_options().contract_fault_injection(true),
        )
    };

    let mut crash = connect()
        .await
        .expect("agent-crash 合同必须建立隔离 JDBC session");
    tokio::time::timeout(Duration::from_secs(10), crash.diagnostic_crash_agent())
        .await
        .expect("Agent 崩溃必须在截止时间内传播")
        .expect_err("diagnostic_crash 必须终止 Agent");
    assert!(
        matches!(crash.ping().await, Err(RdbcError::ConnectionDiscarded)),
        "Agent 崩溃后 canonical 连接必须拒绝复用"
    );

    let mut protocol = connect()
        .await
        .expect("protocol-failure 合同必须启动新的健康 Agent");
    tokio::time::timeout(
        Duration::from_secs(10),
        protocol.diagnostic_protocol_failure(),
    )
    .await
    .expect("协议坏帧必须在截止时间内传播")
    .expect_err("diagnostic_protocol_failure 必须失败关闭");
    assert!(
        matches!(protocol.ping().await, Err(RdbcError::ConnectionDiscarded)),
        "Agent 协议失败后 canonical 连接必须拒绝复用"
    );
    true
}

#[cfg(not(feature = "jdbc-agent"))]
async fn verify_jdbc_agent_failures_if_required(
    profile: &druid_wrapper::driver::DatabaseProfile,
    _url: &str,
) -> bool {
    assert_ne!(
        profile.runtime_mode(),
        DriverRuntimeMode::JdbcAgent,
        "JDBC 产品合同必须启用 jdbc-agent feature"
    );
    true
}

#[cfg(feature = "duckdb-native")]
async fn verify_duckdb_timeout_and_cancel(connection: &mut druid::core::DruidPooledConnection) {
    let sql = "SELECT SUM(a.i * b.i) FROM range(1000000) AS a(i), range(1000000) AS b(i)";
    let mut statement = connection
        .prepare_statement(sql, StatementOptions::default())
        .await
        .expect("DuckDB 长查询必须可 prepare");
    statement
        .set_query_timeout(1)
        .expect("池化 PreparedStatement 必须把 timeout 委托给原生语句");
    let timeout =
        match tokio::time::timeout(Duration::from_secs(10), statement.execute_prepared_query())
            .await
            .expect("DuckDB query timeout 必须在测试截止时间内完成")
        {
            Ok(_) => panic!("DuckDB 长查询必须触发 query timeout"),
            Err(error) => error,
        };
    assert!(matches!(
        timeout,
        RdbcError::SqlException(ref exception)
            if exception.sql_state() == Some("HYT00")
                && exception.class_name() == "java.sql.SQLTimeoutException"
    ));
    eprintln!("contract-stage: duckdb-query-timeout-observed");
    statement.close().await.expect("超时后的池化语句必须可关闭");
    eprintln!("contract-stage: duckdb-timeout-statement-closed");
    connection
        .ping()
        .await
        .expect("query timeout 后连接必须可复用");
    eprintln!("contract-stage: duckdb-timeout-connection-reused");

    let mut physical = DuckDbConnectionAdapter::connect("duckdb::memory:")
        .await
        .expect("取消契约的未池化 DuckDB 连接必须打开");
    let mut executing = physical
        .prepare_statement(sql, StatementOptions::default())
        .await
        .expect("取消契约必须可 prepare");
    // 两个 canonical statement 共享同一 owned executor；一个执行，另一个发出取消请求。
    let mut cancel = physical
        .prepare_statement(sql, StatementOptions::default())
        .await
        .expect("取消契约必须创建共享 executor 的控制语句");
    let task = tokio::spawn(async move { executing.execute_prepared_query().await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    eprintln!("contract-stage: duckdb-explicit-cancel-start");
    cancel
        .cancel()
        .expect("显式 cancel 必须调用 DuckDB 原生 interrupt");
    eprintln!("contract-stage: duckdb-explicit-cancel-returned");
    let cancelled = match tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .expect("显式 cancel 必须在测试截止时间内完成")
        .expect("显式 cancel 任务不得 panic")
    {
        Ok(_) => panic!("DuckDB 长查询必须被显式 cancel"),
        Err(error) => error,
    };
    assert!(matches!(
        cancelled,
        RdbcError::SqlException(ref exception)
            if exception.sql_state() == Some("HY008")
                && exception.class_name() == "duckdb::Error::OperationInterrupted"
    ));
}

#[cfg(not(feature = "duckdb-native"))]
async fn verify_duckdb_timeout_and_cancel(_connection: &mut druid::core::DruidPooledConnection) {
    panic!("DuckDB 完整契约必须启用 duckdb-native feature");
}

async fn verify_embedded_database_restart(
    profile_id: &str,
    url: &str,
    table: &str,
    select_sql: &str,
) {
    assert!(
        url != "duckdb::memory:" && url != "duckdb:///:memory:" && !url.contains(":memory:"),
        "embedded database-restart 契约必须使用文件数据库"
    );
    let pool = DruidDatabasePoolBuilder::new(profile_id, url)
        .name(format!("contract-{profile_id}-restart"))
        .pool(|builder| builder.max_open(1).max_idle(1).test_on_borrow(true))
        .build()
        .await
        .expect("嵌入式文件数据库必须能重新启动");
    let mut connection = pool.get().await.expect("重启后必须能建立物理连接");
    assert_eq!(
        connection
            .fetch(select_sql, vec![Value::Int(4)])
            .await
            .expect("数据库重启后必须能读取已持久化的 batch 数据")
            .len(),
        1
    );
    connection
        .exec(&format!("DROP TABLE {table}"), Vec::new())
        .await
        .expect("数据库重启后必须能清理契约表");
    connection.close().await.expect("重启后的连接必须归还");
    pool.close().await;
    assert!(pool.state().closed);
}

async fn verify_database_restart_if_configured(
    profile: &druid_wrapper::driver::DatabaseProfile,
    url: &str,
    table: &str,
    select_sql: &str,
    embedded_restart: bool,
) -> bool {
    if embedded_restart {
        verify_embedded_database_restart(profile.id().as_str(), url, table, select_sql).await;
        return true;
    }

    let restart_program =
        std::env::var_os("DRUID_CONTRACT_RESTART_PROGRAM").filter(|value| !value.is_empty());
    if let Some(program) = restart_program.as_ref() {
        let arguments = std::env::var("DRUID_CONTRACT_RESTART_ARGS_JSON")
            .ok()
            .filter(|value| !value.is_empty())
            .map(|value| {
                serde_json::from_str::<Vec<String>>(&value)
                    .expect("DRUID_CONTRACT_RESTART_ARGS_JSON 必须是字符串数组")
            })
            .unwrap_or_default();
        let mut command = tokio::process::Command::new(program);
        command.args(arguments).kill_on_drop(true);
        let status = tokio::time::timeout(Duration::from_secs(120), command.status())
            .await
            .expect("数据库重启 hook 必须在截止时间内完成")
            .expect("数据库重启 hook 必须可执行");
        assert!(status.success(), "数据库重启 hook 必须成功");
        eprintln!("contract-stage: restart-hook-complete");
    } else if std::env::var_os("DRUID_CONTRACT_EVIDENCE_PATH").is_some() {
        panic!("生成认证证据必须设置 DRUID_CONTRACT_RESTART_PROGRAM");
    }

    // 重启后的原生建连探针把数据库/URL 故障与 DruidPool 借用故障区分开。
    eprintln!("contract-stage: restart-direct-connect");
    let registry = configured_registry(profile.runtime_mode());
    let config = configured_connection_config(profile.id().as_str(), url);
    let resolved = registry.resolve(&config).expect("重启后档案必须可解析");
    let mut direct_connection =
        tokio::time::timeout(Duration::from_secs(10), resolved.factory().create())
            .await
            .expect("数据库重启后原生建连必须在截止时间内完成")
            .expect("数据库重启后原生建连必须成功");
    tokio::time::timeout(Duration::from_secs(10), direct_connection.ping())
        .await
        .expect("数据库重启后原生 ping 必须在截止时间内完成")
        .expect("数据库重启后原生 ping 必须成功");
    tokio::time::timeout(Duration::from_secs(10), direct_connection.close())
        .await
        .expect("数据库重启后原生关闭必须在截止时间内完成")
        .expect("数据库重启后原生连接必须关闭");

    eprintln!("contract-stage: restart-pool-build");
    let pool = tokio::time::timeout(
        Duration::from_secs(30),
        configured_pool_builder(profile.id().as_str(), url, profile.runtime_mode())
            .name(format!("contract-{}-restart", profile.id()))
            .pool(|builder| {
                builder
                    .max_open(1)
                    .max_idle(1)
                    .test_on_borrow(true)
                    .login_timeout(5)
            })
            .build(),
    )
    .await
    .expect("数据库重启后建池必须在截止时间内完成")
    .expect("数据库重启后必须能重新构建 DruidPool");
    eprintln!("contract-stage: restart-pool-get");
    let acquisition = tokio::time::timeout(
        Duration::from_secs(30),
        pool.get_timeout(Duration::from_secs(10)),
    )
    .await
    .expect("数据库重启后借用必须在截止时间内完成");
    let mut connection = match acquisition {
        Ok(connection) => connection,
        Err(error) => {
            eprintln!(
                "contract-stage: restart-pool-get-error kind={:?} last-create-kind={:?} state={:?}",
                std::mem::discriminant(&error),
                pool.last_create_error()
                    .as_ref()
                    .map(std::mem::discriminant),
                pool.state()
            );
            panic!("数据库重启后必须能重新建连");
        }
    };
    eprintln!("contract-stage: restart-ping");
    tokio::time::timeout(Duration::from_secs(30), connection.ping())
        .await
        .expect("数据库重启后 ping 必须在截止时间内完成")
        .expect("数据库重启后 ping 必须成功");
    eprintln!("contract-stage: restart-fetch");
    assert_eq!(
        connection
            .fetch(select_sql, vec![Value::Int(4)])
            .await
            .expect("数据库重启后必须读取已提交 batch 数据")
            .len(),
        1
    );
    connection
        .exec(&format!("DROP TABLE {table}"), Vec::new())
        .await
        .expect("数据库重启后必须清理契约表");
    connection.close().await.expect("重启后的连接必须归还");
    eprintln!("contract-stage: restart-pool-close");
    pool.close().await;
    restart_program.is_some()
}

fn current_target() -> String {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu".to_owned(),
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu".to_owned(),
        ("x86_64", "macos") => "x86_64-apple-darwin".to_owned(),
        ("aarch64", "macos") => "aarch64-apple-darwin".to_owned(),
        ("x86_64", "windows") => "x86_64-pc-windows-msvc".to_owned(),
        (arch, os) => format!("{arch}-{os}"),
    }
}

fn contract_table_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("druid_contract_{}_{}", std::process::id(), nanos)
}
