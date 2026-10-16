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

/// 将同步写操作拆为有界 JDBC Agent RPC。对应 Java: Clob#setAsciiStream。
pub(super) struct JdbcAgentClobAsciiOutputStream {
    commands: Sender<StreamCommand>,
    blocked_runtime_thread: Option<std::thread::ThreadId>,
}

/// 独立等待远端流关闭，失败时允许标准输出流重新关闭。
pub(super) struct JdbcAgentClobAsciiOutputStreamCloser {
    commands: Sender<StreamCommand>,
    blocked_runtime_thread: Option<std::thread::ThreadId>,
}

impl JdbcAgentClobAsciiOutputStream {
    /// 创建流工作线程，返回写入端及可重试关闭端。
    pub(super) fn new(
        executor: Arc<dyn StandardExecutor>,
        stream_id: String,
        runtime: tokio::runtime::Handle,
    ) -> io::Result<(Self, JdbcAgentClobAsciiOutputStreamCloser)> {
        let blocked_runtime_thread = (runtime.runtime_flavor()
            == tokio::runtime::RuntimeFlavor::CurrentThread)
            .then(|| std::thread::current().id());
        let permit = JdbcAgentBlobStreamPermit::acquire()?;
        let (commands, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("druid-jdbc-clob-ascii-output".to_owned())
            .spawn(move || {
                let _permit = permit;
                while let Ok(command) = receiver.recv() {
                    match command {
                        StreamCommand::Write(bytes, reply) => {
                            let result = runtime
                                .block_on(executor.connection_control(
                                    "clob_write_ascii_output",
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
                                            "Clob ASCII write returned no count",
                                        ))?;
                                    let written = usize::try_from(written).map_err(|error| {
                                        io::Error::new(io::ErrorKind::InvalidData, error)
                                    })?;
                                    if written != bytes.len() {
                                        return Err(io::Error::new(
                                            io::ErrorKind::InvalidData,
                                            "Clob ASCII write count differs from request",
                                        ));
                                    }
                                    Ok(written)
                                });
                            let _ = reply.send(result);
                        }
                        StreamCommand::Flush(reply) => {
                            let result = runtime
                                .block_on(executor.connection_control(
                                    "clob_flush_ascii_output",
                                    json!({"streamId": stream_id}),
                                ))
                                .map(|_| ())
                                .map_err(io::Error::other);
                            let _ = reply.send(result);
                        }
                        StreamCommand::Close(reply) => {
                            let result = runtime
                                .block_on(executor.connection_control(
                                    "clob_close_stream",
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
                // 调用方丢弃流时也释放子句柄，父 Clob 和 session 另有兜底清理。
                if !executor.is_closed() {
                    let _ = runtime.block_on(executor.connection_control(
                        "clob_close_stream",
                        json!({"streamId": stream_id}),
                    ));
                }
            })?;
        Ok((
            Self {
                commands: commands.clone(),
                blocked_runtime_thread,
            },
            JdbcAgentClobAsciiOutputStreamCloser {
                commands,
                blocked_runtime_thread,
            },
        ))
    }
}

impl Write for JdbcAgentClobAsciiOutputStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        self.ensure_not_runtime_thread()?;
        let bytes = bytes[..bytes.len().min(MAX_CHUNK_BYTES)].to_vec();
        let (reply, response) = mpsc::channel();
        self.commands
            .send(StreamCommand::Write(bytes, reply))
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Clob ASCII stream worker stopped",
                )
            })?;
        response.recv().map_err(|_| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Clob ASCII stream worker stopped",
            )
        })?
    }

    fn flush(&mut self) -> io::Result<()> {
        self.ensure_not_runtime_thread()?;
        let (reply, response) = mpsc::channel();
        self.commands
            .send(StreamCommand::Flush(reply))
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Clob ASCII stream worker stopped",
                )
            })?;
        response.recv().map_err(|_| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Clob ASCII stream worker stopped",
            )
        })?
    }
}

impl JdbcAgentClobAsciiOutputStream {
    fn ensure_not_runtime_thread(&self) -> io::Result<()> {
        if self.blocked_runtime_thread == Some(std::thread::current().id()) {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "write JDBC Agent Clob ASCII stream with spawn_blocking",
            ))
        } else {
            Ok(())
        }
    }
}

impl JdbcAgentClobAsciiOutputStreamCloser {
    /// 等待远端关闭确认；失败时保留通道供调用方重试。
    pub(super) fn close(&mut self) -> io::Result<()> {
        if self.blocked_runtime_thread == Some(std::thread::current().id()) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "close JDBC Agent Clob ASCII stream with spawn_blocking",
            ));
        }
        let (reply, response) = mpsc::channel();
        self.commands
            .send(StreamCommand::Close(reply))
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Clob ASCII stream worker stopped",
                )
            })?;
        response.recv().map_err(|_| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Clob ASCII stream worker stopped",
            )
        })?
    }
}
