use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{DatabaseMetaData, DatabaseMetaDataOperation, DatabaseMetaDataRequest};
/// 保留原数据库产品名称的标准元数据夹具。
pub(super) struct LeaseMetaData;
druid_rdbc::impl_standard_wrapper!(LeaseMetaData => DatabaseMetaData);
#[async_trait::async_trait]
impl DatabaseMetaData for LeaseMetaData {
    async fn metadata_string(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<Option<String>, RdbcError> {
        assert_eq!(
            request.operation(),
            DatabaseMetaDataOperation::GetDatabaseProductName
        );
        Ok(Some("lease-metadata".to_string()))
    }
}
