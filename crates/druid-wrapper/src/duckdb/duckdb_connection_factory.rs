//! `DuckDB` 原生 canonical 连接工厂。

use super::DuckDbConnectionAdapter;
use crate::task4_runtime_trace::{
    duckdb_trace_session, RuntimeTraceEmitter, TraceExpectation, TraceProbe,
};
use druid_rdbc::error::RdbcError;

/// `DuckDB` 原生未池化连接工厂。
///
/// 每次 `create` 都通过 duckdb-rs 打开一个独立连接，DruidPool 是唯一池化边界。
#[derive(Debug, Clone)]
pub struct DuckDbConnectionFactory {
    url: String,
    runtime_trace: Option<RuntimeTraceEmitter>,
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
        let mut connection = DuckDbConnectionAdapter::connect(&self.url).await?;
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
