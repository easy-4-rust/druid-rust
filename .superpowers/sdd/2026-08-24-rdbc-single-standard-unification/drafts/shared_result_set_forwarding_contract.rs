//! 共享具体句柄的实际后端转发补充合同草案，尚未接入 Cargo。
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{BufferedRowSet, SharedResultSet};
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::types::{Row, Value};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// 保存可观察关闭尝试和实际后端关闭状态，不用代理缓存制造成功。
#[derive(Debug)]
struct FallibleCloseResultSet {
    attempts: Arc<AtomicUsize>,
    closed: Arc<AtomicBool>,
}

druid_rdbc::impl_standard_wrapper!(FallibleCloseResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for FallibleCloseResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(false)
        }
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        let attempt = self.attempts.fetch_add(1, Ordering::AcqRel);
        if attempt == 0 {
            return Err(RdbcError::DriverError("backend close failed".to_owned()));
        }
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed.load(Ordering::Acquire))
    }
}

#[tokio::test]
async fn shared_result_set_preserves_actual_close_error_and_retry() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let closed = Arc::new(AtomicBool::new(false));
    let first = SharedResultSet::new(Box::new(FallibleCloseResultSet {
        attempts: Arc::clone(&attempts),
        closed: Arc::clone(&closed),
    }));
    let same = first.clone();
    assert_eq!(
        first.close().await,
        Err(RdbcError::DriverError("backend close failed".to_owned()))
    );
    assert_eq!(attempts.load(Ordering::Acquire), 1);
    assert!(!first.is_closed().unwrap());
    assert!(!same.is_closed().unwrap());
    same.close().await.unwrap();
    assert_eq!(attempts.load(Ordering::Acquire), 2);
    assert!(closed.load(Ordering::Acquire));
    assert!(first.is_closed().unwrap());
    // 旧具体句柄每次转发；原始后端自己决定重复关闭是否幂等。
    first.close().await.unwrap();
    assert_eq!(attempts.load(Ordering::Acquire), 3);
}

#[tokio::test]
async fn shared_result_set_borrow_preserves_cursor_and_reports_contention() {
    let first = SharedResultSet::new(Box::new(BufferedRowSet::new(vec![
        Row::new(vec![Value::Int(7)]),
        Row::new(vec![Value::Int(8)]),
    ])));
    let same = first.clone();
    let mut backend = first.try_result_set().unwrap();
    assert!(backend.next().await.unwrap());
    assert_eq!(
        backend.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(7)
    );
    assert!(same.try_result_set().is_err());
    assert!(same.is_closed().is_err());
    assert!(format!("{same:?}").contains("SharedResultSet"));
    drop(backend);
    let mut backend = same.try_result_set().unwrap();
    assert!(backend.next().await.unwrap());
    assert_eq!(
        backend.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(8)
    );
    assert!(!backend.next().await.unwrap());
    drop(backend);
    assert!(!first.is_closed().unwrap());
    first.close().await.unwrap();
    assert!(same.is_closed().unwrap());
}
