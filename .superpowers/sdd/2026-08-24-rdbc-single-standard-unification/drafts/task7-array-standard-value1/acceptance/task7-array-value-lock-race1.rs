#[tokio::test]
async fn task7_array_value_waiting_read_rechecks_release_after_lock_handoff() {
    let probe=value_array_probe(ResourceOperations::array(),false);
    probe.block_cursor.store(true,Ordering::SeqCst);
    let values_calls=Arc::new(AtomicUsize::new(0));
    let value=druid_rdbc::resource::ResourceHandleFactory::array(Box::new(BoundaryArray {inner:ArrayDriver {probe:probe.clone(),freed:false,not_sync:std::cell::Cell::new(0)},values_calls:values_calls.clone(),panic_once:false}));let child=value.clone();
    let cursor_task=tokio::spawn(async move{child.result_set().await});
    probe.cursor_started.notified().await;
    let mut release=Box::pin(value.free());
    std::future::poll_fn(|cx|{assert!(std::future::Future::poll(release.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    let mut read=Box::pin(value.values());
    std::future::poll_fn(|cx|{assert!(std::future::Future::poll(read.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    let calls=probe.calls.lock().unwrap().len();
    probe.cursor_finish.notify_one();let mut cursor=cursor_task.await.unwrap().unwrap();
    release.await.unwrap();assert!(read.await.is_err());
    assert_eq!(probe.calls.lock().unwrap().len(),calls);assert_eq!(probe.frees.load(Ordering::SeqCst),1);assert_eq!(values_calls.load(Ordering::SeqCst),0);
    cursor.close().await.unwrap();
}
