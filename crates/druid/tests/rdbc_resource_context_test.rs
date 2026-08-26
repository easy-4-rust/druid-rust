use druid::core::{
    RdbcBlobAccess, RdbcRefAccess, RdbcResourceAccess, ResourceHandleFactory, ResourceOperations,
};
use druid::core::{RdbcError, RdbcObject, RdbcTypeMap, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug)]
struct CountingAccess {
    free_count: AtomicUsize,
}

#[async_trait::async_trait]
impl RdbcResourceAccess for CountingAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::FREE
    }

    async fn free(&self) -> Result<(), RdbcError> {
        self.free_count.fetch_add(1, Ordering::SeqCst);
        tokio::task::yield_now().await;
        Ok(())
    }
}

#[async_trait::async_trait]
impl RdbcBlobAccess for CountingAccess {
    async fn length(&self) -> Result<i64, RdbcError> {
        Ok(0)
    }

    async fn get_bytes(&self, _position: i64, _length: i32) -> Result<Vec<u8>, RdbcError> {
        Ok(Vec::new())
    }
}

#[derive(Debug)]
struct ReadOnlyRefAccess;

#[async_trait::async_trait]
impl RdbcResourceAccess for ReadOnlyRefAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ
    }
}

#[async_trait::async_trait]
impl RdbcRefAccess for ReadOnlyRefAccess {
    async fn base_type_name(&self) -> Result<String, RdbcError> {
        Ok("schema.kind".to_string())
    }

    async fn object(&self) -> Result<RdbcObject, RdbcError> {
        Err(RdbcError::DriverError("read failed".to_string()))
    }

    async fn object_with_type_map(&self, _type_map: &RdbcTypeMap) -> Result<RdbcObject, RdbcError> {
        unreachable!("the standard handle does not advertise TYPE_MAP")
    }

    async fn set_object(&self, _value: RdbcObject) -> Result<(), RdbcError> {
        unreachable!("the standard handle does not advertise WRITE")
    }
}

#[tokio::test]
async fn detached_handle_free_is_exactly_once_under_concurrency() {
    let access = Arc::new(CountingAccess {
        free_count: AtomicUsize::new(0),
    });
    let blob = ResourceHandleFactory::blob(access.clone());

    let (first, second) = tokio::join!(blob.free(), blob.free());

    assert!(first.is_ok());
    assert!(second.is_ok());
    assert!(blob.free().await.is_ok());
    assert_eq!(access.free_count.load(Ordering::SeqCst), 1);
    assert!(blob.is_freed());
    assert!(!blob.resource_id().is_empty());
}

#[tokio::test]
async fn standard_handle_enforces_operations_without_druid_owner_runtime() {
    let reference = ResourceHandleFactory::reference(Arc::new(ReadOnlyRefAccess));
    let unsupported = reference
        .set_object(RdbcObject::from(Value::Int(1)))
        .await
        .expect_err("WRITE is not advertised");
    assert_eq!(
        unsupported.class_name(),
        "java.sql.SQLFeatureNotSupportedException"
    );
    assert_eq!(
        reference.object().await,
        Err(RdbcError::DriverError("read failed".to_string()))
    );
}
