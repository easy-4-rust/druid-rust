use super::jdbc_agent_clob_ascii_input_stream::JdbcAgentClobAsciiInputStream;
use super::jdbc_agent_clob_ascii_output_stream::JdbcAgentClobAsciiOutputStream;
use super::jdbc_agent_clob_reader::JdbcAgentClobReader;
use super::jdbc_agent_clob_utf16::JdbcAgentClobUtf16;
use super::jdbc_agent_clob_writer::JdbcAgentClobWriter;
use super::jdbc_agent_connection::JdbcAgentConnection;
use crate::standard_adapter_executor::StandardExecutor;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{
    Clob, ClobPattern, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcWriter,
    ResourceOperations,
};
use druid_rdbc::types::RdbcString;
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;

/// 单个 JDBC Agent session 持有的远程 SQL Clob。对应 Java: java.sql.Clob。
pub(super) struct JdbcAgentClob {
    executor: Arc<dyn StandardExecutor>,
    clob_id: String,
    freed: bool,
}

impl JdbcAgentClob {
    /// 保存已由 Agent 注册的 Clob 句柄及其串行执行器。
    pub(super) fn new(executor: Arc<dyn StandardExecutor>, clob_id: String) -> Self {
        Self {
            executor,
            clob_id,
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

    fn stream_id(response: &JsonValue, operation: &str) -> Result<String, RdbcError> {
        response
            .get("streamId")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                JdbcAgentConnection::protocol_error(format!("{operation} returned no streamId"))
            })
    }

    async fn close_failed_stream(&self, stream_id: &str) {
        let _ = self
            .executor
            .connection_control("clob_close_stream", json!({"streamId": stream_id}))
            .await;
    }

    fn runtime() -> Result<tokio::runtime::Handle, RdbcError> {
        tokio::runtime::Handle::try_current().map_err(|error| {
            RdbcError::DriverError(format!("Clob stream requires a Tokio runtime: {error}"))
        })
    }

    async fn open_reader(&self, range: Option<(i64, i64)>) -> Result<RdbcReader, RdbcError> {
        let runtime = Self::runtime()?;
        let payload = match range {
            Some((position, length)) => {
                json!({"clobId": self.clob_id, "position": position, "length": length})
            }
            None => json!({"clobId": self.clob_id}),
        };
        let response = self.control("clob_open_reader", payload).await?;
        let id = Self::stream_id(&response, "clob_open_reader")?;
        match JdbcAgentClobReader::new(Arc::clone(&self.executor), id.clone(), runtime) {
            Ok(reader) => Ok(RdbcReader::new(reader)),
            Err(error) => {
                self.close_failed_stream(&id).await;
                Err(RdbcError::DriverError(format!(
                    "Clob reader worker could not start: {error}"
                )))
            }
        }
    }

    async fn open_writer(&self, position: i64) -> Result<RdbcWriter, RdbcError> {
        let runtime = Self::runtime()?;
        let response = self
            .control(
                "clob_open_writer",
                json!({"clobId": self.clob_id, "position": position}),
            )
            .await?;
        let id = Self::stream_id(&response, "clob_open_writer")?;
        match JdbcAgentClobWriter::new(Arc::clone(&self.executor), id.clone(), runtime) {
            Ok(writer) => Ok(RdbcWriter::new(writer)),
            Err(error) => {
                self.close_failed_stream(&id).await;
                Err(RdbcError::DriverError(format!(
                    "Clob writer worker could not start: {error}"
                )))
            }
        }
    }
}

impl std::fmt::Debug for JdbcAgentClob {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JdbcAgentClob")
            .field("clob_id", &self.clob_id)
            .field("freed", &self.freed)
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl Clob for JdbcAgentClob {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ
            | ResourceOperations::WRITE
            | ResourceOperations::RANGE
            | ResourceOperations::STREAM
            | ResourceOperations::FREE
    }

    async fn length(&mut self) -> Result<i64, RdbcError> {
        let response = self
            .control("clob_length", json!({"clobId": self.clob_id}))
            .await?;
        Self::number(&response, "length", "clob_length")
    }

