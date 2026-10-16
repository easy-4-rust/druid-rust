use super::jdbc_agent_blob_stream_permit::JdbcAgentBlobStreamPermit;
use super::jdbc_agent_clob_utf16::JdbcAgentClobUtf16;
use crate::standard_adapter_executor::StandardExecutor;
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::CharacterWriter;
use druid_rdbc::types::RdbcString;
use serde_json::json;
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;

const MAX_CHUNK_UNITS: usize = 32 * 1024;

enum StreamCommand {
    Write(Vec<u16>, Sender<Result<usize, RdbcError>>),
    Flush(Sender<Result<(), RdbcError>>),
    Close(Sender<Result<(), RdbcError>>),
}

/// 将标准 UTF-16 字符写入同步接口桥接至有界 JDBC Agent RPC。
/// 对应 Java: java.sql.Clob#setCharacterStream。
pub(super) struct JdbcAgentClobWriter {
    commands: Sender<StreamCommand>,
    blocked_runtime_thread: Option<std::thread::ThreadId>,
}

impl JdbcAgentClobWriter {
    /// 创建持有远端 Writer 句柄的工作线程。
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
            .name("druid-jdbc-clob-writer".to_owned())
            .spawn(move || {
                let _permit = permit;
                while let Ok(command) = receiver.recv() {
                    match command {
                        StreamCommand::Write(units, reply) => {
                            let value = RdbcString::from_utf16(units.clone());
                            let result = runtime
                                .block_on(executor.connection_control(
                                    "clob_write_writer",
                                    json!({"streamId": stream_id, "utf16": JdbcAgentClobUtf16::encode(&value)}),
                                ))
                                .and_then(|response| {
                                    let written = response
                                        .get("written")
                                        .and_then(serde_json::Value::as_u64)
                                        .and_then(|count| usize::try_from(count).ok())
                                        .ok_or_else(|| Self::protocol_error("Clob writer returned no count"))?;
                                    if written != units.len() {
                                        return Err(Self::protocol_error("Clob writer count differs from request"));
                                    }
                                    Ok(written)
                                });
                            let _ = reply.send(result);
                        }
                        StreamCommand::Flush(reply) => {
                            let result = runtime
                                .block_on(executor.connection_control(
                                    "clob_flush_writer",
                                    json!({"streamId": stream_id}),
                                ))
                                .map(|_| ());
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
                    let _ = runtime.block_on(executor.connection_control(
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

    fn protocol_error(message: &str) -> RdbcError {
        RdbcError::DriverError(format!("JDBC Agent protocol error: {message}"))
    }

    fn ensure_not_runtime_thread(&self) -> Result<(), RdbcError> {
        if self.blocked_runtime_thread == Some(std::thread::current().id()) {
            Err(RdbcError::DriverError(
                "write JDBC Agent Clob writer with spawn_blocking".to_owned(),
            ))
        } else {
            Ok(())
        }
    }
}

impl std::fmt::Debug for JdbcAgentClobWriter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JdbcAgentClobWriter")
            .finish_non_exhaustive()
    }
}

impl CharacterWriter for JdbcAgentClobWriter {
    fn write_utf16(&mut self, units: &[u16]) -> Result<usize, RdbcError> {
        if units.is_empty() {
            return Ok(0);
        }
        self.ensure_not_runtime_thread()?;
        let (reply, response) = mpsc::channel();
        self.commands
            .send(StreamCommand::Write(
                units[..units.len().min(MAX_CHUNK_UNITS)].to_vec(),
                reply,
            ))
            .map_err(|_| Self::protocol_error("Clob writer worker stopped"))?;
        response
            .recv()
            .map_err(|_| Self::protocol_error("Clob writer worker stopped"))?
    }

    fn flush(&mut self) -> Result<(), RdbcError> {
        self.ensure_not_runtime_thread()?;
        let (reply, response) = mpsc::channel();
        self.commands
            .send(StreamCommand::Flush(reply))
            .map_err(|_| Self::protocol_error("Clob writer worker stopped"))?;
        response
            .recv()
            .map_err(|_| Self::protocol_error("Clob writer worker stopped"))?
    }

    fn close(&mut self) -> Result<(), RdbcError> {
        self.ensure_not_runtime_thread()?;
        let (reply, response) = mpsc::channel();
        self.commands
            .send(StreamCommand::Close(reply))
            .map_err(|_| Self::protocol_error("Clob writer worker stopped"))?;
        response
            .recv()
            .map_err(|_| Self::protocol_error("Clob writer worker stopped"))?
    }
}

impl Drop for JdbcAgentClobWriter {
    fn drop(&mut self) {
        if self.blocked_runtime_thread != Some(std::thread::current().id()) {
            let (reply, _response) = mpsc::channel();
            let _ = self.commands.send(StreamCommand::Close(reply));
        }
    }
}
