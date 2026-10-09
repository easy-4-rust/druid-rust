use druid_rdbc::resource::{BlobPattern, RdbcBlob, ResourceHandleFactory};
#[derive(Debug)]
struct BlobValueProbe {
    supported:ResourceOperations, data:Arc<Mutex<Vec<u8>>>, calls:Mutex<Vec<&'static str>>,
    free_calls:AtomicUsize, failures:AtomicUsize, panic_free:AtomicBool,
    block_free:AtomicBool, free_started:tokio::sync::Notify, free_finish:tokio::sync::Notify,
    block_read:AtomicBool, read_started:tokio::sync::Notify, read_finish:tokio::sync::Notify,
    input:Mutex<Option<RdbcInputStream>>,output:Mutex<Option<RdbcOutputStream>>,same:AtomicUsize,
}
fn blob_value_probe(supported:ResourceOperations)->Arc<BlobValueProbe> {
    Arc::new(BlobValueProbe {supported,data:Arc::new(Mutex::new(vec![1,2,3,4])),calls:Mutex::new(Vec::new()),free_calls:AtomicUsize::new(0),failures:AtomicUsize::new(0),panic_free:AtomicBool::new(false),block_free:AtomicBool::new(false),free_started:tokio::sync::Notify::new(),free_finish:tokio::sync::Notify::new(),block_read:AtomicBool::new(false),read_started:tokio::sync::Notify::new(),read_finish:tokio::sync::Notify::new(),input:Mutex::new(None),output:Mutex::new(None),same:AtomicUsize::new(0)})
}
#[derive(Debug)]
struct BlobValueDriver {probe:Arc<BlobValueProbe>,freed:bool,not_sync:std::cell::Cell<usize>}
impl BlobValueDriver {
    fn touch(&self,name:&'static str)->Result<(),RdbcError>{
        self.probe.calls.lock().unwrap().push(name);self.not_sync.set(self.not_sync.get()+1);
        if self.freed {Err(RdbcError::rdbc_resource_closed("Blob"))}else{Ok(())}
    }
    fn range(&self,position:i64,length:i64)->Result<Vec<u8>,RdbcError>{
        let offset=position.checked_sub(1).and_then(|n|usize::try_from(n).ok()).ok_or_else(||RdbcError::InvalidArgument("position is 1-based".into()))?;
        let count=usize::try_from(length).map_err(|_|RdbcError::InvalidArgument("negative length".into()))?;
        let bytes=self.probe.data.lock().unwrap();let tail=bytes.get(offset..).ok_or_else(||RdbcError::InvalidArgument("position outside Blob".into()))?;Ok(tail[..count.min(tail.len())].to_vec())
    }
    fn search(&self,pattern:&[u8],start:i64)->Result<Option<i64>,RdbcError>{
        let offset=start.checked_sub(1).and_then(|n|usize::try_from(n).ok()).ok_or_else(||RdbcError::InvalidArgument("search is 1-based".into()))?;
        let bytes=self.probe.data.lock().unwrap();let tail=bytes.get(offset..).ok_or_else(||RdbcError::InvalidArgument("search outside Blob".into()))?;
        if pattern.is_empty(){return Ok(Some(start));}Ok(tail.windows(pattern.len()).position(|w|w==pattern).map(|n|(offset+n+1) as i64))
    }
    fn write_bytes(&self,position:i64,bytes:&[u8])->Result<i32,RdbcError>{
        let offset=position.checked_sub(1).and_then(|n|usize::try_from(n).ok()).ok_or_else(||RdbcError::InvalidArgument("write is 1-based".into()))?;
        let mut data=self.probe.data.lock().unwrap();if offset>data.len(){return Err(RdbcError::InvalidArgument("write outside Blob".into()));}
        let new_len=data.len().max(offset+bytes.len());data.resize(new_len,0);data[offset..offset+bytes.len()].copy_from_slice(bytes);Ok(bytes.len() as i32)
    }
}
#[derive(Debug)]
struct BlobValueWriter {data:Arc<Mutex<Vec<u8>>>,position:usize}
impl std::io::Write for BlobValueWriter {
    fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{let mut data=self.data.lock().unwrap();let end=self.position+bytes.len();let new_len=data.len().max(end);data.resize(new_len,0);data[self.position..end].copy_from_slice(bytes);self.position=end;Ok(bytes.len())}
    fn flush(&mut self)->std::io::Result<()>{Ok(())}
}
#[async_trait::async_trait]
impl Blob for BlobValueDriver {
    fn capabilities(&self)->ResourceOperations{self.probe.supported}
    async fn length(&mut self)->Result<i64,RdbcError>{self.touch("length")?;self.probe.read_started.notify_one();if self.probe.block_read.load(Ordering::SeqCst){self.probe.read_finish.notified().await;}Ok(self.probe.data.lock().unwrap().len() as i64)}
    async fn get_bytes(&mut self,position:i64,length:i32)->Result<Vec<u8>,RdbcError>{self.touch("get_bytes")?;self.range(position,i64::from(length))}
    async fn get_binary_stream(&mut self)->Result<RdbcInputStream,RdbcError>{self.touch("get_binary_stream")?;let stream=RdbcInputStream::from_bytes(self.probe.data.lock().unwrap().clone());*self.probe.input.lock().unwrap()=Some(stream.clone());Ok(stream)}
    async fn position_bytes(&mut self,pattern:&[u8],start:i64)->Result<Option<i64>,RdbcError>{self.touch("position_bytes")?;self.search(pattern,start)}
    async fn position_blob(&mut self,pattern:BlobPattern<'_>,start:i64)->Result<Option<i64>,RdbcError>{
        self.touch("position_blob")?;tokio::task::yield_now().await;
        let bytes=match pattern {BlobPattern::SameResource=>{self.probe.same.fetch_add(1,Ordering::SeqCst);self.probe.data.lock().unwrap().clone()},BlobPattern::Other(pattern)=>{let length=i32::try_from(pattern.length().await?).map_err(|_|RdbcError::InvalidArgument("large pattern".into()))?;pattern.get_bytes(1,length).await?}};
        self.search(&bytes,start)
    }
    async fn set_bytes(&mut self,position:i64,bytes:&[u8])->Result<i32,RdbcError>{self.touch("set_bytes")?;self.write_bytes(position,bytes)}
    async fn set_bytes_range(&mut self,position:i64,bytes:&[u8],offset:i32,length:i32)->Result<i32,RdbcError>{
        self.touch("set_bytes_range")?;let offset=usize::try_from(offset).map_err(|_|RdbcError::InvalidArgument("negative offset".into()))?;let length=usize::try_from(length).map_err(|_|RdbcError::InvalidArgument("negative length".into()))?;let end=offset.checked_add(length).ok_or_else(||RdbcError::InvalidArgument("range overflow".into()))?;self.write_bytes(position,bytes.get(offset..end).ok_or_else(||RdbcError::InvalidArgument("input range".into()))?)
    }
    async fn set_binary_stream(&mut self,position:i64)->Result<RdbcOutputStream,RdbcError>{self.touch("set_binary_stream")?;let position=position.checked_sub(1).and_then(|n|usize::try_from(n).ok()).ok_or_else(||RdbcError::InvalidArgument("stream position".into()))?;if position>self.probe.data.lock().unwrap().len(){return Err(RdbcError::InvalidArgument("stream outside Blob".into()));}let stream=RdbcOutputStream::new(BlobValueWriter{data:self.probe.data.clone(),position});*self.probe.output.lock().unwrap()=Some(stream.clone());Ok(stream)}
    async fn truncate(&mut self,length:i64)->Result<(),RdbcError>{self.touch("truncate")?;let length=usize::try_from(length).map_err(|_|RdbcError::InvalidArgument("negative length".into()))?;let mut data=self.probe.data.lock().unwrap();if length>data.len(){return Err(RdbcError::InvalidArgument("truncate expands Blob".into()));}data.truncate(length);Ok(())}
    async fn free(&mut self)->Result<(),RdbcError>{
        self.probe.free_calls.fetch_add(1,Ordering::SeqCst);if self.probe.panic_free.swap(false,Ordering::SeqCst){panic!("controlled Blob free panic");}
        self.probe.free_started.notify_one();if self.probe.block_free.load(Ordering::SeqCst){self.probe.free_finish.notified().await;}
        if self.probe.failures.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|n|n.checked_sub(1)).is_ok(){return Err(fatal_error());}self.freed=true;Ok(())
    }
    async fn get_binary_stream_range(&mut self,position:i64,length:i64)->Result<RdbcInputStream,RdbcError>{self.touch("get_binary_stream_range")?;let offset=position.checked_sub(1).and_then(|n|usize::try_from(n).ok()).ok_or_else(||RdbcError::InvalidArgument("stream position".into()))?;let count=usize::try_from(length).map_err(|_|RdbcError::InvalidArgument("negative length".into()))?;if offset.checked_add(count).is_none_or(|end|end>self.probe.data.lock().unwrap().len()){return Err(RdbcError::InvalidArgument("stream range".into()));}let stream=RdbcInputStream::from_bytes(self.range(position,length)?);*self.probe.input.lock().unwrap()=Some(stream.clone());Ok(stream)}
}
fn blob_value(probe:Arc<BlobValueProbe>)->RdbcBlob {ResourceHandleFactory::blob(Box::new(BlobValueDriver{probe,freed:false,not_sync:std::cell::Cell::new(0)}))}
async fn blob_value_operation(value:&RdbcBlob,pattern:&RdbcBlob,op:usize)->Result<(),RdbcError>{
    match op {0=>value.length().await.map(|_|()),1=>value.get_bytes(1,2).await.map(|_|()),2=>value.get_binary_stream().await.map(|_|()),3=>value.position_bytes(&[9],1).await.map(|_|()),4=>value.position_blob(pattern,1).await.map(|_|()),5=>value.set_bytes(1,&[9]).await.map(|_|()),6=>value.set_bytes_range(1,&[7,8],1,1).await.map(|_|()),7=>value.set_binary_stream(1).await.map(|_|()),8=>value.truncate(3).await,9=>value.get_binary_stream_range(1,2).await.map(|_|()),_=>panic!("unknown Blob operation")}
}
#[tokio::test]
async fn task7_blob_value_all_128_capabilities_and_freed_precedence(){
    let flags=[ResourceOperations::READ,ResourceOperations::WRITE,ResourceOperations::SEARCH,ResourceOperations::TRUNCATE,ResourceOperations::STREAM,ResourceOperations::RANGE,ResourceOperations::FREE];
    let required=[flags[0],flags[0]|flags[5],flags[0]|flags[4],flags[0]|flags[2],flags[0]|flags[2],flags[1],flags[1]|flags[5],flags[1]|flags[4],flags[1]|flags[3],flags[0]|flags[4]|flags[5]];
    for mask in 0..128 {let mut supported=ResourceOperations::empty();for(bit,flag)in flags.iter().enumerate(){if mask&(1<<bit)!=0{supported|=*flag;}}
        let probe=blob_value_probe(supported|ResourceOperations::TYPE_MAP|ResourceOperations::RESULT_SET);let value=blob_value(probe.clone());let pattern=blob_value(blob_value_probe(ResourceOperations::blob()));assert_eq!(value.capabilities(),supported);
        for(op,required)in required.iter().enumerate(){let before=probe.calls.lock().unwrap().len();let result=blob_value_operation(&value,&pattern,op).await;if supported.contains(*required){assert!(result.is_ok(),"mask {mask} operation {op}");assert_eq!(probe.calls.lock().unwrap().len(),before+1);}else{assert_eq!(result.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(probe.calls.lock().unwrap().len(),before);}}
        if supported.contains(ResourceOperations::FREE){value.free().await.unwrap();value.clone().free().await.unwrap();assert!(value.is_freed());assert_eq!(probe.free_calls.load(Ordering::SeqCst),1);for op in 0..10{let before=probe.calls.lock().unwrap().len();assert!(blob_value_operation(&value,&pattern,op).await.is_err());assert_eq!(probe.calls.lock().unwrap().len(),before);}}
        else{assert_eq!(value.free().await.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(probe.free_calls.load(Ordering::SeqCst),0);assert!(!value.is_freed());}
    }
}
#[tokio::test]
async fn task7_blob_value_identity_offsets_search_and_exact_streams(){
    let probe=blob_value_probe(ResourceOperations::blob());let value=blob_value(probe.clone());assert_eq!(value,value.clone());assert_eq!(value.resource_id(),value.clone().resource_id());assert_ne!(value,blob_value(blob_value_probe(ResourceOperations::blob())));
    assert_eq!(value.get_bytes(2,2).await.unwrap(),vec![2,3]);assert!(value.get_bytes(0,1).await.is_err());assert!(value.get_bytes(1,-1).await.is_err());assert_eq!(value.set_bytes_range(2,&[7,8,9],1,2).await.unwrap(),2);assert_eq!(value.get_bytes(1,4).await.unwrap(),vec![1,8,9,4]);assert!(value.set_bytes_range(1,&[1],-1,1).await.is_err());assert_eq!(value.position_bytes(&[8,9],1).await.unwrap(),Some(2));
    let input=value.get_binary_stream_range(2,2).await.unwrap();assert_eq!(Some(input.clone()),*probe.input.lock().unwrap());assert_eq!(input.read_to_end().unwrap(),vec![8,9]);assert!(value.get_binary_stream_range(4,2).await.is_err());
    let output=value.set_binary_stream(3).await.unwrap();assert_eq!(Some(output.clone()),*probe.output.lock().unwrap());assert_eq!(output.write(&[5,6,7]).unwrap(),3);output.close().unwrap();assert_eq!(value.get_bytes(1,5).await.unwrap(),vec![1,8,5,6,7]);value.truncate(2).await.unwrap();assert_eq!(value.length().await.unwrap(),2);value.free().await.unwrap();
}
#[tokio::test]
async fn task7_blob_value_self_pattern_is_delegated_without_snapshot(){
    let probe=blob_value_probe(ResourceOperations::blob());let value=blob_value(probe.clone());
    assert_eq!(value.position_blob(&value,1).await.unwrap(),Some(1));assert_eq!(value.position_blob(&value.clone(),2).await.unwrap(),None);assert!(value.position_blob(&value,0).await.is_err());
    assert_eq!(probe.same.load(Ordering::SeqCst),3);assert_eq!(*probe.calls.lock().unwrap(),vec!["position_blob";3]);
}
#[tokio::test]
async fn task7_blob_value_opposite_searches_use_consistent_lock_order(){
    let a_probe=blob_value_probe(ResourceOperations::blob());let b_probe=blob_value_probe(ResourceOperations::blob());let a=blob_value(a_probe.clone());let b=blob_value(b_probe.clone());let aa=a.clone();let bb=b.clone();
    let first=tokio::spawn(async move{a.position_blob(&b,1).await});let second=tokio::spawn(async move{bb.position_blob(&aa,1).await});
    tokio::time::timeout(Duration::from_secs(2),async{assert_eq!(first.await.unwrap().unwrap(),Some(1));assert_eq!(second.await.unwrap().unwrap(),Some(1));}).await.expect("opposite searches must not deadlock");
    for probe in [a_probe,b_probe]{let calls=probe.calls.lock().unwrap();assert_eq!(calls.iter().filter(|&&c|c=="length").count(),1);assert_eq!(calls.iter().filter(|&&c|c=="get_bytes").count(),1);assert_eq!(calls.iter().filter(|&&c|c=="position_blob").count(),1);}
}
#[tokio::test]
async fn task7_blob_value_closed_pattern_never_enters_target_driver(){
    let probe=blob_value_probe(ResourceOperations::blob());let value=blob_value(probe.clone());let pattern=blob_value(blob_value_probe(ResourceOperations::blob()));pattern.free().await.unwrap();assert!(value.position_blob(&pattern,1).await.is_err());assert!(probe.calls.lock().unwrap().is_empty());
}
#[tokio::test]
async fn task7_blob_value_free_error_cancel_panic_and_retry_confirmation(){
    let probe=blob_value_probe(ResourceOperations::blob());let value=blob_value(probe.clone());probe.failures.store(1,Ordering::SeqCst);assert_eq!(value.free().await.unwrap_err().to_string(),fatal_error().to_string());probe.free_started.notified().await;assert!(!value.is_freed());assert_eq!(value.length().await.unwrap(),4);
    probe.panic_free.store(true,Ordering::SeqCst);let child=value.clone();assert!(tokio::spawn(async move{child.free().await}).await.unwrap_err().is_panic());assert!(!value.is_freed());assert!(value.length().await.is_err());
    probe.block_free.store(true,Ordering::SeqCst);let child=value.clone();let task=tokio::spawn(async move{child.free().await});probe.free_started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());assert!(value.length().await.is_err());probe.block_free.store(false,Ordering::SeqCst);value.free().await.unwrap();assert!(value.is_freed());value.free().await.unwrap();assert_eq!(probe.free_calls.load(Ordering::SeqCst),4);
}
#[tokio::test]
async fn task7_blob_value_concurrent_free_is_once(){
    let probe=blob_value_probe(ResourceOperations::blob());probe.block_free.store(true,Ordering::SeqCst);let value=blob_value(probe.clone());let a=value.clone();let b=value.clone();let first=tokio::spawn(async move{a.free().await});probe.free_started.notified().await;let second=tokio::spawn(async move{b.free().await});assert!(value.length().await.is_err());probe.free_finish.notify_one();first.await.unwrap().unwrap();second.await.unwrap().unwrap();assert_eq!(probe.free_calls.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_blob_value_waiting_read_rechecks_state_after_free(){
    let probe=blob_value_probe(ResourceOperations::blob());probe.block_read.store(true,Ordering::SeqCst);let value=blob_value(probe.clone());let child=value.clone();let first=tokio::spawn(async move{child.length().await});probe.read_started.notified().await;
    let mut release=Box::pin(value.free());std::future::poll_fn(|cx|{assert!(std::future::Future::poll(release.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    let mut read=Box::pin(value.get_bytes(1,1));std::future::poll_fn(|cx|{assert!(std::future::Future::poll(read.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;probe.read_finish.notify_one();first.await.unwrap().unwrap();release.await.unwrap();assert!(read.await.is_err());assert_eq!(*probe.calls.lock().unwrap(),vec!["length"]);
}
#[tokio::test]
async fn task7_blob_value_pooled_cancellation_parent_close_is_once(){
    let control=Arc::new(Control::default());control.block_free.store(true,Ordering::SeqCst);let mut connection=pooled(control.clone()).await;let raw=connection.create_blob().await.unwrap();let probe=control.probes.lock().unwrap()[0].clone();let value=ResourceHandleFactory::blob(raw);let child=value.clone();
    let task=tokio::spawn(async move{child.free().await});probe.started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());assert!(value.length().await.is_err());let close=tokio::spawn(async move{connection.close().await});probe.finish.notify_one();close.await.unwrap().unwrap();value.free().await.unwrap();assert_eq!(probe.calls.load(Ordering::SeqCst),1);assert!(value.is_freed());
}
