use super::jdbc_agent_blob_input_stream::JdbcAgentBlobInputStream;
use super::jdbc_agent_blob_output_stream::JdbcAgentBlobOutputStream;
use super::jdbc_agent_connection::JdbcAgentConnection;
use crate::standard_adapter_executor::StandardExecutor;
use base64::Engine;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{
    Blob, BlobPattern, RdbcInputStream, RdbcOutputStream, ResourceOperations,
};
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;

/// 单个 JDBC Agent session 持有的远程 SQL Blob。对应 Java: java.sql.Blob。
pub(super) struct JdbcAgentBlob {
    executor: Arc<dyn StandardExecutor>,
    blob_id: String,
    freed: bool,
}

impl JdbcAgentBlob {
    /// 保存已由 Agent 注册的 Blob 句柄；参数为串行执行器和不透明标识。
    pub(super) fn new(executor: Arc<dyn StandardExecutor>, blob_id: String) -> Self {
        Self {
            executor,
            blob_id,
            freed: false,
        }
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.freed || self.executor.is_closed() {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    async fn control(&self, operation: &str, payload: JsonValue) -> Result<JsonValue, RdbcError> {
        self.ensure_open()?;
        self.executor.connection_control(operation, payload).await
    }

    fn number(response: &JsonValue, field: &str, operation: &str) -> Result<i64, RdbcError> {
        response
            .get(field)
            .and_then(JsonValue::as_i64)
            .ok_or_else(|| {
                JdbcAgentConnection::protocol_error(format!("{operation} returned no {field}"))
            })
    }

    async fn open_input_stream(
        &self,
        range: Option<(i64, i64)>,
    ) -> Result<RdbcInputStream, RdbcError> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|error| {
            RdbcError::DriverError(format!("Blob stream requires a Tokio runtime: {error}"))
        })?;
        let payload = match range {
            Some((position, length)) => {
                json!({"blobId": self.blob_id, "position": position, "length": length})
            }
            None => json!({"blobId": self.blob_id}),
        };
        let response = self.control("blob_open_input_stream", payload).await?;
        let stream_id = response
            .get("streamId")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| JdbcAgentConnection::protocol_error("Blob open returned no streamId"))?
            .to_owned();
        match JdbcAgentBlobInputStream::new(Arc::clone(&self.executor), stream_id.clone(), runtime)
        {
            Ok(stream) => Ok(RdbcInputStream::new(stream)),
            Err(error) => {
                let _ = self
                    .executor
                    .connection_control("blob_close_stream", json!({"streamId": stream_id}))
                    .await;
                Err(RdbcError::DriverError(format!(
                    "Blob stream worker could not start: {error}"
                )))
            }
        }
    }

    async fn open_output_stream(&self, position: i64) -> Result<RdbcOutputStream, RdbcError> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|error| {
            RdbcError::DriverError(format!("Blob stream requires a Tokio runtime: {error}"))
        })?;
        let response = self
            .control(
                "blob_open_output_stream",
                json!({"blobId": self.blob_id, "position": position}),
            )
            .await?;
        let stream_id = response
            .get("streamId")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| JdbcAgentConnection::protocol_error("Blob open returned no streamId"))?
            .to_owned();
        match JdbcAgentBlobOutputStream::new(Arc::clone(&self.executor), stream_id.clone(), runtime)
        {
            Ok((writer, mut closer)) => {
                Ok(RdbcOutputStream::with_close(writer, move || closer.close()))
            }
            Err(error) => {
                let _ = self
                    .executor
                    .connection_control("blob_close_stream", json!({"streamId": stream_id}))
                    .await;
                Err(RdbcError::DriverError(format!(
                    "Blob stream worker could not start: {error}"
                )))
            }
        }
    }
}