    async fn get_sub_string(
        &mut self,
        position: i64,
        length: i32,
    ) -> Result<RdbcString, RdbcError> {
        let response = self
            .control(
                "clob_get_sub_string",
                json!({"clobId": self.clob_id, "position": position, "length": length}),
            )
            .await?;
        JdbcAgentClobUtf16::decode(&response)
    }

    async fn get_character_stream(&mut self) -> Result<RdbcReader, RdbcError> {
        self.open_reader(None).await
    }

    async fn get_ascii_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        let runtime = Self::runtime()?;
        let response = self
            .control("clob_open_ascii_input", json!({"clobId": self.clob_id}))
            .await?;
        let id = Self::stream_id(&response, "clob_open_ascii_input")?;
        match JdbcAgentClobAsciiInputStream::new(Arc::clone(&self.executor), id.clone(), runtime) {
            Ok(input) => Ok(RdbcInputStream::new(input)),
            Err(error) => {
                self.close_failed_stream(&id).await;
                Err(RdbcError::DriverError(format!(
                    "Clob ASCII input worker could not start: {error}"
                )))
            }
        }
    }

    async fn position_string(
        &mut self,
        pattern: &RdbcString,
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        let response = self.control("clob_position_string", json!({"clobId": self.clob_id, "utf16": JdbcAgentClobUtf16::encode(pattern), "start": start})).await?;
        let position = Self::number(&response, "position", "clob_position_string")?;
        Ok((position >= 0).then_some(position))
    }

    async fn position_clob(
        &mut self,
        _pattern: ClobPattern<'_>,
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        self.ensure_open()?;
        Err(RdbcError::feature_not_supported("Clob#position(Clob,long)"))
    }

    async fn set_string(&mut self, position: i64, value: &RdbcString) -> Result<i32, RdbcError> {
        let response = self.control("clob_set_string", json!({"clobId": self.clob_id, "position": position, "utf16": JdbcAgentClobUtf16::encode(value)})).await?;
        i32::try_from(Self::number(&response, "written", "clob_set_string")?)
            .map_err(|error| JdbcAgentConnection::protocol_error(error.to_string()))
    }

    async fn set_string_range(
        &mut self,
        position: i64,
        value: &RdbcString,
        offset: i32,
        length: i32,
    ) -> Result<i32, RdbcError> {
        let response = self.control("clob_set_string", json!({"clobId": self.clob_id, "position": position, "utf16": JdbcAgentClobUtf16::encode(value), "offset": offset, "length": length})).await?;
        i32::try_from(Self::number(&response, "written", "clob_set_string")?)
            .map_err(|error| JdbcAgentConnection::protocol_error(error.to_string()))
    }

    async fn set_ascii_stream(&mut self, position: i64) -> Result<RdbcOutputStream, RdbcError> {
        let runtime = Self::runtime()?;
        let response = self
            .control(
                "clob_open_ascii_output",
                json!({"clobId": self.clob_id, "position": position}),
            )
            .await?;
        let id = Self::stream_id(&response, "clob_open_ascii_output")?;
        match JdbcAgentClobAsciiOutputStream::new(Arc::clone(&self.executor), id.clone(), runtime) {
            Ok((output, mut closer)) => {
                Ok(RdbcOutputStream::with_close(output, move || closer.close()))
            }
            Err(error) => {
                self.close_failed_stream(&id).await;
                Err(RdbcError::DriverError(format!(
                    "Clob ASCII output worker could not start: {error}"
                )))
            }
        }
    }

    async fn set_character_stream(&mut self, position: i64) -> Result<RdbcWriter, RdbcError> {
        self.open_writer(position).await
    }

    async fn truncate(&mut self, length: i64) -> Result<(), RdbcError> {
        self.control(
            "clob_truncate",
            json!({"clobId": self.clob_id, "length": length}),
        )
        .await?;
        Ok(())
    }

    async fn free(&mut self) -> Result<(), RdbcError> {
        if self.freed {
            return Ok(());
        }
        self.control("free_clob", json!({"clobId": self.clob_id}))
            .await?;
        self.freed = true;
        Ok(())
    }

    async fn get_character_stream_range(
        &mut self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, RdbcError> {
        self.open_reader(Some((position, length))).await
    }
}
