use super::http_sql_connection_adapter::{HttpSqlTransport, ReqwestHttpSqlTransport};
use super::{HttpSqlConnectionAdapter, HttpSqlProvider};
use crate::task4_runtime_trace::{
    http_sql_trace_session, RuntimeTraceEmitter, TraceExpectation, TraceProbe,
};
use druid_rdbc::error::RdbcError;
use std::collections::HashMap;
use std::sync::Arc;

/// 为每个 Druid holder 创建独立 HTTP SQL 逻辑会话的未池化工厂。
#[derive(Clone)]
pub struct HttpSqlConnectionFactory {
    provider: HttpSqlProvider,
    endpoint: String,
    properties: HashMap<String, String>,
    transport: Arc<dyn HttpSqlTransport>,
    runtime_trace: Option<RuntimeTraceEmitter>,
}

impl HttpSqlConnectionFactory {
    /// 创建 HTTP SQL 工厂。核心建池路径不会发起请求，首次 `create` 才验证服务。
    #[must_use]
    pub fn new(provider: HttpSqlProvider, endpoint: impl Into<String>) -> Self {
        Self {
            provider,
            endpoint: endpoint.into(),
            properties: HashMap::new(),
            transport: Arc::new(ReqwestHttpSqlTransport::new()),
            runtime_trace: None,
        }
    }

    /// 设置鉴权或产品连接属性；Debug 输出只展示属性名。
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
        let (emitter, probe, expectation) = http_sql_trace_session();
        self.runtime_trace = Some(emitter);
        (probe, expectation)
    }

    async fn create_adapter_core(&self) -> Result<HttpSqlConnectionAdapter, RdbcError> {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_factory_create();
        }
        let mut connection = HttpSqlConnectionAdapter::new(
            self.provider,
            self.endpoint.clone(),
            self.properties.clone(),
            Arc::clone(&self.transport),
        )?;
        druid_rdbc::connection::Connection::is_valid(&mut connection, 1).await?;
        if let Some(trace) = &self.runtime_trace {
            connection.install_runtime_trace(trace.clone());
            trace.task4_trace_connection_create();
        }
        Ok(connection)
    }

    #[cfg(test)]
    fn with_transport(
        provider: HttpSqlProvider,
        endpoint: impl Into<String>,
        transport: Arc<dyn HttpSqlTransport>,
    ) -> Self {
        Self {
            provider,
            endpoint: endpoint.into(),
            properties: HashMap::new(),
            transport,
            runtime_trace: None,
        }
    }
}

impl std::fmt::Debug for HttpSqlConnectionFactory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpSqlConnectionFactory")
            .field("provider", &self.provider)
            .field("endpoint", &"<redacted connection URL>")
            .field("property_names", &self.properties.keys())
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl druid_rdbc::driver::ConnectionFactory for HttpSqlConnectionFactory {
    fn connection_url(&self) -> Option<&str> {
        Some(&self.endpoint)
    }

    async fn create(&self) -> Result<Box<dyn druid_rdbc::connection::Connection>, RdbcError> {
        Ok(Box::new(self.create_adapter_core().await?))
    }

    fn name(&self) -> &str {
        "http-sql"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use druid_rdbc::connection::Connection;
    use druid_rdbc::result_set::ColumnIndex;
    use druid_rdbc::statement::{ParameterIndex, PreparedParameter, StatementOptions};
    use druid_rdbc::types::Value;
    use serde_json::{json, Value as JsonValue};

    struct DeterministicHttpSqlTransport;

    #[async_trait::async_trait]
    impl HttpSqlTransport for DeterministicHttpSqlTransport {
        async fn send(
            &self,
            _endpoint: reqwest::Url,
            _properties: &HashMap<String, String>,
            body: JsonValue,
        ) -> Result<(reqwest::StatusCode, JsonValue), String> {
            let statement = body
                .as_array()
                .and_then(|batch| batch.first())
                .and_then(JsonValue::as_array)
                .ok_or_else(|| "missing deterministic statement".to_owned())?;
            let sql = statement
                .first()
                .and_then(JsonValue::as_str)
                .unwrap_or_default();
            let payload = if sql.starts_with("UPDATE") {
                json!({"results": [{"rows_affected": 1}]})
            } else {
                let value = statement.get(1).cloned().unwrap_or_else(|| json!(1));
                json!({"results": [{"columns": ["remote_value"], "values": [[value]]}]})
            };
            Ok((reqwest::StatusCode::OK, payload))
        }
    }

    #[tokio::test]
    async fn http_sql_standard_factory_uses_deterministic_transport_owned_core() {
        let mut factory = HttpSqlConnectionFactory::with_transport(
            HttpSqlProvider::Rqlite,
            "https://deterministic.invalid",
            Arc::new(DeterministicHttpSqlTransport),
        );
        let (trace_probe, trace_expectation) = factory.enable_task4_runtime_trace();
        let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
            .await
            .unwrap();
        assert_eq!(
            connection.capabilities(),
            druid_rdbc::extensions::ConnectionCapabilities::default()
        );
        assert!(matches!(
            connection.set_auto_commit(false).await,
            Err(RdbcError::UnsupportedOperation {
                operation: "connection_set_auto_commit"
            })
        ));
        assert!(matches!(
            connection.set_savepoint_named("unsupported").await,
            Err(RdbcError::UnsupportedOperation {
                operation: "connection_set_savepoint_named"
            })
        ));
        assert!(matches!(
            connection
                .prepare_call("CALL unsupported()", StatementOptions::default())
                .await,
            Err(RdbcError::UnsupportedOperation {
                operation: "connection_prepare_call"
            })
        ));
        match connection.create_array_of(Some("INTEGER"), None).await {
            Err(RdbcError::SqlException(error)) => assert_eq!(error.sql_state(), Some("0A000")),
            Err(error) => panic!("unexpected SQL Array error: {error:?}"),
            Ok(_) => panic!("HTTP SQL unexpectedly created a SQL Array"),
        }
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
                value: Value::String("http-owned".to_owned()),
            })
            .unwrap();
        let mut rows = prepared.execute_prepared_query().await.unwrap();
        assert!(rows.next().await.unwrap());
        assert_eq!(
            rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::String("http-owned".to_owned())
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
        assert_eq!(metadata.database_product_name().await.unwrap(), "rqlite");
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
