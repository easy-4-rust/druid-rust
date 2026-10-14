use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::types::Value;
use futures::StreamExt;
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// 记录真实标准游标调用的 Send 非 Sync 外部实现，用于检验扩展所有权与关闭顺序。
/// 对应 Java: java.sql.ResultSet 的外部驱动实现合同。
struct StreamContractCursor {
    next_calls: Arc<AtomicUsize>,
    closes: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
    index: Cell<usize>,
    fail_read: bool,
    fail_close: bool,
}

druid_rdbc::impl_standard_wrapper!(StreamContractCursor => ResultSet);

impl Drop for StreamContractCursor {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

#[async_trait::async_trait]
impl ResultSet for StreamContractCursor {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.next_calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_read {
            return Err(RdbcError::InvalidArgument("read-primary".to_owned()));
        }
        self.index.set(self.index.get() + 1);
        Ok(self.index.get() <= 2)
    }
    fn value(&self, _: ColumnIndex) -> Result<Value, RdbcError> {
        Ok(Value::Int(self.index.get() as i64))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closes.fetch_add(1, Ordering::SeqCst);
        if self.fail_close {
            Err(RdbcError::InvalidArgument("close-secondary".to_owned()))
        } else {
            Ok(())
        }
    }
}

fn cursor(
    fail_read: bool,
    fail_close: bool,
) -> (
    Box<dyn ResultSet>,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
) {
    let next = Arc::new(AtomicUsize::new(0));
    let closes = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let cursor = StreamContractCursor {
        next_calls: Arc::clone(&next),
        closes: Arc::clone(&closes),
        drops: Arc::clone(&drops),
        index: Cell::new(0),
        fail_read,
        fail_close,
    };
    (Box::new(cursor), next, closes, drops)
}

#[tokio::test]
async fn standard_trait_object_stream_is_lazy_send_and_drops_owned_cursor() {
    let (cursor, next, closes, drops) = cursor(false, false);
    assert_eq!(cursor.known_row_count(), None);
    assert_eq!(next.load(Ordering::SeqCst), 0, "count lookup must not poll the stream");
    let mut stream = cursor.into_stream(|row| row.int(ColumnIndex::new(1)?));
    fn assert_send<T: Send>(_: &T) {}
    assert_send(&stream);
    assert_eq!(next.load(Ordering::SeqCst), 0);
    assert_eq!(stream.next().await.unwrap().unwrap(), 1);
    assert_eq!(next.load(Ordering::SeqCst), 1);
    drop(stream);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        closes.load(Ordering::SeqCst),
        0,
        "generic Drop cannot await driver close"
    );
}

#[tokio::test]
async fn stream_eof_closes_exactly_once_and_never_repolls_cursor() {
    let (cursor, next, closes, drops) = cursor(false, false);
    let mut stream = cursor.into_stream(|row| row.int(ColumnIndex::new(1)?));
    assert_eq!(stream.next().await.unwrap().unwrap(), 1);
    assert_eq!(stream.next().await.unwrap().unwrap(), 2);
    assert!(stream.next().await.is_none());
    assert!(stream.next().await.is_none());
    assert_eq!(next.load(Ordering::SeqCst), 3);
    assert_eq!(closes.load(Ordering::SeqCst), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn stream_read_error_is_preserved_over_cleanup_error() {
    let (cursor, next, closes, _) = cursor(true, true);
    let mut stream = cursor.into_stream(|row| row.int(ColumnIndex::new(1)?));
    let error = stream.next().await.unwrap().unwrap_err();
    assert!(error.to_string().contains("read-primary"));
    assert_eq!(next.load(Ordering::SeqCst), 1);
    assert_eq!(closes.load(Ordering::SeqCst), 1);
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn stream_map_error_is_preserved_over_cleanup_error() {
    let (cursor, next, closes, _) = cursor(false, true);
    let mut stream =
        cursor.into_stream(|_| Err::<i32, _>(RdbcError::InvalidArgument("map-primary".to_owned())));
    assert!(stream
        .next()
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("map-primary"));
    assert_eq!(next.load(Ordering::SeqCst), 1);
    assert_eq!(closes.load(Ordering::SeqCst), 1);
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn stream_eof_reports_cleanup_error_when_no_primary_error_exists() {
    let (cursor, _, closes, _) = cursor(false, true);
    let mut stream = cursor.into_stream(|row| row.int(ColumnIndex::new(1)?));
    stream.next().await.unwrap().unwrap();
    stream.next().await.unwrap().unwrap();
    assert!(stream
        .next()
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("close-secondary"));
    assert_eq!(closes.load(Ordering::SeqCst), 1);
    assert!(stream.next().await.is_none());
}
