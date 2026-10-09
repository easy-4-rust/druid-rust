use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{RdbcBlobAccess, RdbcResourceAccess, ResourceOperations};

/// 保留原 Blob 长度和一基范围观察的真实访问夹具。
#[derive(Debug)]
pub(super) struct LeaseBlobAccess;

#[async_trait::async_trait]
impl RdbcResourceAccess for LeaseBlobAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::blob()
    }
}

#[async_trait::async_trait]
impl RdbcBlobAccess for LeaseBlobAccess {
    async fn length(&self) -> Result<i64, RdbcError> {
        Ok(4)
    }

    async fn get_bytes(&self, position: i64, length: i32) -> Result<Vec<u8>, RdbcError> {
        assert_eq!((position, length), (1, 4));
        Ok(vec![1, 2, 3, 4])
    }
}
