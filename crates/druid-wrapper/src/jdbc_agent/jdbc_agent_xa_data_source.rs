use super::jdbc_agent_connection::{JdbcAgentConnector, NativeJdbcAgentConnector};
use super::jdbc_agent_xa_connection::JdbcAgentXaConnection;
use super::JdbcAgentOptions;
use druid_rdbc::data_source::{
    CommonDataSource, RdbcLogWriter, XaDataSource, XaResourceManagerIdentity,
};
use druid_rdbc::error::{RdbcError, SqlException};
use parking_lot::RwLock;
use serde_json::{json, Map, Value as JsonValue};
use std::sync::Arc;

/// 仅使用调用方明确指定的 XADataSource 类和属性创建 JDBC Agent XA session。
/// 对应 Java: `javax.sql.XADataSource`，由 `JdbcAgentConnectionFactory` 的独立 XA 入口承载。
#[derive(Clone)]
pub struct JdbcAgentXaDataSource {
    class_name: String,
    properties: Map<String, JsonValue>,
    validation_query: Option<String>,
    options: JdbcAgentOptions,
    connector: Arc<dyn JdbcAgentConnector>,
    manager_identity: XaResourceManagerIdentity,
    log_writer: Arc<RwLock<Option<RdbcLogWriter>>>,
}

impl JdbcAgentXaDataSource {
    /// 创建显式 XA 数据源；Agent 类名缺失时立即拒绝，不从 JDBC URL 推断。
    pub fn new(
        class_name: impl Into<String>,
        properties: Map<String, JsonValue>,
        options: JdbcAgentOptions,
    ) -> Result<Self, RdbcError> {
        let class_name = class_name.into();
        if class_name.trim().is_empty() {
            return Err(RdbcError::InvalidArgument(
                "XADataSource class name must not be empty".to_owned(),
            ));
        }
        Ok(Self {
            class_name,
            properties,
            validation_query: None,
            options,
            connector: Arc::new(NativeJdbcAgentConnector),
            manager_identity: XaResourceManagerIdentity::new(),
            log_writer: Arc::new(RwLock::new(None)),
        })
    }

    /// 设置 XA session 的连接校验 SQL。
    #[must_use]
    pub fn validation_query(mut self, query: impl Into<String>) -> Self {
        self.validation_query = Some(query.into());
        self
    }

    /// 设置显式 `XADataSource` JavaBean 属性值。
    #[must_use]
    pub fn property(mut self, name: impl Into<String>, value: JsonValue) -> Self {
        self.properties.insert(name.into(), value);
        self
    }

    async fn connect_with_properties(
        &self,
        properties: Map<String, JsonValue>,
    ) -> Result<Box<dyn druid_rdbc::data_source::XaConnection>, RdbcError> {
        let result = self.connect_with_properties_inner(properties).await;
        if let Err(error) = &result {
            if let Err(log_error) = druid_rdbc::data_source::CommonDataSource::write_diagnostic(
                self,
                "xa_connection",
                error,
            ) {
                tracing::warn!(
                    operation = "xa_connection",
                    error_kind = ?log_error.kind(),
                    "XA data-source diagnostic writer failed; preserving the primary error"
                );
            }
        }
        result
    }

    async fn connect_with_properties_inner(
        &self,
        properties: Map<String, JsonValue>,
    ) -> Result<Box<dyn druid_rdbc::data_source::XaConnection>, RdbcError> {
        let (mut transport, session) = self
            .connector
            .connect(
                self.options.clone(),
                json!({
                    "xaDataSourceClassName": self.class_name,
                    "xaDataSourceProperties": JsonValue::Object(properties),
                    "validationQuery": self.validation_query,
                }),
            )
            .await?;
        if session
            .pointer("/capabilities/xa")
            .and_then(JsonValue::as_bool)
            != Some(true)
        {
            if let Err(close_error) = transport.close().await {
                return Err(RdbcError::SqlException(Box::new(
                    SqlException::new(
                        0,
                        Some("0A000".to_owned()),
                        Some(format!(
                            "XA capability was not negotiated; session cleanup failed: {close_error}"
                        )),
                    )
                    .with_class_name("java.sql.SQLFeatureNotSupportedException")
                    .with_assignable_type("java.sql.SQLNonTransientException"),
                )));
            }
            return Err(RdbcError::feature_not_supported("jdbc_agent_xa_session"));
        }
        Ok(Box::new(JdbcAgentXaConnection::new(
            transport,
            session,
            self.manager_identity.clone(),
        )))
    }

    #[cfg(test)]
    fn with_connector(
        class_name: impl Into<String>,
        options: JdbcAgentOptions,
        connector: Arc<dyn JdbcAgentConnector>,
    ) -> Self {
        Self {
            class_name: class_name.into(),
            properties: Map::new(),
            validation_query: None,
            options,
            connector,
            manager_identity: XaResourceManagerIdentity::new(),
            log_writer: Arc::new(RwLock::new(None)),
        }
    }
}

