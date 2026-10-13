use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use druid_rdbc::connection::Connection;
use druid_rdbc::driver::{ConnectionProperties, Driver, DriverExtensionDescriptor, DriverRegistry};
use druid_rdbc::error::SqlException;
use druid_rdbc::RdbcError;

/// 驱动协商成功时返回的最小连接对象。
struct ProbeConnection;

druid_rdbc::impl_standard_wrapper!(ProbeConnection => Connection);

#[async_trait::async_trait]
impl Connection for ProbeConnection {}

/// 合同测试驱动的可观察行为。
#[derive(Clone)]
enum Outcome {
    /// 表示驱动无法建立连接。
    Decline,
    /// 表示驱动返回一条 SQL 异常。
    SqlError(SqlException),
    /// 表示驱动建立连接成功。
    Success,
}

/// 保存单个测试驱动的调用计数与执行顺序。
struct DriverProbe {
    id: &'static str,
    accepts: bool,
    outcome: Outcome,
    accept_calls: AtomicUsize,
    name_calls: AtomicUsize,
    connect_calls: AtomicUsize,
    order: Arc<Mutex<Vec<&'static str>>>,
}

/// 对可观察副作用进行计数的标准驱动。
struct ProbeDriver(Arc<DriverProbe>);

#[async_trait::async_trait]
impl Driver for ProbeDriver {
    fn name(&self) -> &str {
        self.0.name_calls.fetch_add(1, Ordering::SeqCst);
        self.0.id
    }

    fn accepts_url(&self, _url: &str) -> bool {
        self.0.accept_calls.fetch_add(1, Ordering::SeqCst);
        self.0.accepts
    }

    async fn connect(
        &self,
        _url: &str,
        _properties: &ConnectionProperties,
    ) -> Result<Option<Box<dyn Connection>>, RdbcError> {
        self.0.connect_calls.fetch_add(1, Ordering::SeqCst);
        self.0.order.lock().unwrap().push(self.0.id);
        match &self.0.outcome {
            Outcome::Decline => Ok(None),
            Outcome::SqlError(error) => Err(RdbcError::SqlException(Box::new(error.clone()))),
            Outcome::Success => Ok(Some(Box::new(ProbeConnection))),
        }
    }
}

/// 注册指定行为的驱动，并返回用于断言副作用的探针。
fn register(
    registry: &mut DriverRegistry,
    id: &'static str,
    accepts: bool,
    outcome: Outcome,
    order: Arc<Mutex<Vec<&'static str>>>,
) -> Arc<DriverProbe> {
    let probe = Arc::new(DriverProbe {
        id,
        accepts,
        outcome,
        accept_calls: AtomicUsize::new(0),
        name_calls: AtomicUsize::new(0),
        connect_calls: AtomicUsize::new(0),
        order,
    });
    let driver: Arc<dyn Driver> = Arc::new(ProbeDriver(Arc::clone(&probe)));
    registry.register(DriverExtensionDescriptor::new(id, driver, None));
    probe
}

/// SQL 异常包含 SQLState、vendor code 和下一异常链。
fn sql_error(code: i32, state: &str, message: &str) -> SqlException {
    SqlException::new(code, Some(state.to_owned()), Some(message.to_owned()))
}

#[tokio::test]
async fn connect_attempts_drivers_without_url_prefilter_and_stops_at_later_success() {
    let order = Arc::new(Mutex::new(Vec::new()));
    let mut registry = DriverRegistry::new();
    let first = register(
        &mut registry,
        "first",
        false,
        Outcome::SqlError(sql_error(11, "HYT00", "first driver timeout")),
        Arc::clone(&order),
    );
    let second = register(
        &mut registry,
        "second",
        false,
        Outcome::Success,
        Arc::clone(&order),
    );
    let third = register(
        &mut registry,
        "third",
        true,
        Outcome::Success,
        Arc::clone(&order),
    );

    let connection = registry
        .connect("jdbc:probe:test", &ConnectionProperties::new())
        .await
        .expect("后续驱动成功时必须建立连接");

    assert!(connection.is_some());
    assert_eq!(*order.lock().unwrap(), ["first", "second"]);
    for probe in [&first, &second, &third] {
        assert_eq!(probe.accept_calls.load(Ordering::SeqCst), 0);
        assert_eq!(probe.name_calls.load(Ordering::SeqCst), 0);
    }
    assert_eq!(third.connect_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn connect_attempts_all_drivers_and_preserves_the_first_sql_exception() {
    let order = Arc::new(Mutex::new(Vec::new()));
    let mut registry = DriverRegistry::new();
    let mut first_error = sql_error(21, "08001", "first failure");
    first_error.set_next_exception(sql_error(22, "08006", "first next exception"));
    register(
        &mut registry,
        "first",
        false,
        Outcome::SqlError(first_error.clone()),
        Arc::clone(&order),
    );
    register(
        &mut registry,
        "second",
        false,
        Outcome::Decline,
        Arc::clone(&order),
    );
    register(
        &mut registry,
        "third",
        false,
        Outcome::SqlError(sql_error(23, "08006", "later failure")),
        Arc::clone(&order),
    );

    let error = registry
        .connect("jdbc:probe:test", &ConnectionProperties::new())
        .await
        .err()
        .expect("所有候选都失败时应返回首个 SQL 异常");

    assert_eq!(*order.lock().unwrap(), ["first", "second", "third"]);
    assert!(matches!(error, RdbcError::SqlException(actual) if *actual == first_error));
}

#[tokio::test]
async fn lookup_uses_accepts_url_without_connecting_or_querying_names() {
    let order = Arc::new(Mutex::new(Vec::new()));
    let mut registry = DriverRegistry::new();
    let first = register(
        &mut registry,
        "first",
        false,
        Outcome::Success,
        Arc::clone(&order),
    );
    let second = register(
        &mut registry,
        "second",
        true,
        Outcome::Success,
        Arc::clone(&order),
    );
    let third = register(
        &mut registry,
        "third",
        true,
        Outcome::Success,
        Arc::clone(&order),
    );

    let selected = registry
        .first_accepted_driver("jdbc:probe:test")
        .expect("应选择首个接受 URL 的驱动");

    assert_eq!(first.accept_calls.load(Ordering::SeqCst), 1);
    assert_eq!(second.accept_calls.load(Ordering::SeqCst), 1);
    assert_eq!(third.accept_calls.load(Ordering::SeqCst), 0);
    assert!(Arc::ptr_eq(
        &selected,
        &registry.descriptors()[1].driver().clone()
    ));
    assert!(order.lock().unwrap().is_empty());
    for probe in [&first, &second, &third] {
        assert_eq!(probe.connect_calls.load(Ordering::SeqCst), 0);
        assert_eq!(probe.name_calls.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn empty_registry_returns_no_connection() {
    let registry = DriverRegistry::new();
    let result = registry
        .connect("jdbc:probe:missing", &ConnectionProperties::new())
        .await
        .expect("空注册表应是无匹配结果");

    assert!(result.is_none());
}
