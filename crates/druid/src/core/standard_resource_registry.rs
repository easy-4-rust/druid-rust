//! 单 logical lease 的 standard resource close registry。

use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_handle::StandardResourceHandle;
use super::standard_resource_registration::StandardResourceRegistration;
use super::standard_resource_registry_state::{
    StandardResourceCleanupState, StandardResourceRegistryEntry, StandardResourceRegistryState,
};
use super::RdbcError;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::{oneshot, Notify};

mod registry_a;
mod registry_b;

/// 保存 holder-owned async close commands，避免连接先于子 handle 回池。
pub(crate) struct StandardResourceRegistry {
    state: Mutex<StandardResourceRegistryState>,
    next_id: AtomicU64,
    changed: Arc<Notify>,
    shutting_down: Arc<AtomicBool>,
    shutdown: Arc<Notify>,
    // 资源拥有者创建时的执行器，允许调用方在普通线程丢弃 owning Stream。
    coordinator_runtime: Option<tokio::runtime::Handle>,
}

struct StandardResourceCoordinatorGuard {
    registry: Weak<StandardResourceRegistry>,
    generation: u64,
    completed: bool,
}

impl StandardResourceCoordinatorGuard {
    fn new(registry: Weak<StandardResourceRegistry>, generation: u64) -> Self {
        Self {
            registry,
            generation,
            completed: false,
        }
    }

    fn complete(&mut self) {
        self.completed = true;
    }
}

impl Drop for StandardResourceCoordinatorGuard {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        if let Some(registry) = self.registry.upgrade() {
            registry.coordinator_stopped(self.generation);
        }
    }
}

enum StandardResourceCoordinatorStep {
    Work {
        id: u64,
        resource: Arc<dyn StandardResourceHandle>,
    },
    Wait,
    Complete,
}

impl Default for StandardResourceRegistry {
    fn default() -> Self {
        Self {
            state: Mutex::new(StandardResourceRegistryState::default()),
            next_id: AtomicU64::new(1),
            changed: Arc::new(Notify::new()),
            shutting_down: Arc::new(AtomicBool::new(false)),
            shutdown: Arc::new(Notify::new()),
            coordinator_runtime: tokio::runtime::Handle::try_current().ok(),
        }
    }
}

impl Drop for StandardResourceRegistry {
    fn drop(&mut self) {
        self.shutting_down.store(true, Ordering::Release);
        self.shutdown.notify_waiters();
        self.changed.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RdbcError, StandardResourceHandle, StandardResourceRegistration, StandardResourceRegistry,
        StandardResourceRegistryState,
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::sync::Notify;

    struct CountingResource {
        close_count: AtomicUsize,
        closed: AtomicBool,
        failures_remaining: AtomicUsize,
        close_started: Option<Arc<Notify>>,
        close_release: Option<Arc<Notify>>,
        registration: std::sync::OnceLock<StandardResourceRegistration>,
    }

    impl CountingResource {
        fn immediate() -> Arc<Self> {
            Arc::new(Self {
                close_count: AtomicUsize::new(0),
                closed: AtomicBool::new(false),
                failures_remaining: AtomicUsize::new(0),
                close_started: None,
                close_release: None,
                registration: std::sync::OnceLock::new(),
            })
        }

        fn blocked(started: Arc<Notify>, release: Arc<Notify>) -> Arc<Self> {
            Arc::new(Self {
                close_count: AtomicUsize::new(0),
                closed: AtomicBool::new(false),
                failures_remaining: AtomicUsize::new(0),
                close_started: Some(started),
                close_release: Some(release),
                registration: std::sync::OnceLock::new(),
            })
        }

        fn fails_once() -> Arc<Self> {
            Arc::new(Self {
                close_count: AtomicUsize::new(0),
                closed: AtomicBool::new(false),
                failures_remaining: AtomicUsize::new(1),
                close_started: None,
                close_release: None,
                registration: std::sync::OnceLock::new(),
            })
        }
    }

    #[async_trait::async_trait]
    impl StandardResourceHandle for CountingResource {
        async fn close_resource(&self) -> Result<(), RdbcError> {
            if self.closed.load(Ordering::Acquire) {
                self.unregister_resource();
                return Ok(());
            }
            self.close_count.fetch_add(1, Ordering::AcqRel);
            if self
                .failures_remaining
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |remaining| {
                    (remaining > 0).then(|| remaining - 1)
                })
                .is_ok()
            {
                return Err(RdbcError::DriverError(
                    "forced late close failure".to_owned(),
                ));
            }
            if let Some(started) = &self.close_started {
                started.notify_waiters();
            }
            if let Some(release) = &self.close_release {
                release.notified().await;
            }
            self.closed.store(true, Ordering::Release);
            self.unregister_resource();
            Ok(())
        }

        fn is_resource_closed(&self) -> bool {
            self.closed.load(Ordering::Acquire)
        }

        fn registration_slot(&self) -> &std::sync::OnceLock<StandardResourceRegistration> {
            &self.registration
        }
    }

