//! Toasty 未池化 canonical 连接工厂。

use super::ToastyConnectionAdapter;
use crate::task4_runtime_trace::{
    toasty_trace_session, RuntimeTraceEmitter, TraceExpectation, TraceProbe,
};
use druid_rdbc::error::RdbcError;
use druid_rdbc::error::SqlException;
use std::sync::Arc;
use toasty::db::Connect;
use toasty_core::{
    driver::{ConnectContext, Driver},
    schema::app,
    Schema,
};

/// 使用 Toasty Driver API 创建未池化连接。
/// Toasty `Driver`，每次 `create` 直接调用 `Driver#connect`；不会创建或持有
/// `toasty::Db`，所以 `DruidPool` 是唯一的连接池。
#[derive(Clone)]
pub struct ToastyConnectionFactory {
    driver: Arc<dyn Driver>,
    #[cfg(feature = "sqlite")]
    sqlite_driver: Option<Arc<toasty_driver_sqlite::Sqlite>>,
    schema: Arc<Schema>,
    connect_context: ConnectContext,
    url: String,
    driver_name: &'static str,
    max_connections: Option<usize>,
    runtime_trace: Option<RuntimeTraceEmitter>,
}

impl std::fmt::Debug for ToastyConnectionFactory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToastyConnectionFactory")
            .field("url", &self.url)
            .field("driver_name", &self.driver_name)
            .field("max_connections", &self.max_connections)
            .finish_non_exhaustive()
    }
}

