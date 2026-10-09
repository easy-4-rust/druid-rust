// 新合同追加到原58项消费者之后，不替换既有测试体。
fn value_array_probe(supported:ResourceOperations, block:bool)->Arc<ArrayProbe> {
    Arc::new(ArrayProbe {supported,free_failures:AtomicUsize::new(0),free_fatal:AtomicBool::new(false),nonfatal_read:AtomicBool::new(false),args:(Some("vendor.Integer".into()),Some(array_values())),maps:Mutex::new(Vec::new()),calls:Mutex::new(Vec::new()),frees:AtomicUsize::new(0),started:tokio::sync::Notify::new(),finish:tokio::sync::Notify::new(),block,read_failure:AtomicBool::new(false),bad_metadata:AtomicBool::new(false),block_cursor:AtomicBool::new(false),cursor_started:tokio::sync::Notify::new(),cursor_finish:tokio::sync::Notify::new(),cursor_next_failure:AtomicBool::new(false),cursors:Mutex::new(Vec::new())})
}
fn value_array(probe:Arc<ArrayProbe>)->druid_rdbc::resource::RdbcArray {
    druid_rdbc::resource::ResourceHandleFactory::array(Box::new(ArrayDriver {probe,freed:false,not_sync:std::cell::Cell::new(0)}))
}
async fn value_array_operation(value:&druid_rdbc::resource::RdbcArray,op:usize)->Result<(),RdbcError> {
    let map=array_map();
    match op {
        0=>value.base_type_name().await.map(|_|()),1=>value.base_type().await.map(|_|()),
        2=>value.values().await.map(|_|()),3=>value.values_with_type_map(&map).await.map(|_|()),
        4=>value.values_range(1,2).await.map(|_|()),5=>value.values_range_with_type_map(1,2,&map).await.map(|_|()),
        6=>{let mut cursor=value.result_set().await?;cursor.close().await},
        7=>{let mut cursor=value.result_set_with_type_map(&map).await?;cursor.close().await},
        8=>{let mut cursor=value.result_set_range(1,2).await?;cursor.close().await},
        9=>{let mut cursor=value.result_set_range_with_type_map(1,2,&map).await?;cursor.close().await},
        _=>panic!("unknown operation"),
    }
}
#[tokio::test]
async fn task7_array_value_all_capabilities_gate_before_driver_and_after_free() {
    let flags=[ResourceOperations::READ,ResourceOperations::RANGE,ResourceOperations::TYPE_MAP,ResourceOperations::RESULT_SET,ResourceOperations::FREE];
    let required=[flags[0],flags[0],flags[0],flags[0]|flags[2],flags[0]|flags[1],flags[0]|flags[1]|flags[2],flags[0]|flags[3],flags[0]|flags[3]|flags[2],flags[0]|flags[1]|flags[3],flags[0]|flags[1]|flags[3]|flags[2]];
    for mask in 0..32 {
        let mut supported=ResourceOperations::empty();for (bit,flag) in flags.iter().enumerate(){if mask&(1<<bit)!=0 {supported|=*flag;}}
        let probe=value_array_probe(supported|ResourceOperations::WRITE|ResourceOperations::STREAM,false);
        let value=value_array(probe.clone());assert_eq!(value.capabilities(),supported);
        for (op,required) in required.iter().enumerate(){
            let before=probe.calls.lock().unwrap().len();let result=value_array_operation(&value,op).await;
            if supported.contains(*required) {assert!(result.is_ok(),"mask {mask} op {op}");assert_eq!(probe.calls.lock().unwrap().len(),before+1);}
            else {let error=result.unwrap_err();assert_eq!(error.class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(probe.calls.lock().unwrap().len(),before);}
        }
        if supported.contains(ResourceOperations::FREE) {
            value.free().await.unwrap();value.clone().free().await.unwrap();assert!(value.is_freed());assert_eq!(probe.frees.load(Ordering::SeqCst),1);
            for op in 0..10 {let before=probe.calls.lock().unwrap().len();assert!(value_array_operation(&value,op).await.is_err());assert_eq!(probe.calls.lock().unwrap().len(),before);}
        } else {let error=value.free().await.unwrap_err();assert_eq!(error.class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(probe.frees.load(Ordering::SeqCst),0);assert!(!value.is_freed());}
    }
}
#[tokio::test]
async fn task7_array_value_identity_maps_ranges_and_raw_cursor_ownership() {
    let probe=value_array_probe(ResourceOperations::array(),false);let value=value_array(probe.clone());let cloned=value.clone();
    assert_eq!(value,cloned);assert_eq!(value.resource_id(),cloned.resource_id());assert_ne!(value,value_array(value_array_probe(ResourceOperations::array(),false)));
    assert_eq!(value.get_base_type_name().await.unwrap(),"vendor.Integer");assert_eq!(value.get_base_type().await.unwrap(),4);assert_eq!(value.get_array().await.unwrap(),array_values());
    assert_eq!(value.values_range(2,2).await.unwrap(),array_values()[1..]);assert!(value.values_range(0,1).await.is_err());assert!(value.values_range(1,-1).await.is_err());
    let map=array_map();assert_eq!(value.values_with_type_map(&map).await.unwrap(),array_values());assert_eq!(value.values_range_with_type_map(2,1,&map).await.unwrap(),vec![Value::Null.into()]);
    let mut cursor=value.result_set_range_with_type_map(2,1,&map).await.unwrap();assert!(cursor.next().await.unwrap());assert_eq!(cursor.value(ColumnIndex::new(1).unwrap()).unwrap(),Value::Int(2));assert_eq!(cursor.value(ColumnIndex::new(2).unwrap()).unwrap(),Value::Null);
    assert_eq!(*probe.maps.lock().unwrap(),vec![map.clone(),map.clone(),map]);
    value.free().await.unwrap();assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),0);cursor.close().await.unwrap();assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_array_value_original_error_and_failed_free_retry() {
    let probe=value_array_probe(ResourceOperations::array(),false);let value=value_array(probe.clone());
    probe.nonfatal_read.store(true,Ordering::SeqCst);assert_eq!(value.values().await.unwrap_err().to_string(),array_nonfatal_error().to_string());probe.nonfatal_read.store(false,Ordering::SeqCst);
    probe.free_failures.store(1,Ordering::SeqCst);assert_eq!(value.free().await.unwrap_err().to_string(),array_nonfatal_error().to_string());assert!(!value.is_freed());assert_eq!(value.values().await.unwrap(),array_values());
    value.free().await.unwrap();assert!(value.is_freed());assert_eq!(probe.frees.load(Ordering::SeqCst),2);
}
#[tokio::test]
async fn task7_array_value_cancelled_raw_free_requires_retry_confirmation() {
    let probe=value_array_probe(ResourceOperations::array(),true);let value=value_array(probe.clone());let task_value=value.clone();
    let task=tokio::spawn(async move{task_value.free().await});probe.started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());
    assert!(!value.is_freed());let calls=probe.calls.lock().unwrap().len();assert!(value.values().await.is_err());assert_eq!(probe.calls.lock().unwrap().len(),calls);
    let task_value=value.clone();let retry=tokio::spawn(async move{task_value.free().await});probe.started.notified().await;probe.finish.notify_one();retry.await.unwrap().unwrap();
    assert!(value.is_freed());value.free().await.unwrap();assert_eq!(probe.frees.load(Ordering::SeqCst),2);
}
#[tokio::test]
async fn task7_array_value_concurrent_clone_free_calls_driver_once() {
    let probe=value_array_probe(ResourceOperations::array(),true);let value=value_array(probe.clone());let first=value.clone();let second=value.clone();
    let a=tokio::spawn(async move{first.free().await});probe.started.notified().await;let b=tokio::spawn(async move{second.free().await});assert!(value.values().await.is_err());probe.finish.notify_one();
    a.await.unwrap().unwrap();b.await.unwrap().unwrap();assert_eq!(probe.frees.load(Ordering::SeqCst),1);assert!(value.is_freed());
}
#[tokio::test]
async fn task7_array_value_cancelled_read_unlocks_without_consuming_resource() {
    let probe=value_array_probe(ResourceOperations::array(),false);probe.block_cursor.store(true,Ordering::SeqCst);let value=value_array(probe.clone());let child=value.clone();
    let task=tokio::spawn(async move{child.result_set().await});probe.cursor_started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(value.values().await.unwrap(),array_values());assert!(!value.is_freed());value.free().await.unwrap();assert_eq!(probe.frees.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_array_value_pooled_free_cancel_and_parent_close_share_driver_release() {
    let control=Arc::new(Control::default());control.block_free.store(true,Ordering::SeqCst);let mut connection=pooled(control.clone()).await;
    let raw=connection.create_array_of(Some("vendor.Integer"),Some(array_values())).await.unwrap();let probe=array_probe(&control);let value=druid_rdbc::resource::ResourceHandleFactory::array(raw);
    let child=value.clone();let release=tokio::spawn(async move{child.free().await});probe.started.notified().await;release.abort();assert!(release.await.unwrap_err().is_cancelled());assert!(!value.is_freed());assert!(value.values().await.is_err());
    let close=tokio::spawn(async move{connection.close().await});probe.finish.notify_one();close.await.unwrap().unwrap();value.free().await.unwrap();assert!(value.is_freed());assert_eq!(probe.frees.load(Ordering::SeqCst),1);
}
