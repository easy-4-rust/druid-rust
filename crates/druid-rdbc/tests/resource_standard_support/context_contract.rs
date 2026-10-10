use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{Blob, BlobPattern, RdbcInputStream, RdbcOutputStream, Ref};
use druid_rdbc::resource::{ResourceHandleFactory, ResourceOperations};
use druid_rdbc::types::{RdbcObject, RdbcTypeMap, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug)]
struct CountingAccess {
    free_count: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl Blob for CountingAccess {
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
    async fn get_binary_stream_range(
        &mut self,
        _position: i64,
        _length: i64,
    ) -> Result<RdbcInputStream, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Blob#get_binary_stream_range",
        ))
    }

    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::FREE
    }

    async fn free(&mut self) -> Result<(), RdbcError> {
        self.free_count.fetch_add(1, Ordering::SeqCst);
        tokio::task::yield_now().await;
        Ok(())
    }

    async fn length(&mut self) -> Result<i64, RdbcError> {
        Ok(0)
    }

    async fn get_bytes(&mut self, _position: i64, _length: i32) -> Result<Vec<u8>, RdbcError> {
        Ok(Vec::new())
    }
}

#[derive(Debug)]
struct ReadOnlyRefAccess;

#[async_trait::async_trait]
impl Ref for ReadOnlyRefAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ
    }

    async fn base_type_name(&mut self) -> Result<String, RdbcError> {
        Ok("schema.kind".to_string())
    }

    async fn object(&mut self) -> Result<RdbcObject, RdbcError> {
        Err(RdbcError::DriverError("read failed".to_string()))
    }

    async fn object_with_type_map(
        &mut self,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcObject, RdbcError> {
        unreachable!("the standard handle does not advertise TYPE_MAP")
    }

    async fn set_object(&mut self, _value: RdbcObject) -> Result<(), RdbcError> {
        unreachable!("the standard handle does not advertise WRITE")
    }
}

#[tokio::test]
async fn detached_handle_free_is_exactly_once_under_concurrency() {
    let access = Arc::new(CountingAccess {
        free_count: Arc::new(AtomicUsize::new(0)),
    });
    let blob = ResourceHandleFactory::blob(Box::new(CountingAccess {
        free_count: Arc::clone(&access.free_count),
    }));

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
    let reference = ResourceHandleFactory::reference(Box::new(ReadOnlyRefAccess));
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
