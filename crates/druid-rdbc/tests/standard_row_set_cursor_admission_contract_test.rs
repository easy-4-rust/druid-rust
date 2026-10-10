//! 外部 clone 在 provider 已实际 Pending 时不得改变已发布游标。
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{RowSet, RowSetInternal, RowSetReader, RowSetWriter, SharedRowSet};
use druid_rdbc::result_set::ResultSet;
use std::future::{pending, poll_fn, Future};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::task::Poll;
struct PendingReader(Arc<AtomicBool>);
#[async_trait::async_trait]
impl RowSetReader for PendingReader {
    async fn read_data(&mut self, _caller: &mut dyn RowSetInternal) -> Result<(), RdbcError> {
        self.0.store(true, Ordering::SeqCst);
        pending().await
    }
}
struct PendingWriter(Arc<AtomicBool>);
#[async_trait::async_trait]
impl RowSetWriter for PendingWriter {
    async fn write_data(&mut self, _caller: &mut dyn RowSetInternal) -> Result<bool, RdbcError> {
        self.0.store(true, Ordering::SeqCst);
        pending().await
    }
}
#[test]
fn external_cursor_return_is_rejected_while_reader_is_actually_pending() {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(async {
            let entered = Arc::new(AtomicBool::new(false));
            let mut source = SharedRowSet::new();
            source
                .set_reader(Some(Box::new(PendingReader(Arc::clone(&entered)))))
                .unwrap();
            let mut external = source.clone();
            let mut execution = Box::pin(source.execute());
            poll_fn(|cx| {
                assert!(execution.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
            assert!(entered.load(Ordering::SeqCst));
            let result = external.move_to_current_row();
            drop(execution);
            assert!(
                matches!(result, Err(RdbcError::SqlException(_))),
                "busy Reader must reject external cursor movement: {result:?}"
            );
            external.move_to_current_row().unwrap();
        });
}
#[test]
fn external_cursor_return_is_rejected_while_writer_is_actually_pending() {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(async {
            let entered = Arc::new(AtomicBool::new(false));
            let mut source = SharedRowSet::new();
            source
                .set_writer(Some(Box::new(PendingWriter(Arc::clone(&entered)))))
                .unwrap();
            let mut external = source.clone();
            let mut execution = Box::pin(source.accept_changes());
            poll_fn(|cx| {
                assert!(execution.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
            assert!(entered.load(Ordering::SeqCst));
            let result = external.move_to_current_row();
            drop(execution);
            assert!(
                matches!(result, Err(RdbcError::SqlException(_))),
                "busy Writer must reject external cursor movement: {result:?}"
            );
            external.move_to_current_row().unwrap();
        });
}
