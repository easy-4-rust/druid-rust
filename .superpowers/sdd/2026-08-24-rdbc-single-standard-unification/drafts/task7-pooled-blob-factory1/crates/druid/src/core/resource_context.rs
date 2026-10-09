//! Druid connection-bound resource context。

use super::resource_id::ResourceId;
use super::resource_kind::ResourceKind;
use super::resource_owner::ResourceOwner;
use super::resource_runtime_capabilities::ResourceRuntimeCapabilities;
use super::resource_state::ResourceState;
use druid_rdbc::error::RdbcError;
use std::future::Future;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

/// Druid owner 的 resource identity/capability/exactly-once release context。
pub(crate) struct ResourceContext {
    resource_id: ResourceId,
    kind: ResourceKind,
    capabilities: ResourceRuntimeCapabilities,
    owner: Option<Arc<dyn ResourceOwner>>,
    state: AtomicU8,
    state_changed: Notify,
    #[cfg(test)]
    releasing_wait_hook: std::sync::Mutex<Option<(Arc<Notify>, Arc<Notify>)>>,
}

#[allow(dead_code)]
impl ResourceContext {
    pub(crate) fn new(
        resource_id: ResourceId,
        kind: ResourceKind,
        capabilities: ResourceRuntimeCapabilities,
        owner: Arc<dyn ResourceOwner>,
    ) -> Self {
        Self {
            resource_id,
            kind,
            capabilities,
            owner: Some(owner),
            state: AtomicU8::new(ResourceState::Open as u8),
            state_changed: Notify::new(),
            #[cfg(test)]
            releasing_wait_hook: std::sync::Mutex::new(None),
        }
    }

    pub(crate) fn detached(kind: ResourceKind, capabilities: ResourceRuntimeCapabilities) -> Self {
        Self {
            resource_id: ResourceId::local(),
            kind,
            capabilities,
            owner: None,
            state: AtomicU8::new(ResourceState::Open as u8),
            state_changed: Notify::new(),
            #[cfg(test)]
            releasing_wait_hook: std::sync::Mutex::new(None),
        }
    }

    pub(crate) fn resource_id(&self) -> &ResourceId {
        &self.resource_id
    }

    pub(crate) const fn kind(&self) -> ResourceKind {
        self.kind
    }

    pub(crate) const fn capabilities(&self) -> ResourceRuntimeCapabilities {
        self.capabilities
    }

    pub(crate) fn state(&self) -> ResourceState {
        ResourceState::from_code(self.state.load(Ordering::Acquire))
    }

    pub(crate) fn require(
        &self,
        capability: ResourceRuntimeCapabilities,
        operation: &'static str,
    ) -> Result<(), RdbcError> {
        if self.state() != ResourceState::Open {
            return Err(RdbcError::rdbc_resource_closed(self.kind.standard_name()));
        }
        if let Some(owner) = &self.owner {
            owner.ensure_open()?;
        }
        if !self.capabilities.contains(capability) {
            return Err(RdbcError::feature_not_supported(operation));
        }
        Ok(())
    }

    /// 将真实驱动操作失败通知 owner；参数为驱动结果，返回原值或原错误。
    pub(crate) fn classify_operation_result<T>(
        &self,
        result: Result<T, RdbcError>,
    ) -> Result<T, RdbcError> {
        if let (Some(owner), Err(error)) = (&self.owner, &result) {
            owner.resource_failed(&self.resource_id, self.kind, error);
        }
        result
    }

    pub(crate) async fn release_with<F, Fut>(&self, release: F) -> Result<(), RdbcError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(), RdbcError>>,
    {
        loop {
            let changed = self.state_changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let state = self.state();
            #[cfg(test)]
            self.pause_after_releasing_observation(state).await;
            match state {
                ResourceState::Freed => return Ok(()),
                ResourceState::Invalid => {
                    return Err(RdbcError::rdbc_resource_closed(self.kind.standard_name()));
                }
                ResourceState::Releasing => changed.as_mut().await,
                ResourceState::Open => {
                    if self
                        .state
                        .compare_exchange(
                            ResourceState::Open as u8,
                            ResourceState::Releasing as u8,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        break;
                    }
                }
            }
        }
        if let Err(error) = release().await {
            self.state
                .store(ResourceState::Open as u8, Ordering::Release);
            self.state_changed.notify_waiters();
            if let Some(owner) = &self.owner {
                owner.resource_failed(&self.resource_id, self.kind, &error);
            }
            return Err(error);
        }
        let owner_result = match &self.owner {
            Some(owner) => owner.resource_released(&self.resource_id, self.kind).await,
            None => Ok(()),
        };
        self.state
            .store(ResourceState::Freed as u8, Ordering::Release);
        self.state_changed.notify_waiters();
        owner_result
    }

