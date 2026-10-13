use async_trait::async_trait;
use druid_rdbc::connection::Connection;
use druid_rdbc::data_source::{
    CommonDataSource, XaConnection, XaDataSource, XaError, XaErrorCode, XaFlags, XaPrepareResult,
    XaResource, XaResourceManagerIdentity, Xid,
};
use druid_rdbc::error::RdbcError;

struct ExternalXaConnection {
    resource: ExternalXaResource,
}

druid_rdbc::impl_standard_wrapper!(ExternalXaConnection => Connection);

struct ExternalXaResource;

#[async_trait]
impl XaResource for ExternalXaResource {
    async fn start(&mut self, _xid: &Xid, _flags: i32) -> Result<(), XaError> {
        Ok(())
    }

    async fn end(&mut self, _xid: &Xid, _flags: i32) -> Result<(), XaError> {
        Ok(())
    }

    async fn prepare(&mut self, _xid: &Xid) -> Result<XaPrepareResult, XaError> {
        Ok(XaPrepareResult::ReadOnly)
    }

    async fn commit(&mut self, _xid: &Xid, _one_phase: bool) -> Result<(), XaError> {
        Ok(())
    }

    async fn rollback(&mut self, _xid: &Xid) -> Result<(), XaError> {
        Ok(())
    }

    async fn forget(&mut self, _xid: &Xid) -> Result<(), XaError> {
        Ok(())
    }

    async fn recover(&self, _flags: i32) -> Result<Vec<Xid>, XaError> {
        Ok(Vec::new())
    }

    async fn is_same_rm(&self, _other: &dyn XaResource) -> Result<bool, XaError> {
        Ok(false)
    }

    async fn get_transaction_timeout(&self) -> Result<u64, XaError> {
        Ok(0)
    }

    async fn set_transaction_timeout(&mut self, _seconds: u64) -> Result<bool, XaError> {
        Ok(false)
    }
}

#[async_trait]
impl Connection for ExternalXaConnection {
    async fn create_statement(
        &mut self,
        _options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::Statement>, RdbcError> {
        Err(RdbcError::feature_not_supported("test_create_statement"))
    }
}

#[async_trait]
impl druid_rdbc::data_source::PooledConnection for ExternalXaConnection {
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        Err(RdbcError::feature_not_supported("test_logical_connection"))
    }
}

impl XaConnection for ExternalXaConnection {
    fn xa_resource_name(&self) -> &str {
        "external-xa-rm"
    }

    fn xa_resource(&mut self) -> Result<&mut dyn XaResource, XaError> {
        Ok(&mut self.resource)
    }
}

struct ExternalXaDataSource;

impl CommonDataSource for ExternalXaDataSource {}

#[async_trait]
impl XaDataSource for ExternalXaDataSource {
    async fn xa_connection(&self) -> Result<Box<dyn XaConnection>, RdbcError> {
        Ok(Box::new(ExternalXaConnection {
            resource: ExternalXaResource,
        }))
    }
}

#[tokio::test]
async fn external_transaction_manager_uses_only_the_rdbc_xa_contract() {
    let data_source = ExternalXaDataSource;
    let mut connection = data_source.xa_connection().await.unwrap();
    let xid = Xid::new(42, vec![1, 2], vec![3]).unwrap();

    let resource = connection.xa_resource().unwrap();
    resource.start(&xid, XaFlags::TMNOFLAGS).await.unwrap();
    assert_eq!(
        resource.prepare(&xid).await.unwrap(),
        XaPrepareResult::ReadOnly
    );
    assert_eq!(
        XaError::new(XaErrorCode::XAER_PROTO, "invalid XA protocol transition").code(),
        XaErrorCode::XAER_PROTO
    );
}

#[test]
fn external_consumer_can_share_a_real_resource_manager_identity() {
    let identity = XaResourceManagerIdentity::new();
    let same_manager = identity.clone();
    let different_manager = XaResourceManagerIdentity::new();

    assert_eq!(identity, same_manager);
    assert_ne!(identity, different_manager);
}

#[tokio::test]
async fn unsupported_xa_connection_fails_with_feature_not_supported() {
    struct UnsupportedXaConnection;

    #[async_trait]
    impl druid_rdbc::data_source::PooledConnection for UnsupportedXaConnection {
        async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
            Err(RdbcError::feature_not_supported("unused"))
        }
    }

    impl XaConnection for UnsupportedXaConnection {
        fn xa_resource_name(&self) -> &str {
            "unsupported"
        }
    }

    let mut connection = UnsupportedXaConnection;
    match connection.xa_resource() {
        Err(error) => {
            assert_eq!(error.code(), XaErrorCode::XAER_RMERR);
            let source = error.source_error().expect("standard 0A000 cause");
            assert!(
                matches!(source, RdbcError::SqlException(exception) if exception.sql_state() == Some("0A000"))
            );
        }
        Ok(_) => panic!("legacy XA connection must remain explicitly unsupported"),
    }
}

#[test]
fn xa_identifiers_reject_oversized_components_with_structured_code() {
    let error = Xid::new(7, vec![0; 65], Vec::new()).unwrap_err();
    assert_eq!(error.code(), XaErrorCode::XAER_INVAL);
    assert!(error.message().contains("global_transaction_id"));
}