impl std::fmt::Debug for JdbcAgentXaDataSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JdbcAgentXaDataSource")
            .field("class_name", &self.class_name)
            .field("property_count", &self.properties.len())
            .field("has_validation_query", &self.validation_query.is_some())
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

impl CommonDataSource for JdbcAgentXaDataSource {
    fn get_log_writer(&self) -> Option<RdbcLogWriter> {
        self.log_writer.read().clone()
    }

    fn set_log_writer(&self, writer: Option<RdbcLogWriter>) -> Result<(), RdbcError> {
        *self.log_writer.write() = writer;
        Ok(())
    }

    fn get_parent_logger(&self) -> Result<&'static str, RdbcError> {
        Ok("druid.jdbc_agent.xa")
    }
}

#[async_trait::async_trait]
impl XaDataSource for JdbcAgentXaDataSource {
    async fn xa_connection(
        &self,
    ) -> Result<Box<dyn druid_rdbc::data_source::XaConnection>, RdbcError> {
        self.connect_with_properties(self.properties.clone()).await
    }

    async fn xa_connection_with_credentials(
        &self,
        user: &str,
        password: &str,
    ) -> Result<Box<dyn druid_rdbc::data_source::XaConnection>, RdbcError> {
        let mut properties = self.properties.clone();
        properties.insert("user".to_owned(), JsonValue::String(user.to_owned()));
        properties.insert(
            "password".to_owned(),
            JsonValue::String(password.to_owned()),
        );
        self.connect_with_properties(properties).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jdbc_agent::jdbc_agent_connection::JdbcAgentTransport;
    use druid_rdbc::data_source::{XaFlags, Xid};
    use serde_json::Value as JsonValue;
    use std::collections::VecDeque;
    use std::io::Write;
    use std::sync::{Arc as StdArc, Mutex as StdMutex};

    #[test]
    fn xa_data_source_log_writer_configuration_is_shared_across_clones() {
        let (source, _, _) = setup(true);
        let clone = source.clone();
        assert!(source.get_log_writer().is_none());

        let writer: RdbcLogWriter = Arc::new(parking_lot::Mutex::new(Box::new(std::io::sink())));
        source
            .set_log_writer(Some(Arc::clone(&writer)))
            .expect("set XA log writer");
        assert!(Arc::ptr_eq(
            &clone.get_log_writer().expect("clone sees logger"),
            &writer
        ));
        assert_eq!(source.get_parent_logger(), Ok("druid.jdbc_agent.xa"));

        clone.set_log_writer(None).expect("disable XA log writer");
        assert!(source.get_log_writer().is_none());
    }

    #[derive(Clone)]
    struct TestConnector {
        calls: StdArc<StdMutex<Vec<(String, JsonValue)>>>,
        payloads: StdArc<StdMutex<Vec<JsonValue>>>,
        supports_xa: bool,
        close_fails: bool,
        connect_error: Option<RdbcError>,
    }

    #[async_trait::async_trait]
    impl JdbcAgentConnector for TestConnector {
        async fn connect(
            &self,
            _options: JdbcAgentOptions,
            payload: JsonValue,
        ) -> Result<(Box<dyn JdbcAgentTransport>, JsonValue), RdbcError> {
            self.payloads.lock().expect("payloads lock").push(payload);
            if let Some(error) = self.connect_error.clone() {
                return Err(error);
            }
            Ok((
                Box::new(TestTransport {
                    calls: StdArc::clone(&self.calls),
                    closed: false,
                    close_fails: self.close_fails,
                    responses: VecDeque::new(),
                }),
                json!({
                    "capabilities": {"xa": self.supports_xa, "transactions": true},
                    "autoCommit": true,
                    "readOnly": false,
                    "transactionIsolation": 2,
                    "driverName": "test-xa-agent"
                }),
            ))
        }
    }

    struct TestTransport {
        calls: StdArc<StdMutex<Vec<(String, JsonValue)>>>,
        closed: bool,
        close_fails: bool,
        responses: VecDeque<Result<JsonValue, RdbcError>>,
    }

    #[async_trait::async_trait]
    impl JdbcAgentTransport for TestTransport {
        async fn request(
            &mut self,
            operation: &str,
            payload: JsonValue,
        ) -> Result<JsonValue, RdbcError> {
            self.calls
                .lock()
                .expect("calls lock")
                .push((operation.to_owned(), payload));
            if self.closed {
                return Err(RdbcError::ConnectionDiscarded);
            }
            if operation == "xa_start" {
                return Ok(json!({"started": 1}));
            }
            if operation == "execute" {
                return Ok(json!({"kind": "update", "rowsAffected": 1}));
            }
            self.responses.pop_front().unwrap_or_else(|| Ok(json!({})))
        }

        async fn close(&mut self) -> Result<(), RdbcError> {
            self.closed = true;
            self.calls
                .lock()
                .expect("calls lock")
                .push(("close_session".to_owned(), JsonValue::Null));
            if self.close_fails {
                Err(RdbcError::SqlException(Box::new(
                    druid_rdbc::error::SqlException::driver(81, "session close failed")
                        .with_sql_state("08006"),
                )))
            } else {
                Ok(())
            }
        }

        fn is_unusable(&self) -> bool {
            self.closed
        }
    }

    type DataSourceTestSetup = (
        JdbcAgentXaDataSource,
        StdArc<StdMutex<Vec<(String, JsonValue)>>>,
        StdArc<StdMutex<Vec<JsonValue>>>,
    );

    fn setup(supports_xa: bool) -> DataSourceTestSetup {
        let calls = StdArc::new(StdMutex::new(Vec::new()));
        let payloads = StdArc::new(StdMutex::new(Vec::new()));
        let connector = Arc::new(TestConnector {
            calls: StdArc::clone(&calls),
            payloads: StdArc::clone(&payloads),
            supports_xa,
            close_fails: false,
            connect_error: None,
        });
        let source = JdbcAgentXaDataSource::with_connector(
            "org.postgresql.xa.PGXADataSource",
            JdbcAgentOptions::new("test-xa-agent"),
            connector,
        );
        (source, calls, payloads)
    }

    #[tokio::test]
    async fn missing_xa_capability_preserves_0a000_when_session_cleanup_fails() {
        let (mut source, calls, _) = setup(false);
        source.connector = Arc::new(TestConnector {
            calls: StdArc::clone(&calls),
            payloads: StdArc::new(StdMutex::new(Vec::new())),
            supports_xa: false,
            close_fails: true,
            connect_error: None,
        });

        let error = match source.xa_connection().await {
            Ok(_) => panic!("XA must fail closed"),
            Err(error) => error,
        };
        let RdbcError::SqlException(sql_error) = error else {
            panic!("expected SQL exception, got {error:?}");
        };
        assert_eq!(sql_error.sql_state(), Some("0A000"));
        assert!(sql_error
            .message()
            .unwrap_or_default()
            .contains("cleanup failed"));
        assert!(sql_error
            .message()
            .unwrap_or_default()
            .contains("session close failed"));
        assert_eq!(
            calls
                .lock()
                .expect("calls lock")
                .last()
                .map(|(op, _)| op.as_str()),
            Some("close_session")
        );
    }

    #[tokio::test]
    async fn xa_connection_failure_logs_only_safe_context_and_preserves_sql_error() {
        let (mut source, calls, payloads) = setup(true);
        source.connector = Arc::new(TestConnector {
            calls,
            payloads,
            supports_xa: true,
            close_fails: false,
            connect_error: Some(RdbcError::SqlException(Box::new(SqlException::new(
                31,
                Some("08001".to_owned()),
                Some(
                    "jdbc:postgresql://alice:pw123@db/app?token=tok456&api_key=api789&sslpassword=ssl123"
                        .to_owned(),
                ),
            )))),
        });
        let output = StdArc::new(StdMutex::new(Vec::new()));
        source
            .set_log_writer(Some(Arc::new(parking_lot::Mutex::new(Box::new(
                CaptureWriter(StdArc::clone(&output)),
            )))))
            .expect("configure log writer");

        let error = match source
            .xa_connection_with_credentials("alice", "pw123")
            .await
        {
            Ok(_) => panic!("XA connection must fail"),
            Err(error) => error,
        };
        assert_eq!(
            error
                .sql_exception()
                .and_then(|exception| exception.sql_state()),
            Some("08001")
        );
        let diagnostic = String::from_utf8(output.lock().expect("output lock").clone())
            .expect("diagnostic is UTF-8");
        assert!(diagnostic.contains("operation=xa_connection"));
        assert!(diagnostic.contains("sql_state=08001"));
        for secret in ["alice", "pw123", "db/app", "tok456", "api789", "ssl123"] {
            assert!(!diagnostic.contains(secret), "diagnostic leaked {secret}");
        }
    }

    struct CaptureWriter(StdArc<StdMutex<Vec<u8>>>);

    impl Write for CaptureWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("output lock").extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("writer-private-detail"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn xa_log_io_failure_keeps_connection_error_and_configured_properties() {
        let (mut source, calls, payloads) = setup(true);
        source
            .properties
            .insert("loginTimeout".to_owned(), json!(30));
        let primary = RdbcError::SqlException(Box::new(
            SqlException::driver(31, "database unavailable").with_sql_state("08001"),
        ));
        source.connector = Arc::new(TestConnector {
            calls,
            payloads,
            supports_xa: true,
            close_fails: false,
            connect_error: Some(primary.clone()),
        });
        source
            .set_log_writer(Some(Arc::new(parking_lot::Mutex::new(Box::new(
                FailingWriter,
            )))))
            .expect("configure failing writer");

        let error = match source.xa_connection().await {
            Ok(_) => panic!("XA connection must fail"),
            Err(error) => error,
        };
        assert_eq!(error, primary);
        assert_eq!(
            error
                .sql_exception()
                .and_then(|exception| exception.sql_state()),
            Some("08001")
        );
        assert_eq!(source.properties["loginTimeout"], json!(30));
        assert!(source.get_log_writer().is_some());
    }

    #[tokio::test]
    async fn xa_log_io_failure_does_not_change_a_successful_connection() {
        let (source, _, _) = setup(true);
        source
            .set_log_writer(Some(Arc::new(parking_lot::Mutex::new(Box::new(
                FailingWriter,
            )))))
            .expect("configure failing writer");

        let mut connection = source
            .xa_connection()
            .await
            .expect("diagnostic writer must not affect connection success");
        connection.close().await.expect("close XA connection");
    }

    #[tokio::test]
    async fn xa_data_source_uses_only_explicit_xa_class_and_properties() {
        let (source, _, payloads) = setup(true);
        let source = source
            .property("serverName", json!("127.0.0.1"))
            .property("databaseName", json!("rdbc"))
            .validation_query("SELECT 1");
        let mut owner = source.xa_connection().await.unwrap();
        let payload = payloads.lock().expect("payloads lock")[0].clone();
        assert_eq!(
            payload["xaDataSourceClassName"],
            "org.postgresql.xa.PGXADataSource"
        );
        assert_eq!(payload["xaDataSourceProperties"]["serverName"], "127.0.0.1");
        assert_eq!(payload["validationQuery"], "SELECT 1");
        assert!(payload.get("url").is_none());
        assert!(payload.get("properties").is_none());
        owner.close().await.unwrap();
    }

    #[tokio::test]
    async fn logical_close_keeps_xa_owner_live_and_owner_close_invalidates_all_handles() {
        let (source, calls, _) = setup(true);
        let mut owner = source.xa_connection().await.unwrap();
        let mut logical = owner.connection().await.unwrap();
        logical
            .exec("INSERT INTO xa_probe VALUES (1)", Vec::new())
            .await
            .unwrap();
        logical.close().await.unwrap();
        owner
            .xa_resource()
            .unwrap()
            .start(
                &Xid::new(1, vec![1, 2], vec![3]).unwrap(),
                XaFlags::TMNOFLAGS,
            )
            .await
            .unwrap();
        assert!(calls
            .lock()
            .expect("calls lock")
            .iter()
            .any(|(operation, _)| operation == "execute"));
        assert!(calls
            .lock()
            .expect("calls lock")
            .iter()
            .any(|(operation, _)| operation == "close_logical_connection"));
        assert!(calls
            .lock()
            .expect("calls lock")
            .iter()
            .any(|(op, _)| op == "xa_start"));
        let mut active_logical = owner.connection().await.unwrap();
        owner.close().await.unwrap();
        assert!(matches!(
            active_logical
                .exec("INSERT INTO xa_probe VALUES (2)", Vec::new())
                .await,
            Err(RdbcError::ConnectionDiscarded)
        ));
        assert!(owner.xa_resource().is_err());
        assert!(matches!(
            owner.connection().await,
            Err(RdbcError::ConnectionDiscarded)
        ));
    }

    #[tokio::test]
    async fn asking_for_a_new_logical_connection_invalidates_the_previous_generation() {
        let (source, _, _) = setup(true);
        let mut owner = source.xa_connection().await.unwrap();
        let mut first = owner.connection().await.unwrap();
        let mut second = owner.connection().await.unwrap();
        assert!(first.is_closed().unwrap());
        assert!(first.exec("SELECT 1", Vec::new()).await.is_err());
        second.close().await.unwrap();
        owner.close().await.unwrap();
    }

    #[tokio::test]
    async fn explicit_xa_configuration_fails_closed_when_agent_does_not_negotiate_xa() {
        let (source, calls, _) = setup(false);
        let error = match source.xa_connection().await {
            Ok(_) => panic!("missing XA capability must be rejected"),
            Err(error) => error,
        };
        let RdbcError::SqlException(error) = error else {
            panic!("missing XA capability must be reported as SQL feature unsupported");
        };
        assert_eq!(error.sql_state(), Some("0A000"));
        assert!(calls
            .lock()
            .expect("calls lock")
            .iter()
            .any(|(op, _)| op == "close_session"));
    }
}