    pub(crate) fn invalidate(&self) {
        self.state
            .store(ResourceState::Invalid as u8, Ordering::Release);
        self.state_changed.notify_waiters();
    }

    #[cfg(test)]
    fn install_releasing_wait_hook(&self, observed: Arc<Notify>, resume: Arc<Notify>) {
        *self
            .releasing_wait_hook
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((observed, resume));
    }

    #[cfg(test)]
    async fn pause_after_releasing_observation(&self, state: ResourceState) {
        if state != ResourceState::Releasing {
            return;
        }
        let hook = self
            .releasing_wait_hook
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some((observed, resume)) = hook {
            observed.notify_one();
            resume.notified().await;
        }
    }
}

impl Drop for ResourceContext {
    fn drop(&mut self) {
        if self.state() == ResourceState::Open {
            if let Some(owner) = &self.owner {
                owner.resource_abandoned(&self.resource_id, self.kind);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct CountingOwner {
        released: AtomicUsize,
        failed: AtomicUsize,
        abandoned: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl ResourceOwner for CountingOwner {
        async fn resource_released(
            &self,
            _resource_id: &ResourceId,
            _kind: ResourceKind,
        ) -> Result<(), RdbcError> {
            self.released.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn resource_failed(
            &self,
            _resource_id: &ResourceId,
            _kind: ResourceKind,
            _error: &RdbcError,
        ) {
            self.failed.fetch_add(1, Ordering::SeqCst);
        }

        fn resource_abandoned(&self, _resource_id: &ResourceId, _kind: ResourceKind) {
            self.abandoned.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn concurrent_release_notifies_druid_owner_exactly_once() {
        let owner = Arc::new(CountingOwner::default());
        let context = Arc::new(ResourceContext::new(
            ResourceId::new("blob-42"),
            ResourceKind::Blob,
            ResourceRuntimeCapabilities::FREE,
            owner.clone(),
        ));
        let releases = Arc::new(AtomicUsize::new(0));
        let first_releases = releases.clone();
        let second_releases = releases.clone();
        let (first, second) = tokio::join!(
            context.release_with(move || async move {
                first_releases.fetch_add(1, Ordering::SeqCst);
                tokio::task::yield_now().await;
                Ok(())
            }),
            context.release_with(move || async move {
                second_releases.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        );
        assert!(first.is_ok());
        assert!(second.is_ok());
        assert_eq!(releases.load(Ordering::SeqCst), 1);
        assert_eq!(owner.released.load(Ordering::SeqCst), 1);
        assert_eq!(context.state(), ResourceState::Freed);
    }

    #[tokio::test]
    async fn release_waiter_cannot_miss_winner_notification() {
        let context = Arc::new(ResourceContext::detached(
            ResourceKind::Blob,
            ResourceRuntimeCapabilities::FREE,
        ));
        let winner_started = Arc::new(Notify::new());
        let winner_resume = Arc::new(Notify::new());
        let winner_context = Arc::clone(&context);
        let winner_started_signal = Arc::clone(&winner_started);
        let winner_resume_wait = Arc::clone(&winner_resume);
        let winner = tokio::spawn(async move {
            winner_context
                .release_with(move || async move {
                    winner_started_signal.notify_one();
                    winner_resume_wait.notified().await;
                    Ok(())
                })
                .await
        });
        winner_started.notified().await;

        let loser_observed = Arc::new(Notify::new());
        let loser_resume = Arc::new(Notify::new());
        context.install_releasing_wait_hook(Arc::clone(&loser_observed), Arc::clone(&loser_resume));
        let loser_context = Arc::clone(&context);
        let loser = tokio::spawn(async move {
            loser_context
                .release_with(|| async {
                    panic!("loser must not release the resource");
                    #[allow(unreachable_code)]
                    Ok(())
                })
                .await
        });
        loser_observed.notified().await;
        winner_resume.notify_one();
        assert!(winner.await.expect("winner join").is_ok());
        loser_resume.notify_one();

        let loser_result = tokio::time::timeout(std::time::Duration::from_millis(100), loser)
            .await
            .expect("enabled waiter must observe completed release")
            .expect("loser join");
        assert!(loser_result.is_ok());
    }

    #[test]
    fn invalidation_and_abandonment_remain_druid_owner_policy() {
        let owner = Arc::new(CountingOwner::default());
        {
            let context = ResourceContext::new(
                ResourceId::new("ref-7"),
                ResourceKind::Ref,
                ResourceRuntimeCapabilities::READ,
                owner.clone(),
            );
            context.invalidate();
            assert!(context
                .require(ResourceRuntimeCapabilities::READ, "Ref#getObject")
                .is_err());
        }
        assert_eq!(owner.abandoned.load(Ordering::SeqCst), 0);
    }
}
