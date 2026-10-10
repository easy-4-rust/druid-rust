use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{
    Clob, ClobPattern, NClob, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcWriter,
    ResourceOperations,
};
use druid_rdbc::types::RdbcString;
/// 保留原长度与一基范围观察的标准资源夹具。
/// 对应 Java: java.sql.Clob。未支持的操作明确返回0A000。
#[derive(Debug, Default)]
pub(super) struct LeaseClobAccess {
    // 非零大小身份字段确保Box转移的对象地址可观测。
    _identity: u8,
}
#[async_trait::async_trait]
impl Clob for LeaseClobAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ | ResourceOperations::RANGE | ResourceOperations::FREE
    }
    async fn length(&mut self) -> Result<i64, RdbcError> {
        Ok(4)
    }
    async fn get_sub_string(
        &mut self,
        position: i64,
        length: i32,
    ) -> Result<RdbcString, RdbcError> {
        assert_eq!((position, length), (1, 4));
        Ok(RdbcString::from("clob"))
    }
    async fn get_character_stream(&mut self) -> Result<RdbcReader, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Clob#get_character_stream",
        ))
    }
    async fn get_ascii_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#get_ascii_stream"))
    }
    async fn position_string(
        &mut self,
        _pattern: &RdbcString,
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#position_string"))
    }
    async fn position_clob(
        &mut self,
        _pattern: ClobPattern<'_>,
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#position_clob"))
    }
    async fn set_string(&mut self, _position: i64, _value: &RdbcString) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#set_string"))
    }
    async fn set_string_range(
        &mut self,
        _position: i64,
        _value: &RdbcString,
        _offset: i32,
        _length: i32,
    ) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#set_string_range"))
    }
    async fn set_ascii_stream(&mut self, _position: i64) -> Result<RdbcOutputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#set_ascii_stream"))
    }
    async fn set_character_stream(&mut self, _position: i64) -> Result<RdbcWriter, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Clob#set_character_stream",
        ))
    }
    async fn truncate(&mut self, _length: i64) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#truncate"))
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        // 原夹具不持有外部资源，保留原默认free的幂等成功语义。
        Ok(())
    }
    async fn get_character_stream_range(
        &mut self,
        _position: i64,
        _length: i64,
    ) -> Result<RdbcReader, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Clob#get_character_stream_range",
        ))
    }
}
impl NClob for LeaseClobAccess {}
