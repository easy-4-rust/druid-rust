//! RBDC 物理连接工厂。

use super::rbdc_connection_adapter::RbdcConnectionAdapter;
use crate::task4_runtime_trace::{
    rbdc_trace_session, RuntimeTraceEmitter, TraceExpectation, TraceProbe,
};
use druid_rdbc::error::RdbcError;
use druid_rdbc::error::SqlException;
use druid_rdbc::spi::{PhysicalConnection, PhysicalConnectionFactory};
use std::sync::Arc;

/// RBDC 物理连接工厂。
///
/// 对应 Java: `DruidDataSource#createPhysicalConnection`。
/// Factory 持有可共享 Driver，但每次只创建未池化的 RBDC Connection。
pub struct RbdcConnectionFactory {
    driver: Arc<dyn rbdc::db::Driver>,
    url: String,
    runtime_trace: Option<RuntimeTraceEmitter>,
}

impl RbdcConnectionFactory {
    /// 创建 RBDC 物理连接工厂。
    ///
    /// 参数 `driver` 为具体 RBDC Driver，`url` 为数据库 URL。
    pub fn new(driver: Arc<dyn rbdc::db::Driver>, url: impl Into<String>) -> Self {
        Self {
            driver,
            url: url.into(),
            runtime_trace: None,
        }
    }

    /// 返回连接 URL。
    pub fn url(&self) -> &str {
        &self.url
    }

    /// 返回 RBDC 驱动名称。
    pub fn driver_name(&self) -> &str {
        self.driver.name()
    }

    /// 为唯一 Task 4 evidence test 启用 opaque typed runtime trace。
    ///
    /// 返回只读 probe 与不可构造 expectation；两者只能交给 `verify_and_emit` 消费。
    #[doc(hidden)]
    pub fn enable_task4_runtime_trace(&mut self) -> (TraceProbe, TraceExpectation) {
        let (emitter, probe, expectation) = rbdc_trace_session();
        self.runtime_trace = Some(emitter);
        (probe, expectation)
    }

    async fn create_adapter_core(&self) -> Result<RbdcConnectionAdapter, RdbcError> {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_factory_create();
        }
        let connection = self.driver.connect(&self.url).await.map_err(|error| {
            RdbcError::SqlException(Box::new(
                SqlException::driver(0, error.to_string()).with_class_name("rbdc::Error"),
            ))
        })?;
        let mut connection = RbdcConnectionAdapter::new(connection, self.driver.name());
        if let Some(trace) = &self.runtime_trace {
            connection.install_runtime_trace(trace.clone());
            trace.task4_trace_connection_create();
        }
        Ok(connection)
    }
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for RbdcConnectionFactory {
    fn connection_url(&self) -> Option<&str> {
        Some(&self.url)
    }

    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, RdbcError> {
        Ok(Box::new(self.create_adapter_core().await?))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), RdbcError> {
        connection.ping().await
    }
}

#[async_trait::async_trait]
impl druid_rdbc::driver::ConnectionFactory for RbdcConnectionFactory {
    async fn create(&self) -> Result<Box<dyn druid_rdbc::connection::Connection>, RdbcError> {
        Ok(Box::new(self.create_adapter_core().await?))
    }

    fn name(&self) -> &str {
        self.driver.name()
    }
}
