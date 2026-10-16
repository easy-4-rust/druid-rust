use super::jdbc_agent_blob_stream_permit::JdbcAgentBlobStreamPermit;
use super::jdbc_agent_clob_utf16::JdbcAgentClobUtf16;
use crate::standard_adapter_executor::StandardExecutor;
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::CharacterReader;
use serde_json::{json, Value as JsonValue};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;

const MAX_CHUNK_UNITS: usize = 32 * 1024;

enum StreamCommand {
    Read(usize, Sender<Result<Vec<u16>, RdbcError>>),
    Close(Sender<Result<(), RdbcError>>),
}

/// 将标准 UTF-16 字符读取同步接口桥接至有界 JDBC Agent RPC。
/// 对应 Java: java.sql.Clob#getCharacterStream。
pub(super) struct JdbcAgentClobReader {
    commands: Sender<StreamCommand>,
    blocked_runtime_thread: Option<std::thread::ThreadId>,
}

impl JdbcAgentClobReader {
    /// 创建持有远端 Reader 句柄的工作线程。
    pub(super) fn new(
        executor: Arc<dyn StandardExecutor>,
        stream_id: String,
        runtime: tokio::runtime::Handle,
    ) -> std::io::Result<Self> {
        let blocked_runtime_thread = (runtime.runtime_flavor()
            == tokio::runtime::RuntimeFlavor::CurrentThread)
            .then(|| std::thread::current().id());
        let permit = JdbcAgentBlobStreamPermit::acquire()?;
        let (commands, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("druid-jdbc-clob-reader".to_owned())
            .spawn(move || {
                let _permit = permit;
                while let Ok(command) = receiver.recv() {
                    match command {
                        StreamCommand::Read(max_units, reply) => {
                            let result = runtime
                                .block_on(executor.connection_control(
                                    "clob_read_reader",
                                    json!({"streamId": stream_id, "maxUnits": max_units}),
                                ))
                                .and_then(|response| Self::decode_page(&response, max_units));
                            let _ = reply.send(result);
                        }
                        StreamCommand::Close(reply) => {
                            let result = runtime
                                .block_on(executor.connection_control(
                                    "clob_close_stream",
                                    json!({"streamId": stream_id}),
                                ))
                                .map(|_| ());
                            let succeeded = result.is_ok();
                            let _ = reply.send(result);
                            if succeeded {
                                return;
                            }
                        }
                    }
                }
                if !executor.is_closed() {
                    let _ =
                        runtime.block_on(executor.connection_control(
                            "clob_close_stream",
                            json!({"streamId": stream_id}),
                        ));
                }
            })?;
        Ok(Self {
            commands,
            blocked_runtime_thread,
        })
    }

    fn decode_page(response: &JsonValue, max_units: usize) -> Result<Vec<u16>, RdbcError> {
        let eof = response
            .get("eof")
            .and_then(JsonValue::as_bool)
            .ok_or_else(|| Self::protocol_error("Clob reader page has no EOF flag"))?;
        let value = JdbcAgentClobUtf16::decode(response)?;
        if value.len() > max_units || (value.is_empty() && !eof) || (eof && !value.is_empty()) {
            return Err(Self::protocol_error(
                "Clob reader page violates bound or EOF",
            ));
        }
        Ok(value.as_utf16().to_vec())
    }

    fn protocol_error(message: &str) -> RdbcError {
        RdbcError::DriverError(format!("JDBC Agent protocol error: {message}"))
    }

    fn ensure_not_runtime_thread(&self) -> Result<(), RdbcError> {
        if self.blocked_runtime_thread == Some(std::thread::current().id()) {
            Err(RdbcError::DriverError(
                "read JDBC Agent Clob reader with spawn_blocking".to_owned(),
            ))
        } else {
            Ok(())
        }
    }
}

impl std::fmt::Debug for JdbcAgentClobReader {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JdbcAgentClobReader")
            .finish_non_exhaustive()
    }
}

impl CharacterReader for JdbcAgentClobReader {
    fn read_utf16(&mut self, buffer: &mut [u16]) -> Result<usize, RdbcError> {
        if buffer.is_empty() {
            return Ok(0);
        }
        self.ensure_not_runtime_thread()?;
        let (reply, response) = mpsc::channel();
        self.commands
            .send(StreamCommand::Read(
                buffer.len().min(MAX_CHUNK_UNITS),
                reply,
            ))
            .map_err(|_| Self::protocol_error("Clob reader worker stopped"))?;
        let units = response
            .recv()
            .map_err(|_| Self::protocol_error("Clob reader worker stopped"))??;
        buffer[..units.len()].copy_from_slice(&units);
        Ok(units.len())
    }

    fn close(&mut self) -> Result<(), RdbcError> {
        self.ensure_not_runtime_thread()?;
        let (reply, response) = mpsc::channel();
        self.commands
            .send(StreamCommand::Close(reply))
            .map_err(|_| Self::protocol_error("Clob reader worker stopped"))?;
        response
            .recv()
            .map_err(|_| Self::protocol_error("Clob reader worker stopped"))?
    }
}

impl Drop for JdbcAgentClobReader {
    fn drop(&mut self) {
        if self.blocked_runtime_thread != Some(std::thread::current().id()) {
            let (reply, _response) = mpsc::channel();
            let _ = self.commands.send(StreamCommand::Close(reply));
        }
    }
}
