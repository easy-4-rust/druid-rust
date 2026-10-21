//! 完整原 RowSet getter、流及独立共享转发身份合同。
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{BufferedRowSet, SharedResultSet};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::types::{
    RdbcBigDecimal as BigDecimal, RdbcNaiveDate as NaiveDate, RdbcNaiveDateTime as NaiveDateTime,
    RdbcNaiveTime as NaiveTime, Row, Value,
};
use std::future::Future;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

fn require_send<T: Send>(value: T) -> T {
    value
}

#[tokio::test]
async fn shared_close_waits_for_borrow_and_cancelled_waiter_preserves_backend() {
    let first = SharedResultSet::new(Box::new(BufferedRowSet::new(vec![Row::new(vec![
        Value::Int(7),
    ])])));
    let same = first.clone();
    let mut backend = require_send(first.try_result_set().unwrap());
    let identity = std::ptr::from_ref(&*backend);
    let mut cancelled = Box::pin(require_send(same.close()));
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(
        cancelled.as_mut().poll(&mut context),
        Poll::Pending
    ));
    drop(cancelled);
    assert!(backend.next().await.unwrap());
    assert_eq!(
        backend.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(7)
    );
    drop(backend);
    let backend = first.try_result_set().unwrap();
    assert!(std::ptr::eq(std::ptr::from_ref(&*backend), identity));
    let mut waiting = Box::pin(require_send(same.close()));
    assert!(matches!(waiting.as_mut().poll(&mut context), Poll::Pending));
    drop(backend);
    waiting.await.unwrap();
    assert!(first.is_closed().unwrap());
}

/// 真实关闭过程可被取消的后端，用于验证句柄不会丢失拥有的原始对象。
#[derive(Debug)]
struct CancellableCloseResultSet {
    started: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    attempts: Arc<AtomicUsize>,
    closed: Arc<AtomicBool>,
}

druid_rdbc::impl_standard_wrapper!(CancellableCloseResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for CancellableCloseResultSet {
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
            self.started.notify_one();
            self.release.notified().await;
        }
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed.load(Ordering::Acquire))
    }
}

#[tokio::test]
async fn cancelled_active_shared_close_restores_same_backend_and_allows_retry() {
    let started = Arc::new(tokio::sync::Notify::new());
    let attempts = Arc::new(AtomicUsize::new(0));
    let closed = Arc::new(AtomicBool::new(false));
    let first = SharedResultSet::new(Box::new(CancellableCloseResultSet {
        started: Arc::clone(&started),
        release: Arc::new(tokio::sync::Notify::new()),
        attempts: Arc::clone(&attempts),
        closed: Arc::clone(&closed),
    }));
    let backend = first.try_result_set().unwrap();
    let identity = std::ptr::from_ref(&*backend);
    drop(backend);
    let same = first.clone();
    let task = tokio::spawn(async move { same.close().await });
    started.notified().await;
    assert!(first.try_result_set().is_err());
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(attempts.load(Ordering::Acquire), 1);
    assert!(!closed.load(Ordering::Acquire));
    assert!(!first.is_closed().unwrap());
    let backend = first.try_result_set().unwrap();
    assert!(std::ptr::eq(std::ptr::from_ref(&*backend), identity));
    drop(backend);
    first.close().await.unwrap();
    assert_eq!(attempts.load(Ordering::Acquire), 2);
    assert!(first.is_closed().unwrap());
}

fn task6_scalar_default_row_set() -> BufferedRowSet {
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_opt(13, 14, 15).unwrap();
    BufferedRowSet::with_column_labels(
        vec![Row::new(vec![
            Value::Null,
            Value::Bool(true),
            Value::Int(42),
            Value::Float(2.5),
            Value::Decimal(BigDecimal::from_str("3.25").unwrap()),
            Value::String("17".to_string()),
            Value::Bytes(b"bytes".to_vec()),
            Value::Date(date),
            Value::Time(time),
            Value::Timestamp(NaiveDateTime::new(date, time)),
            Value::Bytes(vec![0xff]),
            Value::String("not-a-number".to_string()),
            Value::Int(i64::MAX),
            Value::Bytes(vec![1, 2]),
        ])],
        (1..=14).map(|index| format!("c{index}")).collect(),
    )
}

