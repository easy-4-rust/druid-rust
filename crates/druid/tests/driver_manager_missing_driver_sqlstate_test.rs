use druid::core::RdbcError;
use druid::sql::DriverManager;

/// DriverManager 未找到候选驱动时保留 JDBC 连接异常 SQLState。
#[tokio::test]
async fn missing_driver_uses_sql_state_08001_for_lookup_and_connection() {
    let url = "jdbc:driver-manager-missing-test://localhost/db?password=canary-secret";

    let lookup_error = DriverManager::get_driver(url)
        .err()
        .expect("未注册驱动时查找必须失败");
    assert!(
        matches!(&lookup_error, RdbcError::SqlException(error) if error.sql_state() == Some("08001"))
    );

    let connection_error = DriverManager::get_connection(url)
        .await
        .err()
        .expect("没有驱动返回连接时必须失败");
    assert!(
        matches!(&connection_error, RdbcError::SqlException(error) if error.sql_state() == Some("08001"))
    );
    assert!(!connection_error.to_string().contains(url));
    assert!(!connection_error.to_string().contains("canary-secret"));
}