    #[tokio::test]
    async fn late_registration_after_close_is_rejected_and_closed() {
        let registry = Arc::new(StandardResourceRegistry::default());
        let permit = registry.begin_admission(0).expect("pending admission");
        let closing = Arc::clone(&registry);
        let close = tokio::spawn(async move { closing.close_all().await });
        wait_for_closing(&registry).await;
        let resource = CountingResource::immediate();
        assert!(registry
            .register(&permit, Arc::clone(&resource))
            .await
            .is_err());
        drop(permit);
        close.await.expect("close task").expect("close registry");
        assert_eq!(resource.close_count.load(Ordering::Acquire), 1);
        assert!(!registry.has_open_resources());
    }

    #[tokio::test]
    async fn explicit_close_unregisters_strong_entries_during_long_lease() {
        let registry = Arc::new(StandardResourceRegistry::default());
        let permit = registry.begin_admission(0).expect("pending admission");
        let mut weak = Vec::new();
        for _ in 0..128 {
            let resource = CountingResource::immediate();
            weak.push(Arc::downgrade(&resource));
            registry
                .register(&permit, Arc::clone(&resource))
                .await
                .expect("register");
            resource.close_resource().await.expect("explicit close");
        }
        assert!(weak
            .into_iter()
            .all(|resource| resource.upgrade().is_none()));
        drop(permit);
        assert!(!registry.has_open_resources());
        assert_eq!(retained_count(&registry), 0);
    }

