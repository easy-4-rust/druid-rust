use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::ColumnDescriptor;
use druid_rdbc::types::Row;
use futures::{Stream, TryStreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch};

/// 一次标准 next 请求；同一游标最多保存一个未消费响应。
/// 对应 Java: java.sql.ResultSet#next 的异步驱动适配。
pub(super) struct SqlxCursorRequest {
    pub(super) reply: oneshot::Sender<Result<Option<Row>, RdbcError>>,
}

impl SqlxCursorRequest {
    /// 在持有同一物理连接的任务内按需推进；无请求时绝不轮询 SQLx stream。
    /// 参数为列描述、启动响应、容量为一的请求通道及驱动流；返回结束或驱动错误。
    pub(super) async fn serve<S>(
        columns: Vec<ColumnDescriptor>,
        startup: &mut Option<oneshot::Sender<Result<Vec<ColumnDescriptor>, RdbcError>>>,
        requests: &mut mpsc::Receiver<Self>,
        stop: &mut watch::Receiver<bool>,
        parent_stop: &mut watch::Receiver<bool>,
        statement_stop: &mut watch::Receiver<bool>,
        interrupted: Arc<AtomicBool>,
        rows: S,
    ) -> Result<(), RdbcError>
    where
        S: Stream<Item = Result<Row, RdbcError>> + Send,
    {
        if *stop.borrow() || *parent_stop.borrow() || *statement_stop.borrow() {
            interrupted.store(true, Ordering::Release);
            return Err(RdbcError::ConnectionDiscarded);
        }
        if startup
            .take()
            .is_none_or(|sender| sender.send(Ok(columns)).is_err())
        {
            return Ok(());
        }
        futures::pin_mut!(rows);
        loop {
            let request = tokio::select! {
                biased;
                _ = stop.changed() => {
                    interrupted.store(true, Ordering::Release);
                    return Err(RdbcError::ConnectionDiscarded);
                }
                _ = parent_stop.changed() => {
                    interrupted.store(true, Ordering::Release);
                    return Err(RdbcError::ConnectionDiscarded);
                }
                _ = statement_stop.changed() => {
                    interrupted.store(true, Ordering::Release);
                    return Err(RdbcError::ConnectionDiscarded);
                }
                request = requests.recv() => request,
            };
            let Some(request) = request else {
                break;
            };
            let result = rows.try_next().await;
            let terminal = !matches!(result, Ok(Some(_)));
            if request.reply.send(result).is_err() || terminal {
                break;
            }
        }
        Ok(())
    }
}
