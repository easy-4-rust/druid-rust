//! 单 logical lease 的 standard resource close registry。

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
            } if resources.is_empty() => Ok(*generation),
            StandardResourceRegistryState::Closed {
                generation,
                resources,
            } if resources.is_empty() => {
                let generation = generation.saturating_add(1);
                *state = StandardResourceRegistryState::Open {
                    generation,
                    resources: BTreeMap::new(),
                };
                Ok(generation)
            }
            StandardResourceRegistryState::Open { .. }
            | StandardResourceRegistryState::Closing { .. }
            | StandardResourceRegistryState::Closed { .. } => Err(RdbcError::ConnectionDiscarded),
        }
    }

    pub(crate) async fn register<T>(
        self: &Arc<Self>,
        expected_generation: u64,
        resource: Arc<T>,
    ) -> Result<(), RdbcError>
    where
        T: StandardResourceHandle + 'static,
    {
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
                } if *generation == expected_generation
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

        let close_result = resource.close_resource().await;
        if close_result.is_err() {
            self.retain_retry(id, Arc::clone(&resource) as Arc<dyn StandardResourceHandle>);
        }
        Err(RdbcError::ConnectionDiscarded)
    }

    pub(crate) fn has_open_resources(&self) -> bool {
        match &*self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            StandardResourceRegistryState::Open { resources, .. } => resources
                .values()
                .any(|resource| !resource.is_resource_closed()),
            StandardResourceRegistryState::Closing { .. } => true,
            StandardResourceRegistryState::Closed { resources, .. } => !resources.is_empty(),
        }
    }

    pub(crate) async fn close_all(&self) -> Result<(), RdbcError> {
        let resources = loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let outcome = {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match &mut *state {
                    StandardResourceRegistryState::Closing { .. } => None,
                    StandardResourceRegistryState::Closed { resources, .. }
                        if resources.is_empty() =>
                    {
                        return Ok(())
                    }
                    StandardResourceRegistryState::Open {
                        generation,
                        resources,
                    }
                    | StandardResourceRegistryState::Closed {
                        generation,
                        resources,
                    } => {
                        let generation = *generation;
                        let resources = std::mem::take(resources);
                        *state = StandardResourceRegistryState::Closing {
                            generation,
                            resources: BTreeMap::new(),
                        };
                        Some(resources)
                    }
                }
            };
            if let Some(resources) = outcome {
                break resources;
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
                } => {
                    resources.append(&mut failed);
                    let generation = *generation;
                    let resources = std::mem::take(resources);
                    *state = StandardResourceRegistryState::Closed {
                        generation,
                        resources,
                    };
                }
                StandardResourceRegistryState::Open { .. }
                | StandardResourceRegistryState::Closed { .. } => {
                    return Err(RdbcError::ConnectionDiscarded);
                }
            }
        }
        self.changed.notify_waiters();
        first_error.map_or(Ok(()), Err)
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
            }
            | StandardResourceRegistryState::Closed {
                generation: current,
                resources,
            } if *current == generation => {
                resources.remove(&id);
            }
            StandardResourceRegistryState::Open { .. }
            | StandardResourceRegistryState::Closing { .. }
            | StandardResourceRegistryState::Closed { .. } => {}
        }
    }

    fn retain_retry(&self, id: u64, resource: Arc<dyn StandardResourceHandle>) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &mut *state {
            StandardResourceRegistryState::Open { resources, .. }
            | StandardResourceRegistryState::Closing { resources, .. }
            | StandardResourceRegistryState::Closed { resources, .. } => {
                resources.insert(id, resource);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RdbcError, StandardResourceHandle, StandardResourceRegistration, StandardResourceRegistry,
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::sync::Notify;

    struct CountingResource {
        close_count: AtomicUsize,
        closed: AtomicBool,
        close_started: Option<Arc<Notify>>,
        close_release: Option<Arc<Notify>>,
        registration: std::sync::OnceLock<StandardResourceRegistration>,
    }

    impl CountingResource {
        fn immediate() -> Arc<Self> {
            Arc::new(Self {
                close_count: AtomicUsize::new(0),
                closed: AtomicBool::new(false),
                close_started: None,
                close_release: None,
                registration: std::sync::OnceLock::new(),
            })
        }

        fn blocked(started: Arc<Notify>, release: Arc<Notify>) -> Arc<Self> {
            Arc::new(Self {
                close_count: AtomicUsize::new(0),
                closed: AtomicBool::new(false),
                close_started: Some(started),
                close_release: Some(release),
                registration: std::sync::OnceLock::new(),
            })
        }
    }

    #[async_trait::async_trait]
    impl StandardResourceHandle for CountingResource {
        async fn close_resource(&self) -> Result<(), RdbcError> {
            if self.closed.swap(true, Ordering::AcqRel) {
                self.unregister_resource();
                return Ok(());
            }
            self.close_count.fetch_add(1, Ordering::AcqRel);
            if let Some(started) = &self.close_started {
                started.notify_waiters();
            }
            if let Some(release) = &self.close_release {
                release.notified().await;
            }
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
        registry.close_all().await.expect("seal empty registry");
        let resource = CountingResource::immediate();
        assert!(registry.register(0, Arc::clone(&resource)).await.is_err());
        assert_eq!(resource.close_count.load(Ordering::Acquire), 1);
        assert!(!registry.has_open_resources());
    }

    #[tokio::test]
    async fn explicit_close_unregisters_strong_entries_during_long_lease() {
        let registry = Arc::new(StandardResourceRegistry::default());
        let mut weak = Vec::new();
        for _ in 0..128 {
            let resource = CountingResource::immediate();
            weak.push(Arc::downgrade(&resource));
            registry
                .register(0, Arc::clone(&resource))
                .await
                .expect("register");
            resource.close_resource().await.expect("explicit close");
        }
        assert!(weak
            .into_iter()
            .all(|resource| resource.upgrade().is_none()));
        assert!(!registry.has_open_resources());
    }

    #[tokio::test]
    async fn concurrent_close_all_waits_for_one_exact_drain() {
        let registry = Arc::new(StandardResourceRegistry::default());
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let resource = CountingResource::blocked(Arc::clone(&started), Arc::clone(&release));
        registry
            .register(0, Arc::clone(&resource))
            .await
            .expect("register");

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
}