impl std::fmt::Debug for JdbcAgentBlob {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JdbcAgentBlob")
            .field("blob_id", &self.blob_id)
            .field("freed", &self.freed)
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl Blob for JdbcAgentBlob {
    fn capabilities(&self) -> ResourceOperations {
        // SEARCH、TRUNCATE 在 H2 上会被驱动拒绝。
        ResourceOperations::READ
            | ResourceOperations::WRITE
            | ResourceOperations::RANGE
            | ResourceOperations::STREAM
            | ResourceOperations::FREE
    }

    async fn length(&mut self) -> Result<i64, RdbcError> {
        let response = self
            .control("blob_length", json!({"blobId": self.blob_id}))
            .await?;
        Self::number(&response, "length", "blob_length")
    }

    async fn get_bytes(&mut self, position: i64, length: i32) -> Result<Vec<u8>, RdbcError> {
        let response = self
            .control(
                "blob_get_bytes",
                json!({"blobId": self.blob_id, "position": position, "length": length}),
            )
            .await?;
        let encoded = response
            .get("bytes")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| {
                JdbcAgentConnection::protocol_error("blob_get_bytes returned no bytes")
            })?;
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|error| JdbcAgentConnection::protocol_error(error.to_string()))
    }

    async fn get_binary_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        self.open_input_stream(None).await
    }

    async fn position_bytes(
        &mut self,
        pattern: &[u8],
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        let response = self
            .control(
                "blob_position_bytes",
                json!({
                    "blobId": self.blob_id,
                    "pattern": base64::engine::general_purpose::STANDARD.encode(pattern),
                    "start": start,
                }),
            )
            .await?;
        let position = Self::number(&response, "position", "blob_position_bytes")?;
        Ok((position >= 0).then_some(position))
    }

    async fn position_blob(
        &mut self,
        _pattern: BlobPattern<'_>,
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        self.ensure_open()?;
        Err(RdbcError::feature_not_supported("Blob#position(Blob,long)"))
    }

    async fn set_bytes(&mut self, position: i64, bytes: &[u8]) -> Result<i32, RdbcError> {
        let response = self
            .control(
                "blob_set_bytes",
                json!({
                    "blobId": self.blob_id,
                    "position": position,
                    "bytes": base64::engine::general_purpose::STANDARD.encode(bytes),
                }),
            )
            .await?;
        i32::try_from(Self::number(&response, "written", "blob_set_bytes")?)
            .map_err(|error| JdbcAgentConnection::protocol_error(error.to_string()))
    }

    async fn set_bytes_range(
        &mut self,
        position: i64,
        bytes: &[u8],
        offset: i32,
        length: i32,
    ) -> Result<i32, RdbcError> {
        let response = self
            .control(
                "blob_set_bytes",
                json!({
                    "blobId": self.blob_id,
                    "position": position,
                    "bytes": base64::engine::general_purpose::STANDARD.encode(bytes),
                    "offset": offset,
                    "length": length,
                }),
            )
            .await?;
        i32::try_from(Self::number(&response, "written", "blob_set_bytes")?)
            .map_err(|error| JdbcAgentConnection::protocol_error(error.to_string()))
    }

    async fn set_binary_stream(&mut self, position: i64) -> Result<RdbcOutputStream, RdbcError> {
        self.open_output_stream(position).await
    }

    async fn truncate(&mut self, length: i64) -> Result<(), RdbcError> {
        self.control(
            "blob_truncate",
            json!({"blobId": self.blob_id, "length": length}),
        )
        .await?;
        Ok(())
    }

    async fn free(&mut self) -> Result<(), RdbcError> {
        if self.freed {
            return Ok(());
        }
        self.control("free_blob", json!({"blobId": self.blob_id}))
            .await?;
        self.freed = true;
        Ok(())
    }

    async fn get_binary_stream_range(
        &mut self,
        position: i64,
        length: i64,
    ) -> Result<RdbcInputStream, RdbcError> {
        self.open_input_stream(Some((position, length))).await
    }
}

#[cfg(test)]
mod tests {
    use super::JdbcAgentBlob;
    use crate::standard_adapter_executor::{owned_executor, AdapterExecutionCore};
    use crate::standard_query_result::StandardQueryResult;
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::resource::Blob;
    use druid_rdbc::types::Value;
    use serde_json::{json, Value as JsonValue};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct FreeFailsOnce {
        attempts: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl AdapterExecutionCore for FreeFailsOnce {
        async fn standard_connection_control_core(
            &mut self,
            operation: &str,
            _payload: JsonValue,
        ) -> Result<JsonValue, RdbcError> {
            match operation {
                "blob_length" => Ok(json!({"length": 4})),
                "free_blob" => {
                    if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                        Err(RdbcError::DriverError("driver free failed once".into()))
                    } else {
                        Ok(json!({}))
                    }
                }
                _ => Err(RdbcError::InvalidArgument(operation.to_owned())),
            }
        }

        async fn standard_execute_update_core(
            &mut self,
            _sql: &str,
            _values: Vec<Value>,
        ) -> Result<i64, RdbcError> {
            unreachable!("Blob test never executes SQL")
        }

        async fn standard_execute_query_core(
            &mut self,
            _sql: &str,
            _values: Vec<Value>,
        ) -> Result<StandardQueryResult, RdbcError> {
            unreachable!("Blob test never executes SQL")
        }

        async fn standard_close_connection_core(&mut self) -> Result<(), RdbcError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn failed_driver_free_keeps_blob_handle_retryable() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let executor = owned_executor(
            FreeFailsOnce {
                attempts: Arc::clone(&attempts),
            },
            None,
        );
        let mut blob = JdbcAgentBlob::new(executor, "owned-blob".into());
        assert!(blob.free().await.is_err());
        assert_eq!(blob.length().await.expect("handle retained"), 4);
        blob.free().await.expect("retry succeeds");
        blob.free().await.expect("duplicate release is inert");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert!(blob.length().await.is_err());
    }
}
