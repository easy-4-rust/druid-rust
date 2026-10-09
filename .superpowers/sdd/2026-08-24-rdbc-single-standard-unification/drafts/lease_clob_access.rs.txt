use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{
    RdbcClobAccess, RdbcNClobAccess, RdbcResourceAccess, ResourceOperations,
};
use druid_rdbc::types::RdbcString;

/// 保留原 Clob/NClob 长度和一基子串观察的真实访问夹具。
#[derive(Debug)]
pub(super) struct LeaseClobAccess;

#[async_trait::async_trait]
impl RdbcResourceAccess for LeaseClobAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::clob()
    }
}

#[async_trait::async_trait]
impl RdbcClobAccess for LeaseClobAccess {
    async fn length(&self) -> Result<i64, RdbcError> {
        Ok(4)
    }

    async fn get_sub_string(&self, position: i64, length: i32) -> Result<RdbcString, RdbcError> {
        assert_eq!((position, length), (1, 4));
        Ok(RdbcString::from("clob"))
    }
}

impl RdbcNClobAccess for LeaseClobAccess {}
