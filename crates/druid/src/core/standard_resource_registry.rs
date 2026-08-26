//! 单 logical lease 的 standard resource close registry。

use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_handle::StandardResourceHandle;
use super::standard_resource_registration::StandardResourceRegistration;
use super::standard_resource_registry_state::StandardResourceRegistryState;
use super::RdbcError;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// 保存 holder-owned async close commands，避免连接先于子 handle 回池。
pub(crate) struct StandardResourceRegistry {
    state: Mutex<StandardResourceRegistryState>,
    next_id: AtomicU64,
    changed: Notify,
}

impl Default for StandardResourceRegistry {
    fn default() -> Self {
        Self {
            state: Mutex::new(StandardResourceRegistryState::default()),
            next_id: AtomicU64::new(1),
            changed: Notify::new(),
        }
    }
}

impl StandardResourceRegistry {
    pub(crate) fn current_generation(&self) -> u64 {
        match &*self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            StandardResourceRegistryState::Open { generation, .. }
            | StandardResourceRegistryState::Closing { generation, .. }
            | StandardResourceRegistryState::Closed { generation, .. } => *generation,
        }
    }

    pub(crate) fn open_next_generation(&self) -> Result<u64, RdbcError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &mut *state {
            StandardResourceRegistryState::Open {
                generation,
                resources,
                pending,
            } if resources.is_empty() && *pending == 0 => Ok(*generation),
            StandardResourceRegistryState::Closed {
                generation,
                resources,
                close_error,
            } if resources.is_empty() && close_error.is_none() => {
                let generation = generation.saturating_add(1);
                *state = StandardResourceRegistryState::Open {
                    generation,
                    resources: BTreeMap::new(),
                    pending: 0,
                };
                Ok(generation)
            }
            StandardResourceRegistryState::Open { .. }
            | StandardResourceRegistryState::Closing { .. }
            | StandardResourceRegistryState::Closed { .. } => Err(RdbcError::ConnectionDiscarded),
        }
    }

    pub(crate) fn begin_admission(
        self: &Arc<Self>,
        expected_generation: u64,
    ) -> Result<StandardResourceAdmission, RdbcError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &mut *state {
            StandardResourceRegistryState::Open {
                generation,
                pending,
                ..
            } if *generation == expected_generation => {
                *pending = pending.checked_add(1).ok_or_else(|| {
                    RdbcError::Other("standard resource admission overflow".to_owned())
                })?;
                Ok(StandardResourceAdmission::new(
                    Arc::downgrade(self),
                    expected_generation,
                ))
            }
            StandardResourceRegistryState::Open { .. }
            | StandardResourceRegistryState::Closing { .. }
            | StandardResourceRegistryState::Closed { .. } => Err(RdbcError::ConnectionDiscarded),
        }
    }

    pub(crate) async fn register<T>(
        self: &Arc<Self>,
        permit: &StandardResourceAdmission,
        resource: Arc<T>,
    ) -> Result<(), RdbcError>
    where
        T: StandardResourceHandle + 'static,
    {
        let expected_generation = permit.generation();
        let id = self.next_id.fetch_add(1, Ordering::AcqRel);
        let registration =
            StandardResourceRegistration::new(Arc::downgrade(self), expected_generation, id);
        let accepted = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match &mut *state {
                StandardResourceRegistryState::Open {
                    generation,
                    resources,
                    ..
                } if permit.belongs_to(self)
                    && *generation == expected_generation
                    && resource.bind_registration(registration) =>
                {
                    resources.insert(id, Arc::clone(&resource) as Arc<dyn StandardResourceHandle>);
                    true
                }
                StandardResourceRegistryState::Open { .. }
                | StandardResourceRegistryState::Closing { .. }
                | StandardResourceRegistryState::Closed { .. } => false,
            }
        };
        if accepted {
            return Ok(());
        }

        match resource.close_resource().await {
            Ok(()) => Err(RdbcError::ConnectionDiscarded),
            Err(error) => {
                self.retain_retry(
                    expected_generation,
                    id,
                    Arc::clone(&resource) as Arc<dyn StandardResourceHandle>,
                    error.clone(),
                );
                Err(error)
            }
        }
    }

    pub(crate) fn has_open_resources(&self) -> bool {
        match &*self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            StandardResourceRegistryState::Open {
                resources, pending, ..
            } => {
                *pending > 0
                    || resources
                        .values()
                        .any(|resource| !resource.is_resource_closed())
            }
            StandardResourceRegistryState::Closing { .. } => true,
            StandardResourceRegistryState::Closed {
                resources,
                close_error,
                ..
            } => !resources.is_empty() || close_error.is_some(),
        }
    }

    pub(crate) async fn close_all(&self) -> Result<(), RdbcError> {
        let mut waited_generation = None;
        let (closing_generation, resources) = loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let outcome = {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match &mut *state {
                    StandardResourceRegistryState::Closing { generation, .. } => {
                        waited_generation = Some(*generation);
                        None
                    }
                    StandardResourceRegistryState::Closed {
                        generation,
                        close_error,
                        ..
                    } if waited_generation == Some(*generation) => {
                        return close_error.clone().map_or(Ok(()), Err)
                    }
                    StandardResourceRegistryState::Closed {
                        resources,
                        close_error,
                        ..
                    } if resources.is_empty() && close_error.is_none() => return Ok(()),
                    StandardResourceRegistryState::Open {
                        generation,
                        resources,
                        pending,
                    } => {
                        let generation = *generation;
                        let pending = *pending;
                        let resources = std::mem::take(resources);
                        *state = StandardResourceRegistryState::Closing {
                            generation,
                            resources: BTreeMap::new(),
                            pending,
                            close_error: None,
                        };
                        Some((generation, resources))
                    }
                    StandardResourceRegistryState::Closed {
                        generation,
                        resources,
                        ..
                    } => {
                        let generation = *generation;
                        let resources = std::mem::take(resources);
                        *state = StandardResourceRegistryState::Closing {
                            generation,
                            resources: BTreeMap::new(),
                            pending: 0,
                            close_error: None,
                        };
                        Some((generation, resources))
                    }
                }
            };
            if let Some(outcome) = outcome {
                break outcome;
            }
            notified.await;
        };
        let mut failed = BTreeMap::new();
        let mut first_error = None;
        for (id, resource) in resources.into_iter().rev() {
            if let Err(error) = resource.close_resource().await {
                first_error.get_or_insert(error);
                failed.insert(id, resource);
            }
        }
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match &mut *state {
                StandardResourceRegistryState::Closing {
                    generation,
                    resources,
                    close_error,
                    ..
                } if *generation == closing_generation => {
                    resources.append(&mut failed);
                    if close_error.is_none() {
                        *close_error = first_error.clone();
                    }
                }
                StandardResourceRegistryState::Open { .. }
                | StandardResourceRegistryState::Closing { .. }
                | StandardResourceRegistryState::Closed { .. } => {
                    return Err(RdbcError::ConnectionDiscarded);
                }
            }
        }
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let completion = {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match &mut *state {
                    StandardResourceRegistryState::Closing {
                        generation,
                        resources,
                        pending,
                        close_error,
                    } if *generation == closing_generation && *pending == 0 => {
                        let resources = std::mem::take(resources);
                        let error = close_error.clone();
                        *state = StandardResourceRegistryState::Closed {
                            generation: closing_generation,
                            resources,
                            close_error: error.clone(),
                        };
                        Some(error)
                    }
                    StandardResourceRegistryState::Closing { generation, .. }
                        if *generation == closing_generation =>
                    {
                        None
                    }
                    StandardResourceRegistryState::Open { .. }
                    | StandardResourceRegistryState::Closing { .. }
                    | StandardResourceRegistryState::Closed { .. } => {
                        return Err(RdbcError::ConnectionDiscarded)
                    }
                }
            };
            if let Some(error) = completion {
                self.changed.notify_waiters();
                return error.map_or(Ok(()), Err);
            }
            notified.await;
        }
    }

    pub(crate) fn unregister(&self, generation: u64, id: u64) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &mut *state {
            StandardResourceRegistryState::Open {
                generation: current,
                resources,
                ..
            }
            | StandardResourceRegistryState::Closing {
                generation: current,
                resources,
                ..
            }
            | StandardResourceRegistryState::Closed {
                generation: current,
                resources,
                ..
            } if *current == generation => {
                resources.remove(&id);
            }
            StandardResourceRegistryState::Open { .. }
            | StandardResourceRegistryState::Closing { .. }
            | StandardResourceRegistryState::Closed { .. } => {}
        }
    }

    pub(crate) fn finish_admission(&self, generation: u64) {
        let notify = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match &mut *state {
                StandardResourceRegistryState::Open {
                    generation: current,
                    pending,
                    ..
                }
                | StandardResourceRegistryState::Closing {
                    generation: current,
                    pending,
                    ..
                } if *current == generation && *pending > 0 => {
                    *pending -= 1;
                    *pending == 0
                }
                StandardResourceRegistryState::Open { .. }
                | StandardResourceRegistryState::Closing { .. }
                | StandardResourceRegistryState::Closed { .. } => false,
            }
        };
        if notify {
            self.changed.notify_waiters();
        }
    }

    fn retain_retry(
        &self,
        generation: u64,
        id: u64,
        resource: Arc<dyn StandardResourceHandle>,
        error: RdbcError,
    ) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &mut *state {
            StandardResourceRegistryState::Closing {
                generation: current,
                resources,
                close_error,
                ..
            }
            | StandardResourceRegistryState::Closed {
                generation: current,
                resources,
                close_error,
            } if *current == generation => {
                resources.insert(id, resource);
                if close_error.is_none() {
                    *close_error = Some(error);
                }
            }
            StandardResourceRegistryState::Open { .. }
            | StandardResourceRegistryState::Closing { .. }
            | StandardResourceRegistryState::Closed { .. } => {}
        }
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
}
