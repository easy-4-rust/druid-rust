use super::jdbc_agent_blob_stream_permit::JdbcAgentBlobStreamPermit;
use crate::standard_adapter_executor::StandardExecutor;
use base64::Engine;
use serde_json::json;
use std::io::{self, Write};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;

const MAX_CHUNK_BYTES: usize = 64 * 1024;

enum StreamCommand {
    Write(Vec<u8>, Sender<io::Result<usize>>),
    Flush(Sender<io::Result<()>>),
    Close(Sender<io::Result<()>>),
}

/// 将同步写操作拆为有界 JDBC Agent RPC。对应 Java: Blob#setBinaryStream。
pub(super) struct JdbcAgentBlobOutputStream {
    commands: Sender<StreamCommand>,
}

/// 独立等待远端流关闭，失败时允许标准输出流重新关闭。
pub(super) struct JdbcAgentBlobOutputStreamCloser {
    commands: Sender<StreamCommand>,
}

impl JdbcAgentBlobOutputStream {
    /// 创建流工作线程，返回写入端及可重试关闭端。
    pub(super) fn new(
        executor: Arc<dyn StandardExecutor>,
        stream_id: String,
        runtime: tokio::runtime::Handle,
    ) -> io::Result<(Self, JdbcAgentBlobOutputStreamCloser)> {
        let permit = JdbcAgentBlobStreamPermit::acquire()?;
        let (commands, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("druid-jdbc-blob-output".to_owned())
            .spawn(move || {
                let _permit = permit;
                while let Ok(command) = receiver.recv() {
                    match command {
                        StreamCommand::Write(bytes, reply) => {
                            let result = runtime
                                .block_on(executor.connection_control(
                                    "blob_write_output_stream",
                                    json!({
                                        "streamId": stream_id,
                                        "bytes": base64::engine::general_purpose::STANDARD.encode(&bytes),
                                    }),
                                ))
                                .map_err(io::Error::other)
                                .and_then(|response| {
                                    let written = response
                                        .get("written")
                                        .and_then(serde_json::Value::as_u64)
                                        .ok_or_else(|| io::Error::new(
                                            io::ErrorKind::InvalidData,
                                            "Blob write returned no count",
                                        ))?;
                                    let written = usize::try_from(written).map_err(|error| {
                                        io::Error::new(io::ErrorKind::InvalidData, error)
                                    })?;
                                    if written != bytes.len() {
                                        return Err(io::Error::new(
                                            io::ErrorKind::InvalidData,
                                            "Blob write count differs from request",
                                        ));
                                    }
                                    Ok(written)
                                });
                            let _ = reply.send(result);
                        }
                        StreamCommand::Flush(reply) => {
                            let result = runtime
                                .block_on(executor.connection_control(
                                    "blob_flush_stream",
                                    json!({"streamId": stream_id}),
                                ))
                                .map(|_| ())
                                .map_err(io::Error::other);
                            let _ = reply.send(result);
                        }
                        StreamCommand::Close(reply) => {
                            let result = runtime
                                .block_on(executor.connection_control(
                                    "blob_close_stream",
                                    json!({"streamId": stream_id}),
                                ))
                                .map(|_| ())
                                .map_err(io::Error::other);
                            let succeeded = result.is_ok();
                            let _ = reply.send(result);
                            if succeeded {
                                return;
                            }
                        }
                    }
                }
                // 调用方丢弃流时也释放子句柄，父 Blob 和 session 另有兜底清理。
                if !executor.is_closed() {
                    let _ = runtime.block_on(executor.connection_control(
                        "blob_close_stream",
                        json!({"streamId": stream_id}),
                    ));
                }
            })?;
        Ok((
            Self {
                commands: commands.clone(),
            },
            JdbcAgentBlobOutputStreamCloser { commands },
        ))
    }
}

impl Write for JdbcAgentBlobOutputStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let bytes = bytes[..bytes.len().min(MAX_CHUNK_BYTES)].to_vec();
        let (reply, response) = mpsc::channel();
        self.commands
            .send(StreamCommand::Write(bytes, reply))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Blob stream worker stopped"))?;
        response
            .recv()
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Blob stream worker stopped"))?
    }

    fn flush(&mut self) -> io::Result<()> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(StreamCommand::Flush(reply))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Blob stream worker stopped"))?;
        response
            .recv()
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Blob stream worker stopped"))?
    }
}

impl JdbcAgentBlobOutputStreamCloser {
    /// 等待远端关闭确认；失败时保留通道供调用方重试。
    pub(super) fn close(&mut self) -> io::Result<()> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(StreamCommand::Close(reply))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Blob stream worker stopped"))?;
        response
            .recv()
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Blob stream worker stopped"))?
    }
}
