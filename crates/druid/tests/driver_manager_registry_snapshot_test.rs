use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use druid::core::RdbcError;
use druid::sql::DriverManager;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::{ConnectionProperties, Driver};

/// 注册表快照合同测试返回的最小连接。
struct SnapshotConnection;

druid_rdbc::impl_standard_wrapper!(SnapshotConnection => Connection);

#[async_trait::async_trait]
impl Connection for SnapshotConnection {}

/// 在连接回调中注册候选驱动的探针。
struct RegisterOnConnectDriver {
    late_driver: Arc<dyn Driver>,
    calls: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl Driver for RegisterOnConnectDriver {
    fn name(&self) -> &str {
        "register-on-connect"
    }

    fn accepts_url(&self, _url: &str) -> bool {
        false
    }

    async fn connect(
        &self,
        _url: &str,
        _properties: &ConnectionProperties,
    ) -> Result<Option<Box<dyn Connection>>, RdbcError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        DriverManager::register_driver(Arc::clone(&self.late_driver));
        Ok(None)
    }
}

/// 下一轮尝试才能看见的连接驱动。
struct LateDriver(Arc<AtomicUsize>);

#[async_trait::async_trait]
impl Driver for LateDriver {
    fn name(&self) -> &str {
        "late-driver"
    }

    fn accepts_url(&self, _url: &str) -> bool {
        false
    }

    async fn connect(
        &self,
        _url: &str,
        _properties: &ConnectionProperties,
    ) -> Result<Option<Box<dyn Connection>>, RdbcError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Some(Box::new(SnapshotConnection)))
    }
}

/// 本轮协商固定注册快照，回调内新增驱动从下一次连接开始生效。
#[tokio::test]
async fn registration_during_connect_affects_only_the_next_attempt() {
    let late_calls = Arc::new(AtomicUsize::new(0));
    let late_driver: Arc<dyn Driver> = Arc::new(LateDriver(Arc::clone(&late_calls)));
    let first_calls = Arc::new(AtomicUsize::new(0));
    let first_driver: Arc<dyn Driver> = Arc::new(RegisterOnConnectDriver {
        late_driver: Arc::clone(&late_driver),
        calls: Arc::clone(&first_calls),
    });
    DriverManager::register_driver(Arc::clone(&first_driver));

    let first_attempt = tokio::time::timeout(
        Duration::from_secs(2),
        DriverManager::get_connection("jdbc:registry-snapshot-test://localhost/db"),
    )
    .await
    .expect("驱动回调注册新驱动时不能死锁");
    let first_error = first_attempt
        .err()
        .expect("当前快照没有后续连接时应结束本轮协商");
    assert!(
        matches!(first_error, RdbcError::SqlException(error) if error.sql_state() == Some("08001"))
    );
    assert_eq!(first_calls.load(Ordering::SeqCst), 1);
    assert_eq!(late_calls.load(Ordering::SeqCst), 0);

    let second_attempt =
        DriverManager::get_connection("jdbc:registry-snapshot-test://localhost/db")
            .await
            .expect("下一轮连接应看见新注册驱动");
    drop(second_attempt);
    assert_eq!(first_calls.load(Ordering::SeqCst), 2);
    assert_eq!(late_calls.load(Ordering::SeqCst), 1);

    DriverManager::deregister_driver(&first_driver);
    DriverManager::deregister_driver(&late_driver);
}
