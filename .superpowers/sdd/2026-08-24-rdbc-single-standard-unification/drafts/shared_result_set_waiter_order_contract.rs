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
    assert!(matches!(head.as_mut().poll(&mut head_context), Poll::Pending));
    assert!(matches!(next.as_mut().poll(&mut next_context), Poll::Pending));
    drop(backend);
    assert!(head_wake.0.load(Ordering::Acquire) > 0);
    assert!(next_wake.0.load(Ordering::Acquire) > 0);
    assert!(matches!(next.as_mut().poll(&mut next_context), Poll::Pending));
    let wakes_before_cancel = next_wake.0.load(Ordering::Acquire);
    drop(head);
    assert!(next_wake.0.load(Ordering::Acquire) > wakes_before_cancel);
    assert!(first.try_result_set().is_err());
    let cancelled_head_wakes = head_wake.0.load(Ordering::Acquire);
    assert!(matches!(next.as_mut().poll(&mut next_context), Poll::Ready(Ok(()))));
    assert_eq!(head_wake.0.load(Ordering::Acquire), cancelled_head_wakes);
    assert!(first.is_closed().unwrap());
}