fn task6_assert_default_scalar_successes(result_set: &BufferedRowSet) {
    assert_eq!(
        result_set.string(ColumnIndex::new(1).unwrap()).unwrap(),
        None
    );
    for index in 2..=10 {
        assert!(result_set
            .string(ColumnIndex::new(index).unwrap())
            .unwrap()
            .is_some());
    }
    assert_eq!(
        result_set.string_by_label("c3").unwrap().as_deref(),
        Some("42")
    );
    assert!(!result_set.boolean(ColumnIndex::new(1).unwrap()).unwrap());
    assert!(result_set.boolean(ColumnIndex::new(2).unwrap()).unwrap());
    assert!(result_set.boolean(ColumnIndex::new(3).unwrap()).unwrap());
    assert!(result_set.boolean(ColumnIndex::new(4).unwrap()).unwrap());
    assert!(result_set.boolean(ColumnIndex::new(5).unwrap()).unwrap());
    assert!(!result_set.boolean_by_label("c6").unwrap());
    assert_eq!(result_set.long(ColumnIndex::new(1).unwrap()).unwrap(), 0);
    assert_eq!(result_set.long(ColumnIndex::new(2).unwrap()).unwrap(), 1);
    assert_eq!(result_set.long(ColumnIndex::new(3).unwrap()).unwrap(), 42);
    assert_eq!(result_set.long(ColumnIndex::new(4).unwrap()).unwrap(), 2);
    assert_eq!(result_set.long(ColumnIndex::new(5).unwrap()).unwrap(), 3);
    assert_eq!(result_set.long_by_label("c6").unwrap(), 17);
    assert_eq!(result_set.int_by_label("c3").unwrap(), 42);
    assert_eq!(result_set.short_by_label("c3").unwrap(), 42);
    assert_eq!(result_set.byte_by_label("c3").unwrap(), 42);
    assert_eq!(
        result_set.double(ColumnIndex::new(1).unwrap()).unwrap(),
        0.0
    );
    assert_eq!(
        result_set.double(ColumnIndex::new(2).unwrap()).unwrap(),
        1.0
    );
    assert_eq!(
        result_set.double(ColumnIndex::new(3).unwrap()).unwrap(),
        42.0
    );
    assert_eq!(
        result_set.double(ColumnIndex::new(4).unwrap()).unwrap(),
        2.5
    );
    assert_eq!(
        result_set.double(ColumnIndex::new(5).unwrap()).unwrap(),
        3.25
    );
    assert_eq!(result_set.double_by_label("c6").unwrap(), 17.0);
    assert_eq!(result_set.float_by_label("c4").unwrap(), 2.5);
    assert_eq!(
        result_set.bytes(ColumnIndex::new(1).unwrap()).unwrap(),
        None
    );
    assert_eq!(
        result_set.bytes(ColumnIndex::new(7).unwrap()).unwrap(),
        Some(b"bytes".to_vec())
    );
    assert_eq!(
        result_set.bytes_by_label("c3").unwrap(),
        Some(b"42".to_vec())
    );
    assert_eq!(result_set.value_by_label("c3").unwrap(), Value::Int(42));
}

