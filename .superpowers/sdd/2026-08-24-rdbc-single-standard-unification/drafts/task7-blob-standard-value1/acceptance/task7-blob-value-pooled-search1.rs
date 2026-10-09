#[tokio::test]
async fn task7_blob_value_pooled_self_and_other_search_preserve_owner_lifecycle(){
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
    let first=ResourceHandleFactory::blob(connection.create_blob().await.unwrap());let second=ResourceHandleFactory::blob(connection.create_blob().await.unwrap());
    assert_eq!(first.position_blob(&first.clone(),1).await.unwrap(),Some(1));assert_eq!(first.position_blob(&first,2).await.unwrap(),None);assert_eq!(first.position_blob(&second,1).await.unwrap(),Some(1));
    connection.close().await.unwrap();assert!(first.length().await.is_err());assert!(second.length().await.is_err());
    first.free().await.unwrap();second.free().await.unwrap();assert!(first.is_freed());assert!(second.is_freed());
    for probe in control.probes.lock().unwrap().iter(){assert_eq!(probe.calls.load(Ordering::SeqCst),1);}
    assert_eq!(control.returns.lock().unwrap().len(),1);
}
#[tokio::test]
async fn task7_blob_value_cancelled_pattern_read_releases_both_locks(){
    let ap=blob_value_probe(ResourceOperations::blob());let bp=blob_value_probe(ResourceOperations::blob());bp.block_read.store(true,Ordering::SeqCst);
    let a=blob_value(ap);let b=blob_value(bp.clone());let aa=a.clone();let bb=b.clone();let search=tokio::spawn(async move{aa.position_blob(&bb,1).await});bp.read_started.notified().await;
    search.abort();assert!(search.await.unwrap_err().is_cancelled());bp.block_read.store(false,Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(2),async{assert_eq!(a.length().await.unwrap(),4);assert_eq!(b.length().await.unwrap(),4);a.free().await.unwrap();b.free().await.unwrap();}).await.expect("cancelled search must release both resource locks");
}
