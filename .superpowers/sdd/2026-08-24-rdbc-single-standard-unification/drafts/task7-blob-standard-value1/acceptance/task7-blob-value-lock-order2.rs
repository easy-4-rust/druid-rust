#[tokio::test]
async fn task7_blob_value_blocked_first_locks_preserve_global_order(){
    let ap=blob_value_probe(ResourceOperations::blob());let bp=blob_value_probe(ResourceOperations::blob());
    ap.block_read.store(true,Ordering::SeqCst);bp.block_read.store(true,Ordering::SeqCst);
    let a=blob_value(ap.clone());let b=blob_value(bp.clone());let aa=a.clone();let bb=b.clone();
    let hold_a=tokio::spawn(async move{aa.length().await});ap.read_started.notified().await;
    let hold_b=tokio::spawn(async move{bb.length().await});bp.read_started.notified().await;
    // 两个接收者都被真实读取占用，先让搜索在首把锁排队，避免单线程调度掩盖反向取锁。
    let mut first=Box::pin(a.position_blob(&b,1));let mut second=Box::pin(b.position_blob(&a,1));
    std::future::poll_fn(|cx|{assert!(std::future::Future::poll(first.as_mut(),cx).is_pending());assert!(std::future::Future::poll(second.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    ap.block_read.store(false,Ordering::SeqCst);bp.block_read.store(false,Ordering::SeqCst);
    ap.read_finish.notify_one();bp.read_finish.notify_one();hold_a.await.unwrap().unwrap();hold_b.await.unwrap().unwrap();
    let (left,right)=tokio::time::timeout(Duration::from_secs(2),async{tokio::join!(first,second)}).await.expect("both searches must complete after queued reads release their locks");
    assert_eq!(left.unwrap(),Some(1));assert_eq!(right.unwrap(),Some(1));
    for probe in [ap,bp]{let calls=probe.calls.lock().unwrap();assert_eq!(calls.iter().filter(|&&name|name=="position_blob").count(),1);assert_eq!(calls.iter().filter(|&&name|name=="length").count(),2);}
}
