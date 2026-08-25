use super::jdbc_agent_connection::{JdbcAgentConnector, NativeJdbcAgentConnector};
use super::{JdbcAgentConnection, JdbcAgentOptions};
use druid_rdbc::error::RdbcError;
use druid_rdbc::spi::{PhysicalConnection, PhysicalConnectionFactory};
use std::collections::HashMap;
use std::sync::Arc;

/// 每次创建一个未池化 JDBC Agent 物理连接的 Druid 工厂。
#[derive(Clone)]
pub struct JdbcAgentConnectionFactory {
    url: String,
    validation_query: Option<String>,
    user_name: Option<String>,
    password: Option<String>,
    properties: HashMap<String, String>,
    options: JdbcAgentOptions,
    connector: Arc<dyn JdbcAgentConnector>,
}

impl JdbcAgentConnectionFactory {
    /// 创建显式配置 Agent 进程的未池化连接工厂。
    #[must_use]
    pub fn new(
        url: impl Into<String>,
        validation_query: Option<String>,
        options: JdbcAgentOptions,
    ) -> Self {
        Self {
            url: url.into(),
            validation_query,
            user_name: None,
            password: None,
            properties: HashMap::new(),
            options,
            connector: Arc::new(NativeJdbcAgentConnector),
        }
    }

    /// 设置 JDBC 用户名。
    #[must_use]
    pub fn user_name(mut self, user_name: impl Into<String>) -> Self {
        self.user_name = Some(user_name.into());
        self
    }

    /// 设置 JDBC 密码；Debug 输出不会包含该值。
    #[must_use]
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// 设置额外 JDBC 连接属性。
    #[must_use]
    pub fn property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }

    async fn create_with_properties(
        &self,
        properties: HashMap<String, String>,
    ) -> Result<Box<dyn PhysicalConnection>, RdbcError> {
        Ok(Box::new(self.create_adapter_core(properties).await?))
    }

    async fn create_adapter_core(
        &self,
        properties: HashMap<String, String>,
    ) -> Result<JdbcAgentConnection, RdbcError> {
        let connection = JdbcAgentConnection::connect_with_connector(
            &self.url,
            self.validation_query.as_deref(),
            properties,
            self.options.clone(),
            Arc::clone(&self.connector),
        )
        .await?;
        Ok(connection)
    }

    #[cfg(test)]
    fn with_connector(
        url: impl Into<String>,
        options: JdbcAgentOptions,
        connector: Arc<dyn JdbcAgentConnector>,
    ) -> Self {
        Self {
            url: url.into(),
            validation_query: None,
            user_name: None,
            password: None,
            properties: HashMap::new(),
            options,
            connector,
        }
    }

    fn merged_properties(
        &self,
        one_shot: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        let mut properties = self.properties.clone();
        if let Some(one_shot) = one_shot {
            properties.extend(one_shot.clone());
        }
        if let Some(user_name) = &self.user_name {
            properties
                .entry("user".to_owned())
                .or_insert_with(|| user_name.clone());
        }
        if let Some(password) = &self.password {
            properties
                .entry("password".to_owned())
                .or_insert_with(|| password.clone());
        }
        properties
    }
}

#[async_trait::async_trait]
impl druid_rdbc::driver::ConnectionFactory for JdbcAgentConnectionFactory {
    async fn create(&self) -> Result<Box<dyn druid_rdbc::connection::Connection>, RdbcError> {
        Ok(Box::new(
            self.create_adapter_core(self.merged_properties(None))
                .await?,
        ))
    }

    fn name(&self) -> &str {
        "jdbc-agent"
    }
}

