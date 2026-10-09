mod clob_value_contract {
use super::*;
use druid_rdbc::resource::{ClobPattern,RdbcClob,RdbcNClob,ResourceHandleFactory};
use std::cell::Cell;
#[derive(Debug)]
struct Probe {
    supported:ResourceOperations, units:Arc<Mutex<Vec<u16>>>,calls:Mutex<Vec<&'static str>>,
    frees:AtomicUsize,failures:AtomicUsize,panic_free:AtomicBool,block_free:AtomicBool,
    free_started:tokio::sync::Notify,free_finish:tokio::sync::Notify,
    block_read:AtomicBool,read_started:tokio::sync::Notify,read_finish:tokio::sync::Notify,
    same:AtomicUsize,reader:Mutex<Option<RdbcReader>>,writer:Mutex<Option<RdbcWriter>>,
    input:Mutex<Option<RdbcInputStream>>,output:Mutex<Option<RdbcOutputStream>>,
}
fn probe(supported:ResourceOperations)->Arc<Probe>{Arc::new(Probe{
    supported,units:Arc::new(Mutex::new(vec![65,0xD83D,0xDE00,90])),calls:Mutex::new(Vec::new()),
    frees:AtomicUsize::new(0),failures:AtomicUsize::new(0),panic_free:AtomicBool::new(false),block_free:AtomicBool::new(false),
    free_started:tokio::sync::Notify::new(),free_finish:tokio::sync::Notify::new(),block_read:AtomicBool::new(false),
    read_started:tokio::sync::Notify::new(),read_finish:tokio::sync::Notify::new(),same:AtomicUsize::new(0),
    reader:Mutex::new(None),writer:Mutex::new(None),input:Mutex::new(None),output:Mutex::new(None),
})}
#[derive(Debug)] struct Driver {probe:Arc<Probe>,freed:bool,not_sync:Cell<usize>}
fn offset(position:i64)->Result<usize,RdbcError>{position.checked_sub(1).filter(|v|*v>=0).and_then(|v|usize::try_from(v).ok()).ok_or_else(||RdbcError::InvalidArgument("invalid 1-based position".into()))}
fn count(value:i64)->Result<usize,RdbcError>{usize::try_from(value).map_err(|_|RdbcError::InvalidArgument("negative length".into()))}
fn write(units:&Arc<Mutex<Vec<u16>>>,position:usize,value:&[u16])->Result<usize,RdbcError>{
    let mut data=units.lock().unwrap();if position>data.len(){return Err(RdbcError::InvalidArgument("write outside clob".into()));}
    let end=position.checked_add(value.len()).ok_or_else(||RdbcError::InvalidArgument("overflow".into()))?;let length=data.len().max(end);data.resize(length,0);data[position..end].copy_from_slice(value);Ok(value.len())
}
#[derive(Debug)] struct CharacterSink {units:Arc<Mutex<Vec<u16>>>,position:usize}
impl druid_rdbc::extensions::CharacterWriter for CharacterSink {
    fn write_utf16(&mut self,value:&[u16])->Result<usize,RdbcError>{let n=write(&self.units,self.position,value)?;self.position+=n;Ok(n)}
    fn flush(&mut self)->Result<(),RdbcError>{Ok(())}
    fn close(&mut self)->Result<(),RdbcError>{Ok(())}
}
#[derive(Debug)] struct AsciiSink {units:Arc<Mutex<Vec<u16>>>,position:usize}
impl std::io::Write for AsciiSink {
    fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{let value=bytes.iter().map(|v|u16::from(*v)).collect::<Vec<_>>();let n=write(&self.units,self.position,&value).map_err(std::io::Error::other)?;self.position+=n;Ok(n)}
    fn flush(&mut self)->std::io::Result<()>{Ok(())}
}
impl Driver {
    fn enter(&self,name:&'static str)->Result<(),RdbcError>{if self.freed{return Err(RdbcError::rdbc_resource_closed("Clob"));}self.probe.calls.lock().unwrap().push(name);self.not_sync.set(self.not_sync.get()+1);Ok(())}
    fn range(&self,position:i64,length:i64,strict:bool)->Result<Vec<u16>,RdbcError>{let offset=offset(position)?;let n=count(length)?;let data=self.probe.units.lock().unwrap();let tail=data.get(offset..).ok_or_else(||RdbcError::InvalidArgument("range position".into()))?;if strict && n>tail.len(){return Err(RdbcError::InvalidArgument("range length".into()));}Ok(tail[..n.min(tail.len())].to_vec())}
    fn search(&self,pattern:&[u16],start:i64)->Result<Option<i64>,RdbcError>{let i=offset(start)?;let data=self.probe.units.lock().unwrap();let tail=data.get(i..).ok_or_else(||RdbcError::InvalidArgument("search outside clob".into()))?;if pattern.is_empty(){return Ok(Some(start));}Ok(tail.windows(pattern.len()).position(|w|w==pattern).map(|p|(i+p+1)as i64))}
}
#[async_trait::async_trait]
impl Clob for Driver {
    fn capabilities(&self)->ResourceOperations{self.probe.supported}
    async fn length(&mut self)->Result<i64,RdbcError>{self.enter("length")?;self.probe.read_started.notify_one();if self.probe.block_read.load(Ordering::SeqCst){self.probe.read_finish.notified().await;}Ok(self.probe.units.lock().unwrap().len()as i64)}
    async fn get_sub_string(&mut self,position:i64,length:i32)->Result<RdbcString,RdbcError>{self.enter("get_sub_string")?;Ok(RdbcString::from_utf16(self.range(position,i64::from(length),false)?))}
    async fn get_character_stream(&mut self)->Result<RdbcReader,RdbcError>{self.enter("get_character_stream")?;let reader=RdbcReader::from_utf16(self.probe.units.lock().unwrap().clone());*self.probe.reader.lock().unwrap()=Some(reader.clone());Ok(reader)}
    async fn get_ascii_stream(&mut self)->Result<RdbcInputStream,RdbcError>{self.enter("get_ascii_stream")?;let stream=RdbcInputStream::from_bytes(self.probe.units.lock().unwrap().iter().map(|v|u8::try_from(*v).unwrap_or(b'?')).collect());*self.probe.input.lock().unwrap()=Some(stream.clone());Ok(stream)}
    async fn position_string(&mut self,pattern:&RdbcString,start:i64)->Result<Option<i64>,RdbcError>{self.enter("position_string")?;self.search(pattern.as_utf16(),start)}
    async fn position_clob(&mut self,pattern:ClobPattern<'_>,start:i64)->Result<Option<i64>,RdbcError>{
        self.enter("position_clob")?;let units=match pattern {ClobPattern::SameResource=>{self.probe.same.fetch_add(1,Ordering::SeqCst);self.probe.units.lock().unwrap().clone()},ClobPattern::Other(pattern)=>{let n=i32::try_from(pattern.length().await?).map_err(|_|RdbcError::InvalidArgument("large pattern".into()))?;pattern.get_sub_string(1,n).await?.as_utf16().to_vec()}};self.search(&units,start)
    }
    async fn set_string(&mut self,position:i64,value:&RdbcString)->Result<i32,RdbcError>{self.enter("set_string")?;Ok(write(&self.probe.units,offset(position)?,value.as_utf16())?as i32)}
    async fn set_string_range(&mut self,position:i64,value:&RdbcString,input_offset:i32,length:i32)->Result<i32,RdbcError>{self.enter("set_string_range")?;let start=count(i64::from(input_offset))?;let end=start.checked_add(count(i64::from(length))?).ok_or_else(||RdbcError::InvalidArgument("input overflow".into()))?;let units=value.as_utf16().get(start..end).ok_or_else(||RdbcError::InvalidArgument("input range".into()))?;Ok(write(&self.probe.units,offset(position)?,units)?as i32)}
    async fn set_ascii_stream(&mut self,position:i64)->Result<RdbcOutputStream,RdbcError>{self.enter("set_ascii_stream")?;let stream=RdbcOutputStream::new(AsciiSink{units:self.probe.units.clone(),position:offset(position)?});*self.probe.output.lock().unwrap()=Some(stream.clone());Ok(stream)}
    async fn set_character_stream(&mut self,position:i64)->Result<RdbcWriter,RdbcError>{self.enter("set_character_stream")?;let writer=RdbcWriter::new(CharacterSink{units:self.probe.units.clone(),position:offset(position)?});*self.probe.writer.lock().unwrap()=Some(writer.clone());Ok(writer)}
    async fn truncate(&mut self,length:i64)->Result<(),RdbcError>{self.enter("truncate")?;let n=count(length)?;let mut units=self.probe.units.lock().unwrap();if n>units.len(){return Err(RdbcError::InvalidArgument("cannot expand".into()));}units.truncate(n);Ok(())}
    async fn free(&mut self)->Result<(),RdbcError>{self.probe.frees.fetch_add(1,Ordering::SeqCst);self.probe.free_started.notify_one();if self.probe.panic_free.swap(false,Ordering::SeqCst){panic!("controlled Clob free panic");}if self.probe.block_free.load(Ordering::SeqCst){self.probe.free_finish.notified().await;}if self.probe.failures.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|n|n.checked_sub(1)).is_ok(){return Err(fatal_error());}self.freed=true;Ok(())}
    async fn get_character_stream_range(&mut self,position:i64,length:i64)->Result<RdbcReader,RdbcError>{self.enter("get_character_stream_range")?;let reader=RdbcReader::from_utf16(self.range(position,length,true)?);*self.probe.reader.lock().unwrap()=Some(reader.clone());Ok(reader)}
}
impl NClob for Driver {}
fn driver(probe:Arc<Probe>)->Driver{Driver{probe,freed:false,not_sync:Cell::new(0)}}
fn clob(probe:Arc<Probe>)->RdbcClob{ResourceHandleFactory::clob(Box::new(driver(probe)))}
fn n_clob(probe:Arc<Probe>)->RdbcNClob{ResourceHandleFactory::n_clob(Box::new(driver(probe)))}
async fn operation(value:&RdbcClob,pattern:&RdbcClob,op:usize)->Result<(),RdbcError>{let text=RdbcString::from("Q");match op{
    0=>value.length().await.map(|_|()),1=>value.get_sub_string(1,2).await.map(|_|()),2=>value.get_character_stream().await.map(|_|()),3=>value.get_ascii_stream().await.map(|_|()),4=>value.position_string(&text,1).await.map(|_|()),5=>value.position_clob(pattern,1).await.map(|_|()),6=>value.set_string(1,&text).await.map(|_|()),7=>value.set_string_range(1,&text,0,1).await.map(|_|()),8=>value.set_ascii_stream(1).await.map(|_|()),9=>value.set_character_stream(1).await.map(|_|()),10=>value.truncate(3).await,11=>value.get_character_stream_range(1,2).await.map(|_|()),_=>panic!("unknown Clob operation")}}
#[tokio::test]
async fn all_capability_combinations_and_freed_precedence(){
    let flags=[ResourceOperations::READ,ResourceOperations::WRITE,ResourceOperations::SEARCH,ResourceOperations::TRUNCATE,ResourceOperations::STREAM,ResourceOperations::RANGE,ResourceOperations::FREE];
    let required=[flags[0],flags[0]|flags[5],flags[0]|flags[4],flags[0]|flags[4],flags[0]|flags[2],flags[0]|flags[2],flags[1],flags[1]|flags[5],flags[1]|flags[4],flags[1]|flags[4],flags[1]|flags[3],flags[0]|flags[4]|flags[5]];
    for national in [false,true]{for mask in 0..128{let mut supported=ResourceOperations::empty();for(bit,flag)in flags.iter().enumerate(){if mask&(1<<bit)!=0{supported|=*flag;}}
        let p=probe(supported|ResourceOperations::TYPE_MAP|ResourceOperations::RESULT_SET);let national_value=national.then(||n_clob(p.clone()));let value=if let Some(n)=&national_value{n.as_clob().clone()}else{clob(p.clone())};let pattern=clob(probe(ResourceOperations::clob()));assert_eq!(value.capabilities(),supported);
        for(op,required)in required.iter().enumerate(){let before=p.calls.lock().unwrap().len();let result=operation(&value,&pattern,op).await;if supported.contains(*required){assert!(result.is_ok(),"national {national} mask {mask} op {op}");assert_eq!(p.calls.lock().unwrap().len(),before+1);}else{assert_eq!(result.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(p.calls.lock().unwrap().len(),before);}}
        if supported.contains(ResourceOperations::FREE){value.free().await.unwrap();value.clone().free().await.unwrap();assert_eq!(p.frees.load(Ordering::SeqCst),1);for op in 0..12{let before=p.calls.lock().unwrap().len();assert_eq!(operation(&value,&pattern,op).await.unwrap_err().class_name(),"java.sql.SQLException");assert_eq!(p.calls.lock().unwrap().len(),before);}if let Some(n)=national_value{assert!(n.is_freed());n.free().await.unwrap();}}
        else{assert_eq!(value.free().await.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(p.frees.load(Ordering::SeqCst),0);assert!(!value.is_freed());}
    }}
}
#[tokio::test]
async fn utf16_offsets_and_original_stream_identity(){
    let p=probe(ResourceOperations::clob());let value=clob(p.clone());assert_eq!(value.length().await.unwrap(),4);assert_eq!(value,value.clone());assert_ne!(value,clob(probe(ResourceOperations::clob())));
    assert_eq!(value.get_sub_string(2,1).await.unwrap().as_utf16(),&[0xD83D]);assert_eq!(value.position_string(&RdbcString::from("😀"),1).await.unwrap(),Some(2));assert!(value.get_sub_string(0,1).await.is_err());
    assert_eq!(value.set_string_range(1,&RdbcString::from("😀"),1,1).await.unwrap(),1);assert_eq!(value.get_sub_string(1,1).await.unwrap().as_utf16(),&[0xDE00]);assert!(value.set_string_range(1,&RdbcString::from("😀"),2,1).await.is_err());
    let reader=value.get_character_stream_range(1,2).await.unwrap();assert_eq!(Some(reader.clone()),*p.reader.lock().unwrap());assert_eq!(reader.read_to_end_utf16().unwrap(),vec![0xDE00,0xD83D]);assert!(value.get_character_stream_range(4,2).await.is_err());
    let writer=value.set_character_stream(2).await.unwrap();assert_eq!(Some(writer.clone()),*p.writer.lock().unwrap());assert_eq!(writer.write_utf16(&[0xD800,0xDC00,0xDFFF]).unwrap(),3);writer.close().unwrap();assert_eq!(value.get_sub_string(2,3).await.unwrap().as_utf16(),&[0xD800,0xDC00,0xDFFF]);
    let output=value.set_ascii_stream(1).await.unwrap();assert_eq!(Some(output.clone()),*p.output.lock().unwrap());output.write(b"AB").unwrap();output.close().unwrap();let input=value.get_ascii_stream().await.unwrap();assert_eq!(Some(input.clone()),*p.input.lock().unwrap());assert_eq!(input.read_to_end().unwrap(),b"AB??");
    let full=value.get_character_stream().await.unwrap();assert_eq!(Some(full.clone()),*p.reader.lock().unwrap());assert_eq!(full.read_to_end_utf16().unwrap(),vec![65,66,0xDC00,0xDFFF]);value.truncate(2).await.unwrap();assert_eq!(value.length().await.unwrap(),2);
}
#[tokio::test]
async fn national_view_shares_identity_content_alias_and_free(){
    let p=probe(ResourceOperations::clob());let n=n_clob(p.clone());let view=n.as_clob().clone();assert_eq!(n.resource_id(),view.resource_id());assert_eq!(n,n.clone());
    n.set_string(1,&RdbcString::from("国")).await.unwrap();assert_eq!(view.get_sub_string(1,1).await.unwrap(),RdbcString::from("国"));assert_eq!(n.position_clob(&view,1).await.unwrap(),Some(1));assert_eq!(view.position_clob(n.as_clob(),2).await.unwrap(),None);assert_eq!(p.same.load(Ordering::SeqCst),2);
    view.free().await.unwrap();assert!(n.is_freed());assert!(n.length().await.unwrap_err().to_string().contains("NClob"));n.free().await.unwrap();assert_eq!(p.frees.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn self_pattern_preserves_start_and_no_snapshot(){
    let p=probe(ResourceOperations::clob());let value=clob(p.clone());assert_eq!(value.position_clob(&value,1).await.unwrap(),Some(1));assert_eq!(value.position_clob(&value.clone(),2).await.unwrap(),None);assert!(value.position_clob(&value,0).await.is_err());assert_eq!(p.same.load(Ordering::SeqCst),3);assert_eq!(*p.calls.lock().unwrap(),vec!["position_clob";3]);
}
#[tokio::test]
async fn target_admission_precedes_closed_pattern_in_both_orders(){
    for reverse in [false,true]{let target=probe(ResourceOperations::READ|ResourceOperations::FREE);let other=probe(ResourceOperations::clob());let (value,pattern)=if reverse{let pattern=clob(other);(clob(target.clone()),pattern)}else{(clob(target.clone()),clob(other))};pattern.free().await.unwrap();assert_eq!(value.position_clob(&pattern,1).await.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert!(target.calls.lock().unwrap().is_empty());value.free().await.unwrap();assert_eq!(value.position_clob(&pattern,1).await.unwrap_err().class_name(),"java.sql.SQLException");}
}
#[tokio::test]
async fn failure_panic_cancellation_retry_confirmation(){
    for national in [false,true]{let p=probe(ResourceOperations::clob());let n=national.then(||n_clob(p.clone()));let value=if let Some(n)=&n{n.as_clob().clone()}else{clob(p.clone())};p.failures.store(1,Ordering::SeqCst);assert_eq!(value.free().await.unwrap_err().to_string(),fatal_error().to_string());p.free_started.notified().await;assert!(!value.is_freed());assert_eq!(value.length().await.unwrap(),4);
        p.panic_free.store(true,Ordering::SeqCst);let v=value.clone();assert!(tokio::spawn(async move{v.free().await}).await.unwrap_err().is_panic());p.free_started.notified().await;assert!(value.length().await.is_err());assert!(!value.is_freed());
        p.block_free.store(true,Ordering::SeqCst);let v=value.clone();let task=tokio::spawn(async move{v.free().await});p.free_started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());assert!(!value.is_freed());assert!(value.length().await.is_err());p.block_free.store(false,Ordering::SeqCst);value.free().await.unwrap();value.free().await.unwrap();assert_eq!(p.frees.load(Ordering::SeqCst),4);if let Some(n)=n{assert!(n.is_freed());}
    }
}
#[tokio::test]
async fn concurrent_view_free_releases_driver_once(){
    let p=probe(ResourceOperations::clob());p.block_free.store(true,Ordering::SeqCst);let n=n_clob(p.clone());let view=n.as_clob().clone();let task=tokio::spawn(async move{view.free().await});p.free_started.notified().await;let second=tokio::spawn(async move{n.free().await});p.free_finish.notify_one();task.await.unwrap().unwrap();second.await.unwrap().unwrap();assert_eq!(p.frees.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn cancelled_search_releases_both_resource_locks(){
    let a=clob(probe(ResourceOperations::clob()));let p=probe(ResourceOperations::clob());p.block_read.store(true,Ordering::SeqCst);let b=clob(p.clone());let aa=a.clone();let bb=b.clone();let task=tokio::spawn(async move{aa.position_clob(&bb,1).await});p.read_started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());p.block_read.store(false,Ordering::SeqCst);tokio::time::timeout(Duration::from_secs(2),async{assert_eq!(a.length().await.unwrap(),4);assert_eq!(b.length().await.unwrap(),4);a.free().await.unwrap();b.free().await.unwrap();}).await.unwrap();
}
#[tokio::test]
async fn actual_pooled_values_preserve_views_and_cancel_safe_parent_cleanup(){
    for national in [false,true]{let control=Arc::new(Control::default());control.block_free.store(true,Ordering::SeqCst);let mut connection=pooled(control.clone()).await;
        let n=if national{Some(ResourceHandleFactory::n_clob(connection.create_n_clob().await.unwrap()))}else{None};let value=if let Some(n)=&n{n.as_clob().clone()}else{ResourceHandleFactory::clob(connection.create_clob().await.unwrap())};
        assert_eq!(value.position_clob(&value.clone(),1).await.unwrap(),Some(1));let probe=control.probes.lock().unwrap()[0].clone();let v=value.clone();let release=tokio::spawn(async move{v.free().await});probe.started.notified().await;release.abort();assert!(release.await.unwrap_err().is_cancelled());assert!(!value.is_freed());assert!(value.length().await.is_err());let mut parent=tokio::spawn(async move{connection.close().await});assert!(tokio::time::timeout(Duration::from_millis(20),&mut parent).await.is_err());assert!(control.returns.lock().unwrap().is_empty());probe.finish.notify_one();parent.await.unwrap().unwrap();value.free().await.unwrap();assert!(value.is_freed());if let Some(n)=n{assert!(n.is_freed());n.free().await.unwrap();}assert_eq!(probe.calls.load(Ordering::SeqCst),1);assert_eq!(control.returns.lock().unwrap().len(),1);
    }
}
#[cfg(task7_require_clob_value_sync)] fn negative_sync(){fn requires_sync<T:Sync>(){}requires_sync::<Driver>();}
#[cfg(task7_require_old_clob_access)] use druid_rdbc::resource::{RdbcClobAccess,RdbcNClobAccess};
}
