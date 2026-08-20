extern crate druid_core as druid;
use druid_core::core::{DruidError, RdbcObject, RdbcTypeMap, Value};
use druid_core::spi::{
    RdbcRefAccess, RdbcResourceAccess, RdbcResourceCapabilities, RdbcResourceContext,
    RdbcResourceFactory, RdbcResourceId, RdbcResourceKind, RdbcResourceOwner, RdbcResourceState,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug)]
struct CountingAccess {
    free_count: AtomicUsize,
}

impl CountingAccess {
    fn new() -> Self {
        Self {
            free_count: AtomicUsize::new(0),
        }
    }
}

#[async_trait::async_trait]
impl RdbcResourceAccess for CountingAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        RdbcResourceCapabilities::FREE
    }

    async fn free(&self) -> Result<(), DruidError> {
        self.free_count.fetch_add(1, Ordering::SeqCst);
        tokio::task::yield_now().await;
        Ok(())
    }
}

#[derive(Default)]
struct CountingOwner {
    releases: AtomicUsize,
    failures: AtomicUsize,
    abandonments: AtomicUsize,
}

#[async_trait::async_trait]
impl RdbcResourceOwner for CountingOwner {
    async fn resource_released(
        &self,
        _resource_id: &RdbcResourceId,
        _kind: RdbcResourceKind,
    ) -> Result<(), DruidError> {
        self.releases.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn resource_failed(
        &self,
        _resource_id: &RdbcResourceId,
        _kind: RdbcResourceKind,
        _error: &DruidError,
    ) {
        self.failures.fetch_add(1, Ordering::SeqCst);
    }

    fn resource_abandoned(&self, _resource_id: &RdbcResourceId, _kind: RdbcResourceKind) {
        self.abandonments.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Debug)]
struct ReadOnlyRefAccess;

#[async_trait::async_trait]
impl RdbcResourceAccess for ReadOnlyRefAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        RdbcResourceCapabilities::READ
    }
}

#[async_trait::async_trait]
impl RdbcRefAccess for ReadOnlyRefAccess {
    async fn base_type_name(&self) -> Result<String, DruidError> {
        Ok("schema.kind".to_string())
    }

    async fn object(&self) -> Result<RdbcObject, DruidError> {
        Err(DruidError::DriverError("read failed".to_string()))
    }

    async fn object_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcObject, DruidError> {
        unreachable!("the context does not advertise TYPE_MAP")
    }

    async fn set_object(&self, _value: RdbcObject) -> Result<(), DruidError> {
        unreachable!("the context does not advertise WRITE")
    }
}

#[tokio::test]
async fn concurrent_free_releases_access_and_owner_exactly_once() {
    let owner = Arc::new(CountingOwner::default());
    let context = Arc::new(RdbcResourceContext::new(
        RdbcResourceId::new("blob-42"),
        RdbcResourceKind::Blob,
        RdbcResourceCapabilities::FREE,
        owner.clone(),
    ));
    let access = Arc::new(CountingAccess::new());

    let (first, second) =
        tokio::join!(context.free(access.as_ref()), context.free(access.as_ref()));

    assert!(first.is_ok());
    assert!(second.is_ok());
    assert!(context.free(access.as_ref()).await.is_ok());
    assert_eq!(access.free_count.load(Ordering::SeqCst), 1);
    assert_eq!(owner.releases.load(Ordering::SeqCst), 1);
    assert_eq!(owner.abandonments.load(Ordering::SeqCst), 0);
    assert_eq!(context.state(), RdbcResourceState::Freed);
}

#[tokio::test]
async fn handle_enforces_capabilities_reports_failures_and_honors_invalidation() {
    let owner = Arc::new(CountingOwner::default());
    let context = Arc::new(RdbcResourceContext::new(
        RdbcResourceId::new("ref-7"),
        RdbcResourceKind::Ref,
        RdbcResourceCapabilities::READ,
        owner.clone(),
    ));
    let reference =
        RdbcResourceFactory::reference_with_context(context.clone(), Arc::new(ReadOnlyRefAccess))
            .unwrap();

    let unsupported = reference
        .set_object(RdbcObject::from(Value::Int(1)))
        .await
        .unwrap_err();
    assert_eq!(
        unsupported.class_name(),
        "java.sql.SQLFeatureNotSupportedException"
    );
    assert_eq!(
        unsupported.sql_exception().unwrap().sql_state(),
        Some("0A000")
    );
    assert_eq!(owner.failures.load(Ordering::SeqCst), 0);

    assert_eq!(
        reference.object().await,
        Err(DruidError::DriverError("read failed".to_string()))
    );
    assert_eq!(owner.failures.load(Ordering::SeqCst), 1);

    context.invalidate();
    let closed = reference.base_type_name().await.unwrap_err();
    assert_eq!(closed.sql_exception().unwrap().sql_state(), Some("HY010"));
    assert_eq!(reference.state(), RdbcResourceState::Invalid);
}

#[test]
fn dropping_an_open_bound_context_notifies_its_owner() {
    let owner = Arc::new(CountingOwner::default());
    {
        let _context = RdbcResourceContext::new(
            RdbcResourceId::new("array-abandoned"),
            RdbcResourceKind::Array,
            RdbcResourceCapabilities::array(),
            owner.clone(),
        );
    }
    assert_eq!(owner.abandonments.load(Ordering::SeqCst), 1);
}
