//! `DuckDB` 原生 canonical 连接工厂。

use super::DuckDbConnectionAdapter;
use crate::task4_runtime_trace::{
    duckdb_trace_session, RuntimeTraceEmitter, TraceExpectation, TraceProbe,
};
use druid_rdbc::error::RdbcError;
use parking_lot::Mutex;
use std::sync::{Arc, Weak};

/// `DuckDB` 原生未池化连接工厂。
///
/// 同一工厂通过 duckdb-rs 共享数据库句柄并创建独立连接，DruidPool 是唯一池化边界。
#[derive(Clone)]
pub struct DuckDbConnectionFactory {
    url: String,
    runtime_trace: Option<RuntimeTraceEmitter>,
    shared_connection: Arc<tokio::sync::Mutex<Weak<Mutex<duckdb::Connection>>>>,
}

impl std::fmt::Debug for DuckDbConnectionFactory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DuckDbConnectionFactory")
            .field("url", &"<redacted connection URL>")
            .finish_non_exhaustive()
    }
}

impl DuckDbConnectionFactory {
    /// 创建 `DuckDB` 连接工厂。
    ///
    /// 参数 `url` 必须使用 `duckdb:` scheme；内存库使用
    /// `duckdb::memory:`，文件库使用 `duckdb:/absolute/path` 或显式相对路径。
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            runtime_trace: None,
            shared_connection: Arc::new(tokio::sync::Mutex::new(Weak::new())),
        }
    }

    /// 返回配置的 `DuckDB` URL。
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// 为唯一 Task 4 evidence test 启用 opaque typed runtime trace。
    ///
    /// 返回只读 probe 与不可构造 expectation；两者只能交给 `verify_and_emit` 消费。
    #[doc(hidden)]
    pub fn enable_task4_runtime_trace(&mut self) -> (TraceProbe, TraceExpectation) {
        let (emitter, probe, expectation) = duckdb_trace_session();
        self.runtime_trace = Some(emitter);
        (probe, expectation)
    }

    async fn create_adapter_core(&self) -> Result<DuckDbConnectionAdapter, RdbcError> {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_factory_create();
        }
        let mut shared = self.shared_connection.lock().await;
        let mut connection = if let Some(existing) = shared.upgrade() {
            DuckDbConnectionAdapter::connect_from_existing(existing, &self.url).await?
        } else {
            DuckDbConnectionAdapter::connect(&self.url).await?
        };
        *shared = Arc::downgrade(&connection.connection_ref()?);
        if let Some(trace) = &self.runtime_trace {
            connection.install_runtime_trace(trace.clone());
            trace.task4_trace_connection_create();
        }
        Ok(connection)
    }
}

#[async_trait::async_trait]
impl druid_rdbc::driver::ConnectionFactory for DuckDbConnectionFactory {
    fn connection_url(&self) -> Option<&str> {
        Some(&self.url)
    }

    async fn create(&self) -> Result<Box<dyn druid_rdbc::connection::Connection>, RdbcError> {
        Ok(Box::new(self.create_adapter_core().await?))
    }

    fn name(&self) -> &str {
        "duckdb-rs"
    }
}
