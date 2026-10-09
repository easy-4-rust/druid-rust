mod xml_value_contract {
use super::*;
use druid_rdbc::resource::{RdbcSqlXml,ResourceHandleFactory};
#[derive(Debug)]struct Probe {
    xml:Arc<XmlProbe>,supported:ResourceOperations,panic_read:AtomicBool,panic_write:AtomicBool,fail_write:AtomicBool,
    panic_free:AtomicBool,free_attempts:AtomicUsize,free_started:tokio::sync::Notify,
    input:Mutex<Option<RdbcInputStream>>,output:Mutex<Option<RdbcOutputStream>>,reader:Mutex<Option<RdbcReader>>,writer:Mutex<Option<RdbcWriter>>,
}
fn probe(supported:ResourceOperations,block_free:bool)->Arc<Probe>{Arc::new(Probe{xml:Arc::new(XmlProbe::new(block_free)),supported,panic_read:AtomicBool::new(false),panic_write:AtomicBool::new(false),fail_write:AtomicBool::new(false),panic_free:AtomicBool::new(false),free_attempts:AtomicUsize::new(0),free_started:tokio::sync::Notify::new(),input:Mutex::new(None),output:Mutex::new(None),reader:Mutex::new(None),writer:Mutex::new(None)})}
#[derive(Debug)]struct Driver{inner:XmlDriver,probe:Arc<Probe>}
fn driver(p:Arc<Probe>)->Driver{Driver{inner:XmlDriver{probe:p.xml.clone(),freed:false,not_sync:std::cell::Cell::new(0),stream:true},probe:p}}
fn value(p:Arc<Probe>)->RdbcSqlXml{ResourceHandleFactory::sql_xml(Box::new(driver(p)))}
#[async_trait::async_trait]
impl SqlXml for Driver {
    fn capabilities(&self)->ResourceOperations{self.probe.supported}
    async fn free(&mut self)->Result<(),RdbcError>{self.probe.free_attempts.fetch_add(1,Ordering::SeqCst);self.probe.free_started.notify_one();if self.probe.panic_free.swap(false,Ordering::SeqCst){panic!("controlled XML free panic");}self.inner.free().await}
    async fn binary_stream(&mut self)->Result<RdbcInputStream,RdbcError>{let stream=self.inner.binary_stream().await?;*self.probe.input.lock().unwrap()=Some(stream.clone());Ok(stream)}
    async fn set_binary_stream(&mut self)->Result<RdbcOutputStream,RdbcError>{let stream=self.inner.set_binary_stream().await?;*self.probe.output.lock().unwrap()=Some(stream.clone());Ok(stream)}
    async fn character_stream(&mut self)->Result<RdbcReader,RdbcError>{let reader=self.inner.character_stream().await?;*self.probe.reader.lock().unwrap()=Some(reader.clone());Ok(reader)}
    async fn set_character_stream(&mut self)->Result<RdbcWriter,RdbcError>{let writer=self.inner.set_character_stream().await?;*self.probe.writer.lock().unwrap()=Some(writer.clone());Ok(writer)}
    async fn string(&mut self)->Result<RdbcString,RdbcError>{let text=self.inner.string().await?;if self.probe.panic_read.swap(false,Ordering::SeqCst){panic!("controlled XML read panic");}Ok(text)}
    async fn set_string(&mut self,text:&RdbcString)->Result<(),RdbcError>{self.inner.set_string(text).await?;if self.probe.panic_write.swap(false,Ordering::SeqCst){panic!("controlled XML write panic");}if self.probe.fail_write.load(Ordering::SeqCst){return Err(RdbcError::SqlException(Box::new(SqlException::driver(89,"XML write failed").with_sql_state("22000"))));}Ok(())}
    async fn source(&mut self,representation:Option<&RdbcXmlRepresentationType>)->Result<RdbcXmlSource,RdbcError>{self.inner.source(representation).await}
    async fn result(&mut self,representation:Option<&RdbcXmlRepresentationType>)->Result<RdbcXmlResult,RdbcError>{self.inner.result(representation).await}
}
async fn read(v:&RdbcSqlXml,kind:usize)->Result<(),RdbcError>{match kind{0=>v.string().await.map(|_|()),1=>v.binary_stream().await.map(|_|()),2=>v.character_stream().await.map(|_|()),3=>v.source(None).await.map(|_|()),4=>v.get_string().await.map(|_|()),5=>v.get_binary_stream().await.map(|_|()),6=>v.get_character_stream().await.map(|_|()),_=>panic!("read kind")}}
async fn write(v:&RdbcSqlXml,kind:usize)->Result<(),RdbcError>{match kind{0=>v.set_string(&RdbcString::from("<v/>" )).await,1=>v.set_binary_stream().await.map(|_|()),2=>v.set_character_stream().await.map(|_|()),3=>v.result(None).await.map(|_|()),_=>panic!("write kind")}}
async fn operation(v:&RdbcSqlXml,op:usize)->Result<(),RdbcError>{if op<4{read(v,op).await}else{write(v,op-4).await}}
fn sequence_error(error:RdbcError){assert_eq!(error.class_name(),"java.sql.SQLException");assert_eq!(error.sql_exception().unwrap().sql_state(),Some("HY010"));}
#[tokio::test]
async fn every_capability_combination_and_freed_precedence(){
    let flags=[ResourceOperations::READ,ResourceOperations::WRITE,ResourceOperations::STREAM,ResourceOperations::FREE];let required=[flags[0],flags[0]|flags[2],flags[0]|flags[2],flags[0],flags[1],flags[1]|flags[2],flags[1]|flags[2],flags[1]];
    for mask in 0..16{let mut supported=ResourceOperations::empty();for(bit,flag)in flags.iter().enumerate(){if mask&(1<<bit)!=0{supported|=*flag;}}
        for(op,required)in required.iter().enumerate(){let p=probe(supported|ResourceOperations::TYPE_MAP|ResourceOperations::RESULT_SET|ResourceOperations::RANGE,false);let v=value(p.clone());assert_eq!(v.capabilities(),supported);let result=operation(&v,op).await;
            if supported.contains(*required){assert!(result.is_ok(),"mask {mask} op {op}");assert_eq!(p.xml.reads.load(Ordering::SeqCst)+p.xml.writes.load(Ordering::SeqCst),1);}else{assert_eq!(result.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(p.xml.reads.load(Ordering::SeqCst)+p.xml.writes.load(Ordering::SeqCst),0);}
            if supported.contains(ResourceOperations::FREE){v.free().await.unwrap();v.clone().free().await.unwrap();assert!(v.is_freed());assert_eq!(p.free_attempts.load(Ordering::SeqCst),1);let before=p.xml.reads.load(Ordering::SeqCst)+p.xml.writes.load(Ordering::SeqCst);for other in 0..8{assert_eq!(operation(&v,other).await.unwrap_err().class_name(),"java.sql.SQLException");}assert_eq!(p.xml.reads.load(Ordering::SeqCst)+p.xml.writes.load(Ordering::SeqCst),before);}
            else{assert_eq!(v.free().await.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(p.free_attempts.load(Ordering::SeqCst),0);assert!(!v.is_freed());}
        }
    }
}
#[tokio::test]
async fn aliases_share_all_read_and_write_consumption_gates(){
    for first in 0..7{for second in 0..7{let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());let alias=v.clone();assert_eq!(v,alias);assert_eq!(v.resource_id(),alias.resource_id());read(&v,first).await.unwrap();sequence_error(read(&alias,second).await.unwrap_err());assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);write(&v,0).await.unwrap();assert_eq!(p.xml.writes.load(Ordering::SeqCst),1);v.free().await.unwrap();}}
    for first in 0..4{for second in 0..4{let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());write(&v,first).await.unwrap();sequence_error(write(&v.clone(),second).await.unwrap_err());assert_eq!(p.xml.writes.load(Ordering::SeqCst),1);read(&v,0).await.unwrap();assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);v.free().await.unwrap();}}
}
#[tokio::test]
async fn unsupported_stream_does_not_consume_supported_text_direction(){
    let p=probe(ResourceOperations::READ|ResourceOperations::WRITE|ResourceOperations::FREE,false);let v=value(p.clone());assert!(v.get_binary_stream().await.is_err());assert!(v.get_character_stream().await.is_err());assert!(v.set_binary_stream().await.is_err());assert!(v.set_character_stream().await.is_err());let text=RdbcString::from_utf16(vec![60,0xD800,62]);v.set_string(&text).await.unwrap();assert_eq!(v.get_string().await.unwrap(),text);assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);assert_eq!(p.xml.writes.load(Ordering::SeqCst),1);v.free().await.unwrap();
}
#[tokio::test]
async fn original_streams_utf16_and_nullable_framework_identity(){
    let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());let writer=v.set_character_stream().await.unwrap();assert_eq!(Some(writer.clone()),*p.writer.lock().unwrap());writer.write_utf16(&[60,0xDFFF,62]).unwrap();writer.close().unwrap();let reader=v.get_character_stream().await.unwrap();assert_eq!(Some(reader.clone()),*p.reader.lock().unwrap());assert_eq!(reader.read_to_end_utf16().unwrap(),vec![60,0xDFFF,62]);reader.close().unwrap();v.free().await.unwrap();
    let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());let output=v.set_binary_stream().await.unwrap();assert_eq!(Some(output.clone()),*p.output.lock().unwrap());output.write(b"<bytes/>").unwrap();output.close().unwrap();assert_eq!(*p.xml.binary.lock().unwrap(),b"<bytes/>");let input=v.get_binary_stream().await.unwrap();assert_eq!(Some(input.clone()),*p.input.lock().unwrap());assert_eq!(input.read_to_end().unwrap(),"<v>😀</v>".as_bytes());input.close().unwrap();v.free().await.unwrap();
    for representation in [None,Some(RdbcXmlRepresentationType::Dom),Some(RdbcXmlRepresentationType::Stream),Some(RdbcXmlRepresentationType::Sax),Some(RdbcXmlRepresentationType::Stax),Some(RdbcXmlRepresentationType::Custom("vendor.ExactXml".into()))]{
        let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());let source=v.source(representation.as_ref()).await.unwrap();let result=v.result(representation.as_ref()).await.unwrap();assert_eq!(source,p.xml.sources.lock().unwrap()[0]);assert_eq!(result,p.xml.results.lock().unwrap()[0]);assert_eq!(*p.xml.representations.lock().unwrap(),vec![representation.clone(),representation]);sequence_error(v.source(None).await.unwrap_err());sequence_error(v.result(None).await.unwrap_err());v.free().await.unwrap();
    }
}
#[tokio::test]
async fn errors_and_panics_do_not_rearm_direction(){
    let p=probe(ResourceOperations::sql_xml(),false);p.xml.read_nonfatal.store(true,Ordering::SeqCst);let v=value(p.clone());assert_eq!(v.get_string().await.unwrap_err().sql_exception().unwrap().error_code(),88);p.xml.read_nonfatal.store(false,Ordering::SeqCst);sequence_error(v.source(None).await.unwrap_err());assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);v.set_string(&RdbcString::from("<valid/>" )).await.unwrap();v.free().await.unwrap();
    let p=probe(ResourceOperations::sql_xml(),false);p.fail_write.store(true,Ordering::SeqCst);let v=value(p.clone());assert_eq!(v.set_string(&RdbcString::from("<bad/>" )).await.unwrap_err().sql_exception().unwrap().error_code(),89);p.fail_write.store(false,Ordering::SeqCst);sequence_error(v.result(None).await.unwrap_err());assert_eq!(p.xml.writes.load(Ordering::SeqCst),1);v.get_string().await.unwrap();v.free().await.unwrap();
    for during_read in [false,true]{let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());if during_read{p.panic_read.store(true,Ordering::SeqCst);}else{p.panic_write.store(true,Ordering::SeqCst);}let alias=v.clone();let task=tokio::spawn(async move{if during_read{alias.string().await.map(|_|())}else{alias.set_string(&RdbcString::from("<panic/>" )).await}});assert!(task.await.unwrap_err().is_panic());if during_read{sequence_error(v.string().await.unwrap_err());assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);v.set_string(&RdbcString::from("<other/>" )).await.unwrap();}else{sequence_error(v.set_string(&RdbcString::from("<again/>" )).await.unwrap_err());assert_eq!(p.xml.writes.load(Ordering::SeqCst),1);v.string().await.unwrap();}v.free().await.unwrap();}
}
#[tokio::test]
async fn cancelled_entered_read_or_write_consumes_only_its_direction(){
    for during_read in [false,true]{let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());if during_read{p.xml.read_block.store(true,Ordering::SeqCst);}else{p.xml.write_block.store(true,Ordering::SeqCst);}let alias=v.clone();let task=tokio::spawn(async move{if during_read{alias.string().await.map(|_|())}else{alias.set_string(&RdbcString::from("<cancel/>" )).await}});if during_read{p.xml.read_started.notified().await;}else{p.xml.write_started.notified().await;}task.abort();assert!(task.await.unwrap_err().is_cancelled());p.xml.read_block.store(false,Ordering::SeqCst);p.xml.write_block.store(false,Ordering::SeqCst);if during_read{sequence_error(v.get_binary_stream().await.unwrap_err());v.set_string(&RdbcString::from("<other/>" )).await.unwrap();}else{sequence_error(v.set_character_stream().await.unwrap_err());v.get_string().await.unwrap();}assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);assert_eq!(p.xml.writes.load(Ordering::SeqCst),1);v.free().await.unwrap();}
}
#[tokio::test]
async fn cancelled_waiter_does_not_consume_direction_before_driver_entry(){
    let p=probe(ResourceOperations::sql_xml(),false);p.xml.write_block.store(true,Ordering::SeqCst);let v=value(p.clone());let alias=v.clone();let writing=tokio::spawn(async move{alias.set_string(&RdbcString::from("<held/>" )).await});p.xml.write_started.notified().await;
    let mut waiting=Box::pin(v.get_string());std::future::poll_fn(|cx|{assert!(std::future::Future::poll(waiting.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;drop(waiting);assert_eq!(p.xml.reads.load(Ordering::SeqCst),0);p.xml.write_finish.notify_one();writing.await.unwrap().unwrap();assert_eq!(v.get_string().await.unwrap(),RdbcString::from("<held/>"));assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);v.free().await.unwrap();
}
#[tokio::test]
async fn concurrent_clones_enter_read_driver_once(){
    let p=probe(ResourceOperations::sql_xml(),false);p.xml.read_block.store(true,Ordering::SeqCst);let v=value(p.clone());let alias=v.clone();let reading=tokio::spawn(async move{alias.string().await});p.xml.read_started.notified().await;let mut waiting=Box::pin(v.source(None));std::future::poll_fn(|cx|{assert!(std::future::Future::poll(waiting.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;p.xml.read_finish.notify_one();reading.await.unwrap().unwrap();sequence_error(waiting.await.unwrap_err());assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);v.free().await.unwrap();
}
#[tokio::test]
async fn free_failure_panic_cancel_and_concurrent_retry_confirm_real_release(){
    let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());p.xml.release.failures_remaining.store(1,Ordering::SeqCst);assert_eq!(v.free().await.unwrap_err().to_string(),fatal_error().to_string());p.free_started.notified().await;assert!(!v.is_freed());v.string().await.unwrap();p.panic_free.store(true,Ordering::SeqCst);let alias=v.clone();assert!(tokio::spawn(async move{alias.free().await}).await.unwrap_err().is_panic());assert!(!v.is_freed());assert!(v.set_string(&RdbcString::from("<x/>" )).await.is_err());v.free().await.unwrap();v.free().await.unwrap();assert_eq!(p.free_attempts.load(Ordering::SeqCst),3);assert!(v.is_freed());
    let p=probe(ResourceOperations::sql_xml(),true);let v=value(p.clone());let alias=v.clone();let task=tokio::spawn(async move{alias.free().await});p.xml.release.started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());assert!(!v.is_freed());assert!(v.string().await.is_err());let alias=v.clone();let retry=tokio::spawn(async move{alias.free().await});p.xml.release.started.notified().await;let alias=v.clone();let concurrent=tokio::spawn(async move{alias.free().await});p.xml.release.finish.notify_one();retry.await.unwrap().unwrap();concurrent.await.unwrap().unwrap();assert!(v.is_freed());assert_eq!(p.free_attempts.load(Ordering::SeqCst),2);assert_eq!(p.xml.release.calls.load(Ordering::SeqCst),2);
}
#[tokio::test]
async fn actual_pooled_value_cancellation_parent_cleanup_and_fatal_error(){
    let control=Arc::new(Control::default());control.block_free.store(true,Ordering::SeqCst);let mut connection=pooled(control.clone()).await;let v=ResourceHandleFactory::sql_xml(connection.create_sql_xml().await.unwrap());let probe=control.xml.lock().unwrap()[0].clone();v.set_string(&RdbcString::from("<pool/>" )).await.unwrap();assert_eq!(v.get_string().await.unwrap(),RdbcString::from("<pool/>"));let alias=v.clone();let task=tokio::spawn(async move{alias.free().await});probe.release.started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());assert!(!v.is_freed());let mut parent=tokio::spawn(async move{connection.close().await});assert!(tokio::time::timeout(Duration::from_millis(20),&mut parent).await.is_err());assert!(control.returns.lock().unwrap().is_empty());probe.release.finish.notify_one();parent.await.unwrap().unwrap();v.free().await.unwrap();assert!(v.is_freed());assert_eq!(probe.release.calls.load(Ordering::SeqCst),1);assert_eq!(control.returns.lock().unwrap().len(),1);
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;connection.set_exception_sorter(Arc::new(CountingSorter(control.clone())));let v=ResourceHandleFactory::sql_xml(connection.create_sql_xml().await.unwrap());control.xml.lock().unwrap()[0].release.read_failure.store(true,Ordering::SeqCst);assert_eq!(v.get_string().await.unwrap_err().sql_exception().unwrap().error_code(),77);assert!(connection.is_closed().unwrap());assert_eq!(control.sorter_calls.load(Ordering::SeqCst),1);assert_eq!(control.discards.load(Ordering::SeqCst),1);v.free().await.unwrap();connection.close().await.unwrap();assert!(!control.returns.lock().unwrap()[0].is_reusable());
}
#[cfg(task7_require_xml_value_sync)]fn negative_sync(){fn require_sync<T:Sync>(){}require_sync::<Driver>();}
#[cfg(task7_require_old_xml_access)]use druid_rdbc::resource::{RdbcSqlXmlAccess,RdbcResourceAccess};
}
