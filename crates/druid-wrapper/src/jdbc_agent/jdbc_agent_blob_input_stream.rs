use super::jdbc_agent_blob_stream_permit::JdbcAgentBlobStreamPermit;
use crate::standard_adapter_executor::StandardExecutor;
use base64::Engine;
use serde_json::{json, Value as JsonValue};
use std::io::{self, Read};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;

const MAX_CHUNK_BYTES: usize = 64 * 1024;

enum StreamCommand {
    Read(usize, Sender<io::Result<Vec<u8>>>),
    Close,
}

/// 将同步 RDBC 读取转交给持有异步 JDBC Agent 执行器的工作线程。
/// 对应 Java: java.sql.Blob#getBinaryStream。
pub(super) struct JdbcAgentBlobInputStream {
    commands: Sender<StreamCommand>,
    blocked_runtime_thread: Option<std::thread::ThreadId>,
}

impl JdbcAgentBlobInputStream {
    /// 创建独立流工作线程；参数为执行器、远端句柄及调用方运行时。
    pub(super) fn new(
        executor: Arc<dyn StandardExecutor>,
        stream_id: String,
        runtime: tokio::runtime::Handle,
    ) -> io::Result<Self> {
        let blocked_runtime_thread = (runtime.runtime_flavor()
            == tokio::runtime::RuntimeFlavor::CurrentThread)
            .then(|| std::thread::current().id());
        let permit = JdbcAgentBlobStreamPermit::acquire()?;
        let (commands, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("druid-jdbc-blob-input".to_owned())
            .spawn(move || {
                let _permit = permit;
                while let Ok(command) = receiver.recv() {
                    match command {
                        StreamCommand::Read(max_bytes, reply) => {
                            let result = runtime
                                .block_on(executor.connection_control(
                                    "blob_read_input_stream",
                                    json!({"streamId": stream_id, "maxBytes": max_bytes}),
                                ))
                                .map_err(io::Error::other)
                                .and_then(|response| Self::decode_page(&response, max_bytes));
                            let _ = reply.send(result);
                        }
                        StreamCommand::Close => {
                            if !executor.is_closed() {
                                let _ = runtime.block_on(executor.connection_control(
                                    "blob_close_stream",
                                    json!({"streamId": stream_id}),
                                ));
                            }
                            break;
                        }
                    }
                }
            })?;
        Ok(Self {
            commands,
            blocked_runtime_thread,
        })
    }

    fn decode_page(response: &JsonValue, max_bytes: usize) -> io::Result<Vec<u8>> {
        let encoded = response
            .get("bytes")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Blob page has no bytes"))?;
        let eof = response
            .get("eof")
            .and_then(JsonValue::as_bool)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Blob page has no EOF"))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if bytes.len() > max_bytes || (bytes.is_empty() && !eof) || (eof && !bytes.is_empty()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Blob page violates requested bound or EOF contract",
            ));
        }
        Ok(bytes)
    }
}

impl Read for JdbcAgentBlobInputStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.blocked_runtime_thread == Some(std::thread::current().id()) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "read JDBC Agent Blob stream with spawn_blocking",
            ));
        }
        let (reply, response) = mpsc::channel();
        self.commands
            .send(StreamCommand::Read(
                buffer.len().min(MAX_CHUNK_BYTES),
                reply,
            ))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Blob stream worker stopped"))?;
        let bytes = response.recv().map_err(|_| {
            io::Error::new(io::ErrorKind::BrokenPipe, "Blob stream worker stopped")
        })??;
        buffer[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
}

impl Drop for JdbcAgentBlobInputStream {
    fn drop(&mut self) {
        let _ = self.commands.send(StreamCommand::Close);
    }
}