fn task6_assert_default_scalar_failures(result_set: &BufferedRowSet) {
    assert!(matches!(
        result_set.string(ColumnIndex::new(11).unwrap()),
        Err(RdbcError::SqlException(error)) if error.class_name() == "java.sql.SQLDataException" && error.sql_state() == Some("22021")
    ));
    assert!(matches!(
        result_set.boolean(ColumnIndex::new(7).unwrap()),
        Err(RdbcError::SqlException(error)) if error.class_name() == "java.sql.SQLDataException" && error.sql_state() == Some("22005")
    ));
    assert!(matches!(
        result_set.long(ColumnIndex::new(12).unwrap()),
        Err(RdbcError::SqlException(error))
            if error.class_name() == "java.sql.SQLDataException"
                && error.sql_state() == Some("22018")
    ));
    assert!(matches!(
        result_set.long(ColumnIndex::new(14).unwrap()),
        Err(RdbcError::SqlException(error)) if error.class_name() == "java.sql.SQLDataException" && error.sql_state() == Some("22005")
    ));
    assert!(matches!(
        result_set.double(ColumnIndex::new(12).unwrap()),
        Err(RdbcError::SqlException(error)) if error.class_name() == "java.sql.SQLDataException" && error.sql_state() == Some("22018")
    ));
    assert!(matches!(
        result_set.double(ColumnIndex::new(14).unwrap()),
        Err(RdbcError::SqlException(error)) if error.class_name() == "java.sql.SQLDataException" && error.sql_state() == Some("22005")
    ));
    assert!(matches!(
        result_set.int(ColumnIndex::new(13).unwrap()),
        Err(RdbcError::SqlException(error))
            if error.class_name() == "java.sql.SQLDataException"
                && error.sql_state() == Some("22003")
    ));
    assert!(matches!(
        result_set.short(ColumnIndex::new(13).unwrap()),
        Err(RdbcError::SqlException(error))
            if error.class_name() == "java.sql.SQLDataException"
                && error.sql_state() == Some("22003")
    ));
    assert!(matches!(
        result_set.byte(ColumnIndex::new(13).unwrap()),
        Err(RdbcError::SqlException(error))
            if error.class_name() == "java.sql.SQLDataException"
                && error.sql_state() == Some("22003")
    ));
    assert!(matches!(
        result_set.value(ColumnIndex::new(15).unwrap()),
        Err(RdbcError::InvalidArgument(message)) if message.contains("exceeds row width")
    ));
}

fn task6_assert_default_stream_conversions(result_set: &BufferedRowSet) {
    assert!(result_set
        .binary_stream(ColumnIndex::new(1).unwrap())
        .unwrap()
        .is_none());
    assert!(result_set
        .character_stream(ColumnIndex::new(1).unwrap())
        .unwrap()
        .is_none());
    for index in 2..=10 {
        assert!(result_set
            .binary_stream(ColumnIndex::new(index).unwrap())
            .unwrap()
            .is_some());
        assert!(result_set
            .character_stream(ColumnIndex::new(index).unwrap())
            .unwrap()
            .is_some());
    }
    assert!(matches!(
        result_set.character_stream(ColumnIndex::new(11).unwrap()),
        Err(RdbcError::SqlException(error)) if error.class_name() == "java.sql.SQLDataException" && error.sql_state() == Some("22021")
    ));
}

/// 与原稀疏后端对应的独立实例；明确拒绝推进，不冒充成功行。
#[derive(Debug)]
struct SparseSharedResultSetBackend;
druid_rdbc::impl_standard_wrapper!(SparseSharedResultSetBackend => ResultSet);
#[async_trait::async_trait]
impl ResultSet for SparseSharedResultSetBackend {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_next",
        })
    }
}

/// 保留全部原 getter/stream helpers、导航及后端指针身份观察。
/// 对应 Java: java.sql.ResultSet；原共享句柄的 concrete forwarding handoff。
#[tokio::test]
async fn task6_row_set_default_getters_cover_null_label_conversion_and_stream_contracts() {
    let mut result_set = task6_scalar_default_row_set();
    assert!(result_set.next().await.unwrap());
    task6_assert_default_scalar_successes(&result_set);
    task6_assert_default_scalar_failures(&result_set);
    task6_assert_default_stream_conversions(&result_set);
    assert!(!result_set.absolute(0).await.unwrap());
    assert!(result_set.absolute(1).await.unwrap());
    let raw: Box<dyn ResultSet> = Box::new(result_set);
    let raw_identity = std::ptr::from_ref(raw.as_ref());
    let first = SharedResultSet::new(raw);
    let same = first.clone();
    let other = SharedResultSet::new(Box::new(SparseSharedResultSetBackend));
    assert_eq!(first, same);
    assert_ne!(first, other);
    assert!(std::ptr::eq(
        std::ptr::from_ref(&*first.try_result_set().unwrap()),
        raw_identity
    ));
    assert!(format!("{first:?}").contains("SharedResultSet"));
    first.close().await.unwrap();
    assert!(first.is_closed().unwrap());
    assert!(same.is_closed().unwrap());
}

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

