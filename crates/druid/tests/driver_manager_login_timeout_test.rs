use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use druid::core::RdbcError;
use druid::sql::DriverManager;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::{ConnectionProperties, Driver};

/// DriverManager 登录期限测试使用的最小连接对象。
struct TimeoutConnection;

druid_rdbc::impl_standard_wrapper!(TimeoutConnection => Connection);

#[async_trait::async_trait]
impl Connection for TimeoutConnection {}

/// 在 connect 中等待并记录尝试次数的驱动。
struct DelayedDriver {
    delay: Duration,
    calls: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl Driver for DelayedDriver {
    fn name(&self) -> &str {
        "delayed-driver"
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
        tokio::time::sleep(self.delay).await;
        Ok(Some(Box::new(TimeoutConnection)))
    }
}

/// DriverManager 整体登录期限到达后取消本轮协商。
#[tokio::test]
async fn overall_login_timeout_stops_before_starting_later_drivers() {
    DriverManager::set_login_timeout(1);
    let first_calls = Arc::new(AtomicUsize::new(0));
    let second_calls = Arc::new(AtomicUsize::new(0));
    let first: Arc<dyn Driver> = Arc::new(DelayedDriver {
        delay: Duration::from_secs(30),
        calls: Arc::clone(&first_calls),
    });
    let second: Arc<dyn Driver> = Arc::new(DelayedDriver {
        delay: Duration::from_millis(1),
        calls: Arc::clone(&second_calls),
    });
    DriverManager::register_driver(Arc::clone(&first));
    DriverManager::register_driver(Arc::clone(&second));

    let error = DriverManager::get_connection("jdbc:login-timeout-test://localhost/db")
        .await
        .err()
        .expect("整体登录期限到达时必须返回错误");

    assert!(matches!(error, RdbcError::LoginTimeout));
    assert_eq!(first_calls.load(Ordering::SeqCst), 1);
    assert_eq!(second_calls.load(Ordering::SeqCst), 0);

    DriverManager::deregister_driver(&first);
    DriverManager::deregister_driver(&second);
    DriverManager::set_login_timeout(0);
}