impl ToastyConnectionFactory {
    /// 解析 URL 并创建 Toasty canonical 连接工厂。
    ///
    /// URL scheme 必须启用对应 feature。Druid 是 SQL 数据源，因此 `DynamoDB`
    /// 等非 SQL Toasty driver 会被明确拒绝。
    pub async fn new(url: impl Into<String>) -> Result<Self, RdbcError> {
        let url = url.into();
        if url
            .split_once(':')
            .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("dynamodb"))
        {
            // DynamoDB Driver 初始化会读取 AWS 环境；Druid SQL 边界在执行任何
            // 外部初始化前拒绝它，Toasty ORM 本身仍可独立使用该 driver。
            return Err(RdbcError::UnsupportedOperation {
                operation: "toasty_non_sql_connection",
            });
        }
        #[cfg(feature = "sqlite")]
        if url
            .split_once(':')
            .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("sqlite"))
        {
            return Self::from_driver(
                toasty_driver_sqlite::Sqlite::new(url)
                    .map_err(|error| Self::driver_error(&error))?,
            );
        }
        let driver = Connect::new(&url)
            .await
            .map_err(|error| Self::driver_error(&error))?;
        Self::from_driver(driver)
    }

    /// 从已经配置好的 Toasty Driver 创建工厂。
    ///
    /// 该入口用于 `SQLite` 内存驱动或需要厂商参数的 Toasty driver；返回类型不
    /// 暴露第三方连接对象。
    pub fn from_driver(driver: impl Driver) -> Result<Self, RdbcError> {
        let capability = driver.capability();
        capability
            .validate()
            .map_err(|error| Self::driver_error(&error))?;
        if !capability.sql {
            return Err(RdbcError::UnsupportedOperation {
                operation: "toasty_non_sql_connection",
            });
        }

        // Raw SQL 不依赖应用模型，但 Toasty Connection SPI 要求完整 Schema。
        // 空 app schema 经官方 Builder 构造，避免伪造内部对象。
        let schema = Schema::builder()
            .build(app::Schema::default(), capability)
            .map_err(|error| Self::driver_error(&error))?;
        let url = driver.url().into_owned();
        let driver_name = capability.driver_name;
        let max_connections = driver.max_connections().or_else(|| {
            // Toasty 0.9 的 `db::Connect` 会委托 url/capability/connect，却未委托
            // SQLite driver 的 `max_connections()`。内存库每次 connect 都是独立
            // 数据库，因此在 Druid 边界恢复官方 SQLite driver 的单连接约束。
            (driver_name.eq_ignore_ascii_case("sqlite") && url == "sqlite::memory:").then_some(1)
        });

        // 仅识别真实 SQLite 类型，保留配置好的原实例；自定义驱动仍走官方路径。
        let driver = Arc::new(driver);
        #[cfg(feature = "sqlite")]
        let sqlite_driver = {
            let erased: Arc<dyn std::any::Any + Send + Sync> = driver.clone();
            erased.downcast::<toasty_driver_sqlite::Sqlite>().ok()
        };
        Ok(Self {
            driver,
            #[cfg(feature = "sqlite")]
            sqlite_driver,
            schema: Arc::new(schema),
            connect_context: ConnectContext::default(),
            url,
            driver_name,
            max_connections,
            runtime_trace: None,
        })
    }

    /// 返回 Toasty driver URL。
    pub fn url(&self) -> &str {
        &self.url
    }

    /// 返回 Toasty capability 中的驱动名称。
    pub fn driver_name(&self) -> &str {
        self.driver_name
    }

    /// 返回驱动自身的并发连接上限。
    ///
    /// `sqlite::memory:` 为 `Some(1)`；构建 `DruidPool` 时 `max_open` 不得超过
    /// 该值，否则每个物理连接会得到彼此隔离的内存数据库。
    pub fn max_connections(&self) -> Option<usize> {
        self.max_connections
    }

    /// 为唯一 Task 4 evidence test 启用 opaque typed runtime trace。
    ///
    /// 返回只读 probe 与不可构造 expectation；两者只能交给 `verify_and_emit` 消费。
    #[doc(hidden)]
    pub fn enable_task4_runtime_trace(&mut self) -> (TraceProbe, TraceExpectation) {
        let (emitter, probe, expectation) = toasty_trace_session();
        self.runtime_trace = Some(emitter);
        (probe, expectation)
    }

    fn driver_error(error: &toasty_core::Error) -> RdbcError {
        if error.is_connection_lost() {
            RdbcError::ConnectionDiscarded
        } else if error.is_driver_operation_failed() {
            RdbcError::SqlException(Box::new(
                SqlException::driver(0, error.to_string())
                    .with_class_name("toasty_core::error::DriverOperationFailed"),
            ))
        } else {
            RdbcError::DriverError(error.to_string())
        }
    }

    async fn create_adapter_core(&self) -> Result<ToastyConnectionAdapter, RdbcError> {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_factory_create();
        }
        #[cfg(feature = "sqlite")]
        let (connection, query_metadata): (Box<dyn toasty_core::Connection>, _) =
            if let Some(driver) = &self.sqlite_driver {
                let (connection, metadata) = driver
                    .connect_with_query_metadata(&self.connect_context)
                    .map_err(|error| Self::driver_error(&error))?;
                (Box::new(connection), Some(metadata))
            } else {
                (
                    self.driver
                        .connect(&self.connect_context)
                        .await
                        .map_err(|error| Self::driver_error(&error))?,
                    None,
                )
            };
        #[cfg(not(feature = "sqlite"))]
        let connection = self
            .driver
            .connect(&self.connect_context)
            .await
            .map_err(|error| Self::driver_error(&error))?;
        if !connection.is_valid() {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let mut connection = ToastyConnectionAdapter::new(
            connection,
            Arc::clone(&self.schema),
            self.driver.capability(),
            self.url.clone(),
        );
        #[cfg(feature = "sqlite")]
        connection.install_query_metadata(query_metadata);
        if let Some(trace) = &self.runtime_trace {
            connection.install_runtime_trace(trace.clone());
            trace.task4_trace_connection_create();
        }
        Ok(connection)
    }
}

#[async_trait::async_trait]
impl druid_rdbc::driver::ConnectionFactory for ToastyConnectionFactory {
    fn connection_url(&self) -> Option<&str> {
        Some(&self.url)
    }

    async fn create(&self) -> Result<Box<dyn druid_rdbc::connection::Connection>, RdbcError> {
        Ok(Box::new(self.create_adapter_core().await?))
    }

    fn name(&self) -> &str {
        self.driver_name
    }
}
