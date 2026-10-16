use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use parking_lot::RwLock;

use super::{
    ConnectionProperties, Driver, DriverAction, DriverExtensionDescriptor, DriverRegistry,
};
use crate::connection::Connection;
use crate::data_source::RdbcLogWriter;
use crate::error::{RdbcError, SqlException};

#[derive(Clone)]
struct RegisteredDriver {
    driver: Arc<dyn Driver>,
    action: Option<Arc<dyn DriverAction>>,
}

/// 管理进程级 RDBC 驱动注册、连接协商和诊断输出。
///
/// `get_driver` 依注册顺序查询驱动的 URL 接受能力；连接入口依次调用驱动 `connect`，
/// 首个连接成功即返回，SQL 异常保留首个错误并继续协商。整体登录期限限制一次连接协商。
/// 对应 Java：`java.sql.DriverManager`。
pub struct DriverManager;

impl DriverManager {
    fn registry() -> &'static RwLock<Vec<RegisteredDriver>> {
        static REGISTRY: OnceLock<RwLock<Vec<RegisteredDriver>>> = OnceLock::new();
        REGISTRY.get_or_init(|| RwLock::new(Vec::new()))
    }

    fn login_timeout_seconds() -> &'static AtomicU64 {
        static LOGIN_TIMEOUT_SECONDS: AtomicU64 = AtomicU64::new(0);
        &LOGIN_TIMEOUT_SECONDS
    }

    fn log_writer_slot() -> &'static RwLock<Option<RdbcLogWriter>> {
        static LOG_WRITER: OnceLock<RwLock<Option<RdbcLogWriter>>> = OnceLock::new();
        LOG_WRITER.get_or_init(|| RwLock::new(None))
    }

    /// 注册驱动实例。参数 `driver` 是待注册驱动；同一 `Arc` 实例不会重复插入。
    /// 对应 Java: `DriverManager#registerDriver(Driver)`。
    pub fn register_driver(driver: Arc<dyn Driver>) {
        Self::register_driver_with_action(driver, None);
    }

    /// 注册 `driver` 和可选注销回调 `action`，无返回值。
    /// 成功注销后回调只执行一次；重复注册不会替换原回调。
    /// 对应 Java: `DriverManager#registerDriver(Driver, DriverAction)`。
    pub fn register_driver_with_action(
        driver: Arc<dyn Driver>,
        action: Option<Arc<dyn DriverAction>>,
    ) {
        let mut registry = Self::registry().write();
        if !registry
            .iter()
            .any(|entry| Arc::ptr_eq(&entry.driver, &driver))
        {
            registry.push(RegisteredDriver { driver, action });
        }
    }

    /// 注销参数 `driver` 指定的实例并调用其回调；返回是否移除了同一 `Arc` 实例。
    /// 对应 Java: `DriverManager#deregisterDriver`。
    pub fn deregister_driver(driver: &Arc<dyn Driver>) -> bool {
        let removed = {
            let mut registry = Self::registry().write();
            registry
                .iter()
                .position(|entry| Arc::ptr_eq(&entry.driver, driver))
                .map(|index| registry.remove(index))
        };
        if let Some(entry) = removed {
            if let Some(action) = entry.action {
                action.deregister();
            }
            true
        } else {
            false
        }
    }

    /// 返回按注册顺序排列的驱动快照；后续注册变化不会修改返回的 `Vec`。
    /// 对应 Java: `DriverManager#getDrivers`。
    #[must_use]
    pub fn drivers() -> Vec<Arc<dyn Driver>> {
        Self::registry()
            .read()
            .iter()
            .map(|entry| Arc::clone(&entry.driver))
            .collect()
    }

    /// 返回按注册顺序排列的驱动快照；对应 Java: `DriverManager#getDrivers`。
    #[must_use]
    pub fn get_drivers() -> Vec<Arc<dyn Driver>> {
        Self::drivers()
    }

    /// 查询首个接受参数 `url` 的驱动；无合适驱动时返回 SQL 错误。
    /// 对应 Java: `DriverManager#getDriver`。
    pub fn get_driver(url: &str) -> Result<Arc<dyn Driver>, RdbcError> {
        Self::driver_for(url)
    }

    /// 设置所有连接尝试的登录超时 `seconds`；零表示无限制，不控制语句执行时间。
    /// 对应 Java: `DriverManager#setLoginTimeout`。
    pub fn set_login_timeout(seconds: u64) {
        Self::login_timeout_seconds().store(seconds, Ordering::Release);
    }

    /// 返回全局驱动登录超时秒数；零表示无限制。
    #[must_use]
    pub fn login_timeout() -> u64 {
        Self::login_timeout_seconds().load(Ordering::Acquire)
    }

    /// 返回全局登录超时秒数；对应 Java: `DriverManager#getLoginTimeout`。
    #[must_use]
    pub fn get_login_timeout() -> u64 {
        Self::login_timeout()
    }

    /// 设置进程级诊断输出 `writer`；`None` 禁用输出。
    /// 诊断可能含数据库结构或数据，调用方须控制访问并脱敏。
    /// 对应 Java: `DriverManager#setLogWriter`。
    pub fn set_log_writer(writer: Option<RdbcLogWriter>) {
        *Self::log_writer_slot().write() = writer;
    }

    /// 返回进程级诊断输出；禁用时返回 `None`。
    #[must_use]
    pub fn log_writer() -> Option<RdbcLogWriter> {
        Self::log_writer_slot().read().clone()
    }

    /// 返回进程级诊断输出；对应 Java: `DriverManager#getLogWriter`。
    #[must_use]
    pub fn get_log_writer() -> Option<RdbcLogWriter> {
        Self::log_writer()
    }

    /// 将参数 `message` 写入诊断输出并换行；未配置输出时返回成功。
    /// I/O 失败返回 `RdbcError::Other`；对应 Java: `DriverManager#println`。
    pub fn println(message: &str) -> Result<(), RdbcError> {
        if let Some(writer) = Self::log_writer() {
            use std::io::Write as _;
            writeln!(writer.lock(), "{message}")
                .map_err(|error| RdbcError::Other(format!("RDBC log writer failed: {error}")))?;
        }
        Ok(())
    }

    /// 使用首个接受参数 `url` 的驱动打开非池化连接并返回。
    /// 无驱动、驱动失败和登录超时保留原错误语义。
    /// 对应 Java: `DriverManager#getConnection(String)`。
    pub async fn get_connection(url: &str) -> Result<Box<dyn Connection>, RdbcError> {
        Self::connect_registered(url, &ConnectionProperties::new()).await
    }

    /// 使用参数 `url` 和属性 `info` 打开物理连接并返回。
    /// 属性交给驱动，由驱动处理 URL 内的重复属性。
    /// 对应 Java: `DriverManager#getConnection(String, Properties)`。
    pub async fn get_connection_with_properties(
        url: &str,
        info: &HashMap<String, String>,
    ) -> Result<Box<dyn Connection>, RdbcError> {
        let properties = info
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        Self::connect_registered(url, &properties).await
    }

    /// 使用 `url`、`username` 和 `password` 打开物理连接并返回。
    /// 凭据只传给驱动，不得记录到日志。
    /// 对应 Java: `DriverManager#getConnection(String, String, String)`。
    pub async fn get_connection_with_credentials(
        url: &str,
        username: &str,
        password: &str,
    ) -> Result<Box<dyn Connection>, RdbcError> {
        let properties = ConnectionProperties::from([
            ("user".to_owned(), username.to_owned()),
            ("password".to_owned(), password.to_owned()),
        ]);
        Self::connect_registered(url, &properties).await
    }

    fn driver_for(url: &str) -> Result<Arc<dyn Driver>, RdbcError> {
        // 先复制注册顺序，再交给标准 registry 在锁外执行第三方回调。
        // 查询路径不调用 driver.name()，保留原有 get_driver 副作用边界。
        let mut registry = DriverRegistry::new();
        for driver in Self::drivers() {
            registry.register(DriverExtensionDescriptor::new("", driver, None));
        }
        registry
            .first_accepted_driver(url)
            .ok_or_else(Self::no_suitable_driver_error)
    }

    /// 创建 JDBC 无合适驱动异常；消息不回显可能包含凭据的 URL。
    fn no_suitable_driver_error() -> RdbcError {
        RdbcError::SqlException(Box::new(SqlException::new(
            0,
            Some("08001".to_owned()),
            Some("No suitable driver found".to_owned()),
        )))
    }

    async fn connect_registered(
        url: &str,
        properties: &ConnectionProperties,
    ) -> Result<Box<dyn Connection>, RdbcError> {
        // 兼容层只保存 Java 注册顺序；连接协商委托唯一标准 registry 算法。
        let mut registry = DriverRegistry::new();
        for driver in Self::drivers() {
            // 标准协商只需要 driver 实例；查询无关驱动的名称会引入额外外部回调。
            registry.register(DriverExtensionDescriptor::new("", driver, None));
        }
        Self::with_login_timeout(registry.connect(url, properties)).await
    }

    async fn with_login_timeout<F>(future: F) -> Result<Box<dyn Connection>, RdbcError>
    where
        F: std::future::Future<Output = Result<Option<Box<dyn Connection>>, RdbcError>>,
    {
        let seconds = Self::login_timeout();
        let connection = if seconds == 0 {
            future.await
        } else {
            tokio::time::timeout(Duration::from_secs(seconds), future)
                .await
                .map_err(|_| RdbcError::LoginTimeout)?
        }?;
        connection.ok_or_else(Self::no_suitable_driver_error)
    }
}