    #[tokio::test]
    async fn concurrent_close_all_waits_for_one_exact_drain() {
        let registry = Arc::new(StandardResourceRegistry::default());
        let permit = registry.begin_admission(0).expect("pending admission");
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let resource = CountingResource::blocked(Arc::clone(&started), Arc::clone(&release));
        registry
            .register(&permit, Arc::clone(&resource))
            .await
            .expect("register");
        drop(permit);

        let started_wait = started.notified();
        tokio::pin!(started_wait);
        started_wait.as_mut().enable();
        let first_registry = Arc::clone(&registry);
        let first = tokio::spawn(async move { first_registry.close_all().await });
        started_wait.await;
        let second_registry = Arc::clone(&registry);
        let (second_done, mut second_observed) = tokio::sync::oneshot::channel();
        let second = tokio::spawn(async move {
            let result = second_registry.close_all().await;
            let _ = second_done.send(());
            result
        });
        tokio::task::yield_now().await;
        assert!(matches!(
            second_observed.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
        release.notify_one();
        first.await.expect("first task").expect("first close");
        second.await.expect("second task").expect("second close");
        assert_eq!(resource.close_count.load(Ordering::Acquire), 1);
    }

    #[tokio::test]
    async fn aborted_close_all_caller_does_not_cancel_registry_owned_drain() {
        let registry = Arc::new(StandardResourceRegistry::default());
        let permit = registry.begin_admission(0).expect("pending admission");
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let resource = CountingResource::blocked(Arc::clone(&started), Arc::clone(&release));
        let first_closed = CountingResource::immediate();
        registry
            .register(&permit, Arc::clone(&resource))
            .await
            .expect("register");
        registry
            .register(&permit, Arc::clone(&first_closed))
            .await
            .expect("register first-close resource");
        drop(permit);

        let started_wait = started.notified();
        tokio::pin!(started_wait);
        started_wait.as_mut().enable();
        let first_registry = Arc::clone(&registry);
        let first = tokio::spawn(async move { first_registry.close_all().await });
        started_wait.await;
        first.abort();
        assert!(first
            .await
            .expect_err("first close is aborted")
            .is_cancelled());

        let second_registry = Arc::clone(&registry);
        let second = tokio::spawn(async move { second_registry.close_all().await });
        release.notify_one();
        tokio::time::timeout(std::time::Duration::from_secs(2), second)
            .await
            .expect("second close must not inherit a leaderless Closing state")
            .expect("second close task")
            .expect("registry drain");
        assert_eq!(resource.close_count.load(Ordering::Acquire), 1);
        assert_eq!(first_closed.close_count.load(Ordering::Acquire), 1);
        assert!(resource.closed.load(Ordering::Acquire));
        assert!(first_closed.closed.load(Ordering::Acquire));
        assert!(!registry.has_open_resources());
        assert_eq!(retained_count(&registry), 0);
    }

    #[tokio::test]
    async fn dropping_registry_cancels_blocked_coordinator_task() {
        let registry = Arc::new(StandardResourceRegistry::default());
        let permit = registry.begin_admission(0).expect("pending admission");
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let resource = CountingResource::blocked(Arc::clone(&started), release);
        let weak_resource = Arc::downgrade(&resource);
        registry
            .register(&permit, Arc::clone(&resource))
            .await
            .expect("register");
        drop(permit);
        drop(resource);

        let started_wait = started.notified();
        tokio::pin!(started_wait);
        started_wait.as_mut().enable();
        let closing = Arc::clone(&registry);
        let close = tokio::spawn(async move { closing.close_all().await });
        started_wait.await;
        close.abort();
        assert!(close
            .await
            .expect_err("close caller aborted")
            .is_cancelled());
        drop(registry);

        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while weak_resource.upgrade().is_some() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("registry drop must stop its blocked coordinator task");
    }

    #[tokio::test]
    async fn pending_admission_cancellation_unblocks_close() {
        let registry = Arc::new(StandardResourceRegistry::default());
        let permit = registry.begin_admission(0).expect("pending admission");
        let closing = Arc::clone(&registry);
        let (done, mut observed) = tokio::sync::oneshot::channel();
        let close = tokio::spawn(async move {
            let result = closing.close_all().await;
            let _ = done.send(());
            result
        });
        wait_for_closing(&registry).await;
        assert!(matches!(
            observed.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
        drop(permit);
        close.await.expect("close task").expect("close registry");
    }

    #[tokio::test]
    async fn multiple_pending_admissions_all_finish_before_close_returns() {
        let registry = Arc::new(StandardResourceRegistry::default());
        let first = registry.begin_admission(0).expect("first permit");
        let second = registry.begin_admission(0).expect("second permit");
        let closing = Arc::clone(&registry);
        let (done, mut observed) = tokio::sync::oneshot::channel();
        let close = tokio::spawn(async move {
            let result = closing.close_all().await;
            let _ = done.send(());
            result
        });
        wait_for_closing(&registry).await;
        drop(first);
        tokio::task::yield_now().await;
        assert!(matches!(
            observed.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
        drop(second);
        close.await.expect("close task").expect("close registry");
    }

    #[tokio::test]
    async fn close_all_waits_for_rejected_late_resource_close_completion() {
        let registry = Arc::new(StandardResourceRegistry::default());
        let permit = registry.begin_admission(0).expect("pending admission");
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let resource = CountingResource::blocked(Arc::clone(&started), Arc::clone(&release));
        let closing = Arc::clone(&registry);
        let (done, mut observed) = tokio::sync::oneshot::channel();
        let close = tokio::spawn(async move {
            let result = closing.close_all().await;
            let _ = done.send(());
            result
        });
        wait_for_closing(&registry).await;
        let started_wait = started.notified();
        tokio::pin!(started_wait);
        started_wait.as_mut().enable();
        let registering = Arc::clone(&registry);
        let registration = tokio::spawn(async move {
            let result = registering.register(&permit, resource).await;
            drop(permit);
            result
        });
        started_wait.await;
        assert!(matches!(
            observed.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
        release.notify_one();
        assert!(registration.await.expect("register task").is_err());
        close.await.expect("close task").expect("close registry");
    }

    #[tokio::test]
    async fn cancelled_rejected_registration_keeps_registry_cleanup_ownership() {
        let registry = Arc::new(StandardResourceRegistry::default());
        let permit = registry.begin_admission(0).expect("pending admission");
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let resource = CountingResource::blocked(Arc::clone(&started), Arc::clone(&release));
        let observed_resource = Arc::clone(&resource);
        let closing = Arc::clone(&registry);
        let close = tokio::spawn(async move { closing.close_all().await });
        wait_for_closing(&registry).await;

        let started_wait = started.notified();
        tokio::pin!(started_wait);
        started_wait.as_mut().enable();
        let registering = Arc::clone(&registry);
        let registration =
            tokio::spawn(async move { registering.register(&permit, resource).await });
        started_wait.await;
        registration.abort();
        assert!(registration
            .await
            .expect_err("registration is aborted")
            .is_cancelled());
        release.notify_one();

        tokio::time::timeout(std::time::Duration::from_secs(2), close)
            .await
            .expect("close waits for registry-owned rejected cleanup")
            .expect("close task")
            .expect("close registry");
        assert_eq!(observed_resource.close_count.load(Ordering::Acquire), 1);
        assert!(observed_resource.closed.load(Ordering::Acquire));
        assert!(!registry.has_open_resources());
    }

    #[tokio::test]
    async fn late_registration_close_failure_stays_old_generation_and_blocks_reopen() {
        let registry = Arc::new(StandardResourceRegistry::default());
        let permit = registry.begin_admission(0).expect("pending admission");
        let closing = Arc::clone(&registry);
        let close = tokio::spawn(async move { closing.close_all().await });
        wait_for_closing(&registry).await;
        let resource = CountingResource::fails_once();
        assert!(matches!(
            registry.register(&permit, Arc::clone(&resource)).await,
            Err(RdbcError::DriverError(_))
        ));
        drop(permit);
        assert!(close.await.expect("close task").is_err());
        assert!(registry.open_next_generation().is_err());
        assert_eq!(resource.close_count.load(Ordering::Acquire), 1);
        registry.close_all().await.expect("retry old generation");
        assert_eq!(resource.close_count.load(Ordering::Acquire), 2);
        assert_eq!(registry.open_next_generation().expect("reopen"), 1);
    }

    async fn wait_for_closing(registry: &StandardResourceRegistry) {
        loop {
            if matches!(
                &*registry
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
                StandardResourceRegistryState::Closing { .. }
            ) {
                return;
            }
            tokio::task::yield_now().await;
        }
    }

    fn retained_count(registry: &StandardResourceRegistry) -> usize {
        match &*registry
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            StandardResourceRegistryState::Open { resources, .. }
            | StandardResourceRegistryState::Closing { resources, .. }
            | StandardResourceRegistryState::Closed { resources, .. } => resources.len(),
        }
    }
}
