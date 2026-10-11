use super::libsql_connection_adapter::LibSqlTransport;
use super::LibSqlConnectionAdapter;
use crate::task4_runtime_trace::{
    libsql_trace_session, RuntimeTraceEmitter, TraceExpectation, TraceProbe,
};
use druid_rdbc::error::RdbcError;
use std::collections::HashMap;
use std::sync::Arc;

/// Turso/libSQL 原生远程未池化连接工厂。
///
/// 每次 `create` 都构造独立的 libSQL `Database` 与 `Connection`，`DruidPool`
/// 是唯一池化边界，不复用 libSQL 客户端池。
#[derive(Clone)]
pub struct LibSqlConnectionFactory {
    url: String,
    properties: HashMap<String, String>,
    transport: Option<Arc<dyn LibSqlTransport>>,
    runtime_trace: Option<RuntimeTraceEmitter>,
}

impl std::fmt::Debug for LibSqlConnectionFactory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LibSqlConnectionFactory")
            .field("url", &"<redacted connection URL>")
            .field(
                "property_names",
                &self.properties.keys().collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl LibSqlConnectionFactory {
    /// 创建 Turso/libSQL 工厂。
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            properties: HashMap::new(),
            transport: None,
            runtime_trace: None,
        }
    }

    /// 添加连接属性；令牌使用 `token` 或 `auth_token`。
    #[must_use]
    pub fn property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }

    /// 为唯一 Task 4 evidence test 启用 opaque typed runtime trace。
    ///
    /// 返回只读 probe 与不可构造 expectation；两者只能交给 `verify_and_emit` 消费。
    #[doc(hidden)]
    pub fn enable_task4_runtime_trace(&mut self) -> (TraceProbe, TraceExpectation) {
        let (emitter, probe, expectation) = libsql_trace_session();
        self.runtime_trace = Some(emitter);
        (probe, expectation)
    }

    async fn create_adapter_core(&self) -> Result<LibSqlConnectionAdapter, RdbcError> {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_factory_create();
        }
        if let Some(transport) = &self.transport {
            let mut connection =
                LibSqlConnectionAdapter::from_transport(self.url.clone(), Arc::clone(transport));
            druid_rdbc::connection::Connection::is_valid(&mut connection, 1).await?;
            if let Some(trace) = &self.runtime_trace {
                connection.install_runtime_trace(trace.clone());
                trace.task4_trace_connection_create();
            }
            return Ok(connection);
        }
        let token = self
            .properties
            .get("token")
            .or_else(|| self.properties.get("auth_token"))
            .cloned()
            .unwrap_or_default();
        let mut connection = LibSqlConnectionAdapter::connect(&self.url, token).await?;
        druid_rdbc::connection::Connection::is_valid(&mut connection, 1).await?;
        if let Some(trace) = &self.runtime_trace {
            connection.install_runtime_trace(trace.clone());
            trace.task4_trace_connection_create();
        }
        Ok(connection)
    }

    #[cfg(test)]
    fn with_transport(url: impl Into<String>, transport: Arc<dyn LibSqlTransport>) -> Self {
        Self {
            url: url.into(),
            properties: HashMap::new(),
            transport: Some(transport),
            runtime_trace: None,
        }
    }
}

#[async_trait::async_trait]
impl druid_rdbc::driver::ConnectionFactory for LibSqlConnectionFactory {
    fn connection_url(&self) -> Option<&str> {
        Some(&self.url)
    }

    async fn create(&self) -> Result<Box<dyn druid_rdbc::connection::Connection>, RdbcError> {
        Ok(Box::new(self.create_adapter_core().await?))
    }

    fn name(&self) -> &str {
        "libsql-rs"
    }
}

#[cfg(test)]
mod tests {
    use super::super::libsql_connection_adapter::LibSqlResult;
    use super::*;
    use druid_rdbc::connection::Connection;
    use druid_rdbc::result_set::ColumnIndex;
    use druid_rdbc::statement::{ParameterIndex, PreparedParameter, StatementOptions};
    use druid_rdbc::types::{Row, Value};

    struct DeterministicLibSqlTransport;

    #[async_trait::async_trait]
    impl LibSqlTransport for DeterministicLibSqlTransport {
        async fn request(&self, sql: &str, params: Vec<Value>) -> Result<LibSqlResult, RdbcError> {
            if sql.starts_with("UPDATE") {
                Ok(LibSqlResult {
                    rows: Vec::new(),
                    labels: Vec::new(),
                    rows_affected: 1,
                    last_insert_id: Some(9),
                })
            } else {
                Ok(LibSqlResult {
                    rows: vec![Row::new(vec![params
                        .into_iter()
                        .next()
                        .unwrap_or(Value::Int(1))])],
                    labels: vec!["remote_value".to_owned()],
                    rows_affected: 0,
                    last_insert_id: None,
                })
            }
        }
    }

    #[tokio::test]
    async fn libsql_standard_factory_uses_deterministic_transport_owned_core() {
        let mut factory = LibSqlConnectionFactory::with_transport(
            "libsql://deterministic.invalid",
            Arc::new(DeterministicLibSqlTransport),
        );
        let (trace_probe, trace_expectation) = factory.enable_task4_runtime_trace();
        let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
            .await
            .unwrap();
        let mut statement = connection
            .create_statement(StatementOptions::default())
            .await
            .unwrap();
        assert_eq!(
            statement
                .execute_update(
                    "UPDATE remote_item SET value = 1",
                    druid_rdbc::statement::GeneratedKeys::None,
                )
                .await
                .unwrap(),
            1
        );
        let mut prepared = connection
            .prepare_statement("SELECT ?", StatementOptions::default())
            .await
            .unwrap();
        prepared
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::String("libsql-owned".to_owned()),
            })
            .unwrap();
        let mut rows = prepared.execute_prepared_query().await.unwrap();
        assert!(rows.next().await.unwrap());
        assert_eq!(
            rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::String("libsql-owned".to_owned())
        );
        let snapshot = rows.meta_data().unwrap();
        assert_eq!(
            snapshot
                .column(ColumnIndex::new(1).unwrap())
                .unwrap()
                .label(),
            "remote_value"
        );
        let mut metadata = connection.database_meta_data().await.unwrap();
        assert_eq!(
            metadata.database_product_name().await.unwrap(),
            "Turso/libSQL"
        );
        statement.close().await.unwrap();
        Connection::close(connection.as_mut()).await.unwrap();
        assert!(prepared.clear_parameters().is_err());
        assert!(prepared.execute_prepared_query().await.is_err());
        assert!(metadata.database_product_name().await.is_err());
        assert!(matches!(
            metadata
                .tables(druid_rdbc::metadata::TablesQuery::default())
                .await,
            Err(RdbcError::ConnectionDiscarded)
        ));
        Connection::close(connection.as_mut()).await.unwrap();
        assert_eq!(snapshot.column_count(), 1);
        crate::task4_runtime_trace::verify_and_emit(trace_probe, trace_expectation).unwrap();
    }
}
