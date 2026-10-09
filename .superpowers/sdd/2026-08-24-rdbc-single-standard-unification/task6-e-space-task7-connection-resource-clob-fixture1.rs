use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{RdbcInputStream,RdbcOutputStream,RdbcReader,RdbcWriter,ResourceOperations};
use druid_rdbc::resource::Clob;
use druid_rdbc::resource::NClob;
use druid_rdbc::types::RdbcString;
use std::cell::Cell;
use std::future::Future;
#[derive(Debug)]
struct SendOnlyClob { units: Vec<u16>, freed: bool, releases: Cell<usize> }
impl SendOnlyClob {
    fn ensure_open(&self) -> Result<(),RdbcError> {
        if self.freed {Err(RdbcError::rdbc_resource_closed("Clob"))} else {Ok(())}
    }
    fn offset(position:i64) -> Result<usize,RdbcError> {
        position.checked_sub(1).filter(|value| *value>=0).and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| RdbcError::InvalidArgument("invalid 1-based position".into()))
    }
    fn write_units(&mut self, position:i64, units:&[u16]) -> Result<i32,RdbcError> {
        self.ensure_open()?;let start=Self::offset(position)?;
        if start>self.units.len() {return Err(RdbcError::InvalidArgument("write position outside clob".into()));}
        let count=i32::try_from(units.len()).map_err(|_|RdbcError::InvalidArgument("write length overflow".into()))?;
        let end=start.checked_add(units.len()).ok_or_else(||RdbcError::InvalidArgument("write range overflow".into()))?;
        self.units.resize(self.units.len().max(end),0);self.units[start..end].copy_from_slice(units);Ok(count)
    }
}
#[async_trait::async_trait]
impl Clob for SendOnlyClob {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ|ResourceOperations::WRITE|ResourceOperations::RANGE|
        ResourceOperations::SEARCH|ResourceOperations::TRUNCATE|ResourceOperations::FREE
    }
    async fn length(&mut self) -> Result<i64,RdbcError> {
        self.ensure_open()?;i64::try_from(self.units.len()).map_err(|_|RdbcError::InvalidArgument("length overflow".into()))
    }
    async fn get_sub_string(&mut self, position:i64, length:i32) -> Result<RdbcString,RdbcError> {
        self.ensure_open()?;let start=Self::offset(position)?;
        let count=usize::try_from(length).map_err(|_|RdbcError::InvalidArgument("negative length".into()))?;
        let tail=self.units.get(start..).ok_or_else(||RdbcError::InvalidArgument("position outside clob".into()))?;
        Ok(RdbcString::from_utf16(tail[..count.min(tail.len())].to_vec()))
    }
    async fn position_string(&mut self, pattern:&RdbcString, start:i64) -> Result<Option<i64>,RdbcError> {
        self.ensure_open()?;let offset=Self::offset(start)?;
        let tail=self.units.get(offset..).ok_or_else(||RdbcError::InvalidArgument("search outside clob".into()))?;
        if pattern.is_empty() {return Ok(Some(start));}
        Ok(tail.windows(pattern.len()).position(|part|part==pattern.as_utf16())
            .map(|found|i64::try_from(offset+found+1).expect("bounded clob length")))
    }
    async fn position_clob(&mut self, pattern:&mut dyn Clob, start:i64) -> Result<Option<i64>,RdbcError> {
        self.ensure_open()?;let count=i32::try_from(pattern.length().await?).map_err(|_|RdbcError::InvalidArgument("pattern too long".into()))?;
        let value=pattern.get_sub_string(1,count).await?;self.position_string(&value,start).await
    }
    async fn set_string(&mut self, position:i64, value:&RdbcString) -> Result<i32,RdbcError> {
        self.write_units(position,value.as_utf16())
    }
    async fn set_string_range(&mut self, position:i64, value:&RdbcString, offset:i32, length:i32) -> Result<i32,RdbcError> {
        self.ensure_open()?;
        let start=usize::try_from(offset).map_err(|_|RdbcError::InvalidArgument("negative input offset".into()))?;
        let count=usize::try_from(length).map_err(|_|RdbcError::InvalidArgument("negative input length".into()))?;
        let end=start.checked_add(count).ok_or_else(||RdbcError::InvalidArgument("input range overflow".into()))?;
        let units=value.as_utf16().get(start..end).ok_or_else(||RdbcError::InvalidArgument("input range outside string".into()))?;
        self.write_units(position,units)
    }
    async fn truncate(&mut self, length:i64) -> Result<(),RdbcError> {
        self.ensure_open()?;let count=usize::try_from(length).map_err(|_|RdbcError::InvalidArgument("negative truncate length".into()))?;
        if count>self.units.len() {return Err(RdbcError::InvalidArgument("truncate expands clob".into()));}
        self.units.truncate(count);Ok(())
    }
    async fn free(&mut self) -> Result<(),RdbcError> {
        if !self.freed {self.releases.set(self.releases.get()+1);self.freed=true;}Ok(())
    }
    async fn get_character_stream(&mut self) -> Result<RdbcReader,RdbcError> {
        self.ensure_open()?;Err(RdbcError::feature_not_supported("Clob#getCharacterStream"))
    }
    async fn get_ascii_stream(&mut self) -> Result<RdbcInputStream,RdbcError> {
        self.ensure_open()?;Err(RdbcError::feature_not_supported("Clob#getAsciiStream"))
    }
    async fn set_ascii_stream(&mut self,_position:i64) -> Result<RdbcOutputStream,RdbcError> {
        self.ensure_open()?;Err(RdbcError::feature_not_supported("Clob#setAsciiStream"))
    }
    async fn set_character_stream(&mut self,_position:i64) -> Result<RdbcWriter,RdbcError> {
        self.ensure_open()?;Err(RdbcError::feature_not_supported("Clob#setCharacterStream"))
    }
    async fn get_character_stream_range(&mut self,_position:i64,_length:i64) -> Result<RdbcReader,RdbcError> {
        self.ensure_open()?;Err(RdbcError::feature_not_supported("Clob#getCharacterStream(long,long)"))
    }
}
impl NClob for SendOnlyClob {}
fn clob() -> SendOnlyClob {SendOnlyClob {units:RdbcString::from("A😀Z").as_utf16().to_vec(),freed:false,releases:Cell::new(0)}}
fn send_future<F:Future+Send>(future:F) -> F {future}
fn send_static<T:Send+'static>() {}
#[tokio::test]
async fn send_only_n_clob_inherits_mutable_clob_and_send_futures() {
    send_static::<SendOnlyClob>();let mut resource:Box<dyn NClob>=Box::new(clob());
    let view:&mut dyn Clob=&mut *resource;
    assert_eq!(send_future(view.length()).await.unwrap(),4);
    assert_eq!(send_future(view.get_sub_string(2,2)).await.unwrap().as_utf16(),&[0xD83D,0xDE00]);
    assert_eq!(view.set_string(4,&RdbcString::from("Q")).await.unwrap(),1);
    assert_eq!(view.get_sub_string(4,99).await.unwrap().as_utf16(),&[81]);
}
#[tokio::test]
async fn clob_preserves_unpaired_surrogates_and_utf16_input_offsets() {
    let mut resource=clob();let lone=resource.get_sub_string(2,1).await.unwrap();
    assert_eq!(lone.as_utf16(),&[0xD83D]);assert!(lone.to_rust_string().is_err());
    assert_eq!(resource.set_string_range(1,&RdbcString::from("😀"),1,1).await.unwrap(),1);
    assert_eq!(resource.get_sub_string(1,4).await.unwrap().as_utf16(),&[0xDE00,0xD83D,0xDE00,90]);
    let before=resource.get_sub_string(1,4).await.unwrap();
    assert!(resource.set_string_range(1,&RdbcString::from("😀"),2,1).await.is_err());
    assert_eq!(resource.get_sub_string(1,4).await.unwrap(),before);
    resource.truncate(2).await.unwrap();
    assert_eq!(resource.get_sub_string(1,99).await.unwrap().as_utf16(),&[0xDE00,0xD83D]);
}
#[tokio::test]
async fn clob_search_uses_utf16_positions_and_standard_pattern_resource() {
    let mut resource=clob();assert_eq!(resource.position_string(&RdbcString::from("😀"),1).await.unwrap(),Some(2));
    assert_eq!(resource.position_string(&RdbcString::from("Z"),1).await.unwrap(),Some(4));
    let mut pattern=SendOnlyClob {units:vec![0xD83D,0xDE00],freed:false,releases:Cell::new(0)};
    assert_eq!(send_future(resource.position_clob(&mut pattern,1)).await.unwrap(),Some(2));
    assert_eq!(resource.position_string(&RdbcString::from("missing"),1).await.unwrap(),None);
    assert!(resource.get_sub_string(0,1).await.is_err());assert!(resource.get_sub_string(1,-1).await.is_err());
}
#[tokio::test]
async fn clob_closed_error_precedes_unsupported_for_every_jdbc_operation() {
    let mut resource=clob();assert!(!resource.capabilities().contains(ResourceOperations::STREAM));
    let unsupported=[resource.get_character_stream().await.err().unwrap(),resource.get_ascii_stream().await.err().unwrap(),
        resource.set_ascii_stream(1).await.err().unwrap(),resource.set_character_stream(1).await.err().unwrap(),
        resource.get_character_stream_range(1,1).await.err().unwrap()];
    for error in unsupported {assert_eq!(error.class_name(),"java.sql.SQLFeatureNotSupportedException");}
    resource.free().await.unwrap();resource.free().await.unwrap();let mut pattern=clob();let text=RdbcString::from("x");
    let closed=[resource.length().await.err().unwrap(),resource.get_sub_string(1,1).await.err().unwrap(),
        resource.get_character_stream().await.err().unwrap(),resource.get_ascii_stream().await.err().unwrap(),
        resource.position_string(&text,1).await.err().unwrap(),resource.position_clob(&mut pattern,1).await.err().unwrap(),
        resource.set_string(1,&text).await.err().unwrap(),resource.set_string_range(1,&text,0,1).await.err().unwrap(),
        resource.set_ascii_stream(1).await.err().unwrap(),resource.set_character_stream(1).await.err().unwrap(),
        resource.truncate(0).await.err().unwrap(),resource.get_character_stream_range(1,1).await.err().unwrap()];
    for error in closed {assert_eq!(error.class_name(),"java.sql.SQLException");}
    assert_eq!(resource.releases.get(),1);
}

pub fn resource() -> Box<dyn Clob> { Box::new(clob()) }
pub fn national_resource() -> Box<dyn NClob> { Box::new(clob()) }
