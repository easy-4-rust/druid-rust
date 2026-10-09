use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{
    Blob, BlobPattern, RdbcInputStream, RdbcOutputStream, ResourceOperations,
};
/// 保留原长度与一基范围观察的标准资源夹具。
/// 对应 Java: java.sql.Blob。未支持的操作明确返回0A000。
#[derive(Debug, Default)]
pub(super) struct LeaseBlobAccess {
    // 非零大小身份字段确保Box转移的对象地址可观测。
    _identity: u8,
}
#[async_trait::async_trait]
impl Blob for LeaseBlobAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ | ResourceOperations::RANGE | ResourceOperations::FREE
    }
    async fn length(&mut self) -> Result<i64, RdbcError> {
        Ok(4)
    }
    async fn get_bytes(&mut self, position: i64, length: i32) -> Result<Vec<u8>, RdbcError> {
        assert_eq!((position, length), (1, 4));
        Ok(vec![1, 2, 3, 4])
    }
    async fn get_binary_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("Blob#get_binary_stream"))
    }
    async fn position_bytes(
        &mut self,
        _pattern: &[u8],
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::feature_not_supported("Blob#position_bytes"))
    }
    async fn position_blob(
        &mut self,
        _pattern: BlobPattern<'_>,
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::feature_not_supported("Blob#position_blob"))
    }
    async fn set_bytes(&mut self, _position: i64, _bytes: &[u8]) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported("Blob#set_bytes"))
    }
    async fn set_bytes_range(
        &mut self,
        _position: i64,
        _bytes: &[u8],
        _offset: i32,
        _length: i32,
    ) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported("Blob#set_bytes_range"))
    }
    async fn set_binary_stream(&mut self, _position: i64) -> Result<RdbcOutputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("Blob#set_binary_stream"))
    }
    async fn truncate(&mut self, _length: i64) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("Blob#truncate"))
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        // 原夹具不持有外部资源，保留原默认free的幂等成功语义。
        Ok(())
    }
    async fn get_binary_stream_range(
        &mut self,
        _position: i64,
        _length: i64,
    ) -> Result<RdbcInputStream, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Blob#get_binary_stream_range",
        ))
    }
}