/// 记录执行器的实际唤醒，避免通过 sleep 猜测关闭等待状态。
struct SharedCloseWake(std::sync::atomic::AtomicUsize);

impl std::task::Wake for SharedCloseWake {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
}

#[test]
fn shared_close_cancelled_queue_head_wakes_next_without_runtime() {
    let first = SharedResultSet::new(Box::new(BufferedRowSet::new(Vec::new())));
    let backend = first.try_result_set().unwrap();
    let head_wake = Arc::new(SharedCloseWake(AtomicUsize::new(0)));
    let next_wake = Arc::new(SharedCloseWake(AtomicUsize::new(0)));
    let head_waker = Waker::from(Arc::clone(&head_wake));
    let next_waker = Waker::from(Arc::clone(&next_wake));
    let mut head_context = Context::from_waker(&head_waker);
    let mut next_context = Context::from_waker(&next_waker);
    let mut head = Box::pin(require_send(first.close()));
    let mut next = Box::pin(require_send(first.close()));
    assert!(matches!(
        head.as_mut().poll(&mut head_context),
        Poll::Pending
    ));
    assert!(matches!(
        next.as_mut().poll(&mut next_context),
        Poll::Pending
    ));
    drop(backend);
    assert!(head_wake.0.load(Ordering::Acquire) > 0);
    assert!(next_wake.0.load(Ordering::Acquire) > 0);
    assert!(matches!(
        next.as_mut().poll(&mut next_context),
        Poll::Pending
    ));
    let wakes_before_cancel = next_wake.0.load(Ordering::Acquire);
    drop(head);
    assert!(next_wake.0.load(Ordering::Acquire) > wakes_before_cancel);
    assert!(first.try_result_set().is_err());
    let cancelled_head_wakes = head_wake.0.load(Ordering::Acquire);
    assert!(matches!(
        next.as_mut().poll(&mut next_context),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(head_wake.0.load(Ordering::Acquire), cancelled_head_wakes);
    assert!(first.is_closed().unwrap());
}

/// 通过真实公开 ResultSet/RowSet 入口检查下标与列名两条类型化转换路径。
#[tokio::test]
async fn buffered_typed_integer_reads_preserve_data_errors_and_null_state() {
    use druid_rdbc::types::{RdbcObject, RdbcTargetType};
    let mut result = BufferedRowSet::with_column_labels(
        vec![Row::new(vec![
            Value::Float(f64::NAN),
            Value::Float(9223372036854775808.0),
            Value::Int(128),
            Value::String("private-invalid-number".to_owned()),
            Value::Null,
            Value::Float(-17.9),
        ])],
        ["nan", "overflow", "byte", "invalid", "null", "fraction"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
    );
    assert!(result.next().await.unwrap());
    for (index, label, target, state) in [
        (1, "nan", RdbcTargetType::Long, "22003"),
        (2, "overflow", RdbcTargetType::Long, "22003"),
        (3, "byte", RdbcTargetType::Byte, "22003"),
        (4, "invalid", RdbcTargetType::Long, "22018"),
    ] {
        for read in [
            result.object_as(ColumnIndex::new(index).unwrap(), &target),
            result.object_as_by_label(label, &target),
        ] {
            let RdbcError::SqlException(error) = read.unwrap_err() else {
                panic!("expected SQLDataException")
            };
            assert_eq!(error.class_name(), "java.sql.SQLDataException");
            assert_eq!(error.sql_state(), Some(state));
            assert!(!format!("{error:?}").contains("private-invalid-number"));
        }
    }
    assert_eq!(
        result
            .object_as_by_label("null", &RdbcTargetType::Long)
            .unwrap(),
        RdbcObject::Scalar(Value::Null)
    );
    assert!(result.was_null().unwrap());
    assert_eq!(
        result
            .object_as_by_label("fraction", &RdbcTargetType::Long)
            .unwrap(),
        RdbcObject::Long(-17)
    );
    assert!(!result.was_null().unwrap());
    result.close().await.unwrap();
}