impl std::fmt::Debug for JdbcAgentConnectionFactory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JdbcAgentConnectionFactory")
            .field("url", &self.url)
            .field("validation_query", &self.validation_query)
            .field("user_name", &self.user_name)
            .field("has_password", &self.password.is_some())
            .field("property_names", &self.properties.keys())
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for JdbcAgentConnectionFactory {
    fn connection_url(&self) -> Option<&str> {
        Some(&self.url)
    }

    fn user_name(&self) -> Option<&str> {
        self.user_name.as_deref()
    }

    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, RdbcError> {
        self.create_with_properties(self.merged_properties(None))
            .await
    }

    async fn create_info_with_properties(
        &self,
        properties: &HashMap<String, String>,
    ) -> Result<druid_rdbc::spi::PhysicalConnectionInfo, RdbcError> {
        let started_at = std::time::Instant::now();
        let connection = self
            .create_with_properties(self.merged_properties(Some(properties)))
            .await?;
        Ok(druid_rdbc::spi::PhysicalConnectionInfo::connected(
            connection, started_at,
        ))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), RdbcError> {
        connection.ping().await
    }
}

#[cfg(test)]
mod tests {
    use super::super::jdbc_agent_connection::JdbcAgentTransport;
    use super::*;
    use druid_rdbc::connection::Connection;
    use druid_rdbc::result_set::ColumnIndex;
    use druid_rdbc::statement::{ParameterIndex, PreparedParameter, StatementOptions};
    use druid_rdbc::types::Value;
    use serde_json::{json, Value as JsonValue};

    struct DeterministicJdbcAgentConnector;

    #[async_trait::async_trait]
    impl JdbcAgentConnector for DeterministicJdbcAgentConnector {
        async fn connect(
            &self,
            _options: JdbcAgentOptions,
            _payload: JsonValue,
        ) -> Result<(Box<dyn JdbcAgentTransport>, JsonValue), RdbcError> {
            Ok((
                Box::new(DeterministicJdbcAgentTransport { closed: false }),
                json!({
                    "sessionId": "deterministic-session",
                    "driverName": "deterministic-jdbc",
                    "databaseProductName": "Deterministic JDBC",
                    "autoCommit": true,
                    "capabilities": {}
                }),
            ))
        }
    }

    struct DeterministicJdbcAgentTransport {
        closed: bool,
    }

    #[async_trait::async_trait]
    impl JdbcAgentTransport for DeterministicJdbcAgentTransport {
        async fn request(
            &mut self,
            operation: &str,
            payload: JsonValue,
        ) -> Result<JsonValue, RdbcError> {
            if self.closed {
                return Err(RdbcError::ConnectionDiscarded);
            }
            match operation {
                "execute" => Ok(json!({
                    "kind": "update",
                    "rowsAffected": 1,
                    "lastInsertId": 9
                })),
                "execute_query" => Ok(json!({
                    "columns": [{"label": "remote_value"}],
                    "rows": [[payload
                        .get("params")
                        .and_then(JsonValue::as_array)
                        .and_then(|params| params.first())
                        .cloned()
                        .unwrap_or_else(|| json!({"kind": "int", "value": 1}))]],
                    "hasMore": false
                })),
                _ => Err(RdbcError::UnsupportedOperation {
                    operation: "deterministic_jdbc_agent_operation",
                }),
            }
        }

        async fn close(&mut self) -> Result<(), RdbcError> {
            self.closed = true;
            Ok(())
        }

        fn is_unusable(&self) -> bool {
            self.closed
        }

        fn statement_context(&self) -> Option<(super::super::AgentRequestHandle, String)> {
            None
        }
    }

    #[tokio::test]
    async fn jdbc_agent_standard_factory_uses_deterministic_runtime_owned_core() {
        let factory = JdbcAgentConnectionFactory::with_connector(
            "jdbc:deterministic:test",
            JdbcAgentOptions::new("deterministic-agent"),
            Arc::new(DeterministicJdbcAgentConnector),
        );
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
                value: Value::String("jdbc-owned".to_owned()),
            })
            .unwrap();
        let mut rows = prepared.execute_prepared_query().await.unwrap();
        assert!(rows.next().await.unwrap());
        assert_eq!(
            rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::String("jdbc-owned".to_owned())
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
            "Deterministic JDBC"
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
    }
}
