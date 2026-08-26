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

/// 保存 holder-owned async close commands，避免连接先于子 handle 回池。
pub(crate) struct StandardResourceRegistry {
    state: Mutex<StandardResourceRegistryState>,
    next_id: AtomicU64,
    changed: Arc<Notify>,
    shutting_down: Arc<AtomicBool>,
    shutdown: Arc<Notify>,
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
                coordinator_running,
            } if resources.is_empty() && *pending == 0 && !*coordinator_running => Ok(*generation),
            StandardResourceRegistryState::Closed {
                generation,
                resources,
                close_error,
                coordinator_running,
            } if resources.is_empty() && close_error.is_none() && !*coordinator_running => {
                let generation = generation.saturating_add(1);
                *state = StandardResourceRegistryState::Open {
                    generation,
                    resources: BTreeMap::new(),
                    pending: 0,
                    coordinator_running: false,
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
        let resource: Arc<dyn StandardResourceHandle> = resource;
        self.register_batch(permit, vec![resource])
            .await
            .map(|_| ())
    }

    /// 在同一短临界区接管一批 actor；拒绝时先入 registry cleanup queue 再等待。
    pub(crate) async fn register_batch(
        self: &Arc<Self>,
        permit: &StandardResourceAdmission,
        handles: Vec<Arc<dyn StandardResourceHandle>>,
    ) -> Result<Vec<StandardResourceRegistration>, RdbcError> {
        if handles.is_empty() {
            return Ok(Vec::new());
        }
        let expected_generation = permit.generation();
        let mut receivers = Vec::new();
        let mut registrations = Vec::with_capacity(handles.len());
        let mut bind_failed = false;
        let mut spawn_generation = None;
        let accepted = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let valid_open = matches!(
                &*state,
                StandardResourceRegistryState::Open {
                    generation,
                    ..
                } if permit.belongs_to(self) && *generation == expected_generation
            );
            let (generation, resources, coordinator_running) = match &mut *state {
                StandardResourceRegistryState::Open {
                    generation,
                    resources,
                    coordinator_running,
                    ..
                }
                | StandardResourceRegistryState::Closing {
                    generation,
                    resources,
                    coordinator_running,
                    ..
                }
                | StandardResourceRegistryState::Closed {
                    generation,
                    resources,
                    coordinator_running,
                    ..
                } => (*generation, resources, coordinator_running),
            };
            let registration_generation = if valid_open {
                expected_generation
            } else {
                generation
            };
            let cleanup_state = if valid_open {
                StandardResourceCleanupState::Open
            } else {
                StandardResourceCleanupState::Queued
            };
            let mut inserted_ids = Vec::with_capacity(handles.len());
            for resource in handles {
                let id = self.next_id.fetch_add(1, Ordering::AcqRel);
                let registration = StandardResourceRegistration::new(
                    Arc::downgrade(self),
                    registration_generation,
                    id,
                );
                if resource.bind_registration(registration.clone()) {
                    resources.insert(
                        id,
                        StandardResourceRegistryEntry::new(resource, cleanup_state),
                    );
                    inserted_ids.push(id);
                    registrations.push(registration);
                } else {
                    bind_failed = true;
                }
            }
            if valid_open && !bind_failed {
                true
            } else {
                for id in inserted_ids {
                    if let Some(entry) = resources.get_mut(&id) {
                        entry.cleanup_state = StandardResourceCleanupState::Queued;
                        let (sender, receiver) = oneshot::channel();
                        entry.cleanup_waiters.push(sender);
                        receivers.push(receiver);
                    }
                }
                if !receivers.is_empty() && !*coordinator_running {
                    *coordinator_running = true;
                    spawn_generation = Some(generation);
                }
                false
            }
        };
        if accepted {
            return Ok(registrations);
        }
        // 拒绝资源已进入 registry queue；现在释放 pending 不会丢失 raw ownership。
        permit.finish();
        if let Some(generation) = spawn_generation {
            self.spawn_coordinator(generation);
        }
        self.changed.notify_waiters();
        let cleanup_result = Self::await_cleanup(receivers).await;
        if bind_failed {
            return Err(RdbcError::Other(
                "standard resource already has a registry owner".to_owned(),
            ));
        }
        cleanup_result.and(Err(RdbcError::ConnectionDiscarded))
    }

    /// 把一组已注册 actor 转入 cleanup queue，随后结束 admission 并等待本轮结果。
    pub(crate) async fn close_registered_after_admission(
        self: &Arc<Self>,
        registrations: &[StandardResourceRegistration],
        admission: &StandardResourceAdmission,
    ) -> Result<(), RdbcError> {
        let mut receivers = Vec::new();
        let mut spawn_generation = None;
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let (generation, resources, coordinator_running) = match &mut *state {
                StandardResourceRegistryState::Open {
                    generation,
                    resources,
                    coordinator_running,
                    ..
                }
                | StandardResourceRegistryState::Closing {
                    generation,
                    resources,
                    coordinator_running,
                    ..
                }
                | StandardResourceRegistryState::Closed {
                    generation,
                    resources,
                    coordinator_running,
                    ..
                } => (*generation, resources, coordinator_running),
            };
            for registration in registrations {
                if registration.generation() != generation {
                    continue;
                }
                let Some(entry) = resources.get_mut(&registration.id()) else {
                    continue;
                };
                if matches!(
                    entry.cleanup_state,
                    StandardResourceCleanupState::Open | StandardResourceCleanupState::Retry
                ) {
                    entry.cleanup_state = StandardResourceCleanupState::Queued;
                }
                let (sender, receiver) = oneshot::channel();
                entry.cleanup_waiters.push(sender);
                receivers.push(receiver);
            }
            if !receivers.is_empty() && !*coordinator_running {
                *coordinator_running = true;
                spawn_generation = Some(generation);
            }
        }
        // waiters 与 actor 都已在 registry state 中；caller cancellation 从此不影响清理。
        admission.finish();
        if let Some(generation) = spawn_generation {
            self.spawn_coordinator(generation);
        }
        if !receivers.is_empty() {
            self.changed.notify_waiters();
        }
        Self::await_cleanup(receivers).await
    }

    pub(crate) fn has_open_resources(&self) -> bool {
        match &*self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            StandardResourceRegistryState::Open {
                resources,
                pending,
                coordinator_running,
                ..
            } => {
                *pending > 0
                    || *coordinator_running
                    || resources.values().any(|entry| {
                        !entry.resource.is_resource_closed()
                            || !matches!(entry.cleanup_state, StandardResourceCleanupState::Open)
                    })
            }
            StandardResourceRegistryState::Closing { .. } => true,
            StandardResourceRegistryState::Closed {
                resources,
                close_error,
                coordinator_running,
                ..
            } => !resources.is_empty() || close_error.is_some() || *coordinator_running,
        }
    }

    pub(crate) async fn close_all(self: &Arc<Self>) -> Result<(), RdbcError> {
        let mut waited_generation = None;
        loop {
            let notified = Arc::clone(&self.changed).notified_owned();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let mut spawn_generation = None;
            let immediate = {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match &mut *state {
                    StandardResourceRegistryState::Closing {
                        generation,
                        coordinator_running,
                        ..
                    } => {
                        waited_generation = Some(*generation);
                        if !*coordinator_running {
                            *coordinator_running = true;
                            spawn_generation = Some(*generation);
                        }
                        None
                    }
                    StandardResourceRegistryState::Closed {
                        generation,
                        close_error,
                        ..
                    } if waited_generation == Some(*generation) => {
                        Some(close_error.clone().map_or(Ok(()), Err))
                    }
                    StandardResourceRegistryState::Closed {
                        resources,
                        close_error,
                        coordinator_running,
                        ..
                    } if resources.is_empty() && close_error.is_none() && !*coordinator_running => {
                        Some(Ok(()))
                    }
                    StandardResourceRegistryState::Open {
                        generation,
                        resources,
                        pending,
                        coordinator_running,
                    } => {
                        let generation = *generation;
                        let pending = *pending;
                        for entry in resources.values_mut() {
                            if matches!(
                                entry.cleanup_state,
                                StandardResourceCleanupState::Open
                                    | StandardResourceCleanupState::Retry
                            ) {
                                entry.cleanup_state = StandardResourceCleanupState::Queued;
                            }
                        }
                        let resources = std::mem::take(resources);
                        let running = *coordinator_running;
                        *state = StandardResourceRegistryState::Closing {
                            generation,
                            resources,
                            pending,
                            close_error: None,
                            coordinator_running: true,
                        };
                        waited_generation = Some(generation);
                        if !running {
                            spawn_generation = Some(generation);
                        }
                        None
                    }
                    StandardResourceRegistryState::Closed {
                        generation,
                        resources,
                        coordinator_running,
                        ..
                    } => {
                        let generation = *generation;
                        for entry in resources.values_mut() {
                            if matches!(
                                entry.cleanup_state,
                                StandardResourceCleanupState::Open
                                    | StandardResourceCleanupState::Retry
                            ) {
                                entry.cleanup_state = StandardResourceCleanupState::Queued;
                            }
                        }
                        let resources = std::mem::take(resources);
                        let running = *coordinator_running;
                        *state = StandardResourceRegistryState::Closing {
                            generation,
                            resources,
                            pending: 0,
                            close_error: None,
                            coordinator_running: true,
                        };
                        waited_generation = Some(generation);
                        if !running {
                            spawn_generation = Some(generation);
                        }
                        None
                    }
                }
            };
            if let Some(generation) = spawn_generation {
                self.spawn_coordinator(generation);
            }
            if let Some(result) = immediate {
                return result;
            }
            notified.await;
        }
    }

    async fn await_cleanup(
        receivers: Vec<oneshot::Receiver<Result<(), RdbcError>>>,
    ) -> Result<(), RdbcError> {
        let mut first_error = None;
        for receiver in receivers {
            match receiver.await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    first_error.get_or_insert(error);
                }
                Err(_) => {
                    first_error.get_or_insert(RdbcError::ConnectionDiscarded);
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    fn spawn_coordinator(self: &Arc<Self>, generation: u64) {
        let registry = Arc::downgrade(self);
        tokio::spawn(async move {
            Self::run_coordinator(registry, generation).await;
        });
    }

    async fn run_coordinator(registry: Weak<Self>, generation: u64) {
        let Some(owner) = registry.upgrade() else {
            return;
        };
        let shutting_down = Arc::clone(&owner.shutting_down);
        let shutdown = Arc::clone(&owner.shutdown);
        drop(owner);
        let mut guard = StandardResourceCoordinatorGuard::new(registry.clone(), generation);
        loop {
            let shutdown_notified = Arc::clone(&shutdown).notified_owned();
            tokio::pin!(shutdown_notified);
            shutdown_notified.as_mut().enable();
            if shutting_down.load(Ordering::Acquire) {
                guard.complete();
                return;
            }
            let Some(owner) = registry.upgrade() else {
                guard.complete();
                return;
            };
            let notified = Arc::clone(&owner.changed).notified_owned();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let step = {
                let mut state = owner
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let pending_admissions = matches!(
                    &*state,
                    StandardResourceRegistryState::Closing { pending, .. } if *pending > 0
                );
                let (current_generation, resources, coordinator_running) = match &mut *state {
                    StandardResourceRegistryState::Open {
                        generation,
                        resources,
                        coordinator_running,
                        ..
                    }
                    | StandardResourceRegistryState::Closing {
                        generation,
                        resources,
                        coordinator_running,
                        ..
                    }
                    | StandardResourceRegistryState::Closed {
                        generation,
                        resources,
                        coordinator_running,
                        ..
                    } => (*generation, resources, coordinator_running),
                };
                if current_generation != generation || !*coordinator_running {
                    StandardResourceCoordinatorStep::Complete
                } else if pending_admissions {
                    StandardResourceCoordinatorStep::Wait
                } else if let Some((id, entry)) = resources.iter_mut().rev().find(|(_, entry)| {
                    matches!(entry.cleanup_state, StandardResourceCleanupState::Queued)
                }) {
                    let id = *id;
                    entry.cleanup_state = StandardResourceCleanupState::Closing;
                    StandardResourceCoordinatorStep::Work {
                        id,
                        resource: Arc::clone(&entry.resource),
                    }
                } else if resources.values().any(|entry| {
                    matches!(entry.cleanup_state, StandardResourceCleanupState::Closing)
                }) {
                    StandardResourceCoordinatorStep::Wait
                } else {
                    match &mut *state {
                        StandardResourceRegistryState::Closing { pending, .. } if *pending > 0 => {
                            StandardResourceCoordinatorStep::Wait
                        }
                        StandardResourceRegistryState::Closing { .. } => {
                            let previous = std::mem::take(&mut *state);
                            *state = match previous {
                                StandardResourceRegistryState::Closing {
                                    generation,
                                    resources,
                                    close_error,
                                    ..
                                } => StandardResourceRegistryState::Closed {
                                    generation,
                                    resources,
                                    close_error,
                                    coordinator_running: false,
                                },
                                other => other,
                            };
                            StandardResourceCoordinatorStep::Complete
                        }
                        StandardResourceRegistryState::Open {
                            coordinator_running,
                            ..
                        }
                        | StandardResourceRegistryState::Closed {
                            coordinator_running,
                            ..
                        } => {
                            *coordinator_running = false;
                            StandardResourceCoordinatorStep::Complete
                        }
                    }
                }
            };
            drop(owner);
            match step {
                StandardResourceCoordinatorStep::Work { id, resource } => {
                    let close = resource.close_resource();
                    tokio::pin!(close);
                    let result = tokio::select! {
                        biased;
                        _ = &mut shutdown_notified => {
                            guard.complete();
                            return;
                        }
                        result = &mut close => result,
                    };
                    let mut waiters = Vec::new();
                    if let Some(owner) = registry.upgrade() {
                        let mut state = owner
                            .state
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        let (current_generation, resources) = match &mut *state {
                            StandardResourceRegistryState::Open {
                                generation,
                                resources,
                                ..
                            }
                            | StandardResourceRegistryState::Closing {
                                generation,
                                resources,
                                ..
                            }
                            | StandardResourceRegistryState::Closed {
                                generation,
                                resources,
                                ..
                            } => (*generation, resources),
                        };
                        if current_generation == generation {
                            match &result {
                                Ok(()) => {
                                    if let Some(mut entry) = resources.remove(&id) {
                                        waiters.append(&mut entry.cleanup_waiters);
                                    }
                                }
                                Err(_) => {
                                    if let Some(entry) = resources.get_mut(&id) {
                                        entry.cleanup_state = StandardResourceCleanupState::Retry;
                                        waiters.append(&mut entry.cleanup_waiters);
                                    }
                                }
                            }
                            if let (
                                Err(error),
                                StandardResourceRegistryState::Closing { close_error, .. },
                            ) = (&result, &mut *state)
                            {
                                if close_error.is_none() {
                                    *close_error = Some(error.clone());
                                }
                            }
                        }
                        owner.changed.notify_waiters();
                    }
                    for waiter in waiters {
                        let _ = waiter.send(result.clone());
                    }
                }
                StandardResourceCoordinatorStep::Wait => {
                    tokio::select! {
                        biased;
                        _ = &mut shutdown_notified => {
                            guard.complete();
                            return;
                        }
                        _ = notified => {}
                    }
                }
                StandardResourceCoordinatorStep::Complete => {
                    if let Some(owner) = registry.upgrade() {
                        owner.changed.notify_waiters();
                    }
                    guard.complete();
                    return;
                }
            }
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
                let remove = resources.get(&id).is_some_and(|entry| {
                    matches!(
                        entry.cleanup_state,
                        StandardResourceCleanupState::Open | StandardResourceCleanupState::Retry
                    )
                });
                if remove {
                    resources.remove(&id);
                }
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

    pub(crate) fn cleanup_requested(&self, generation: u64, id: u64) -> bool {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &*state {
            StandardResourceRegistryState::Open {
                generation: current,
                resources,
                ..
            } if *current == generation => resources.get(&id).is_none_or(|entry| {
                !matches!(entry.cleanup_state, StandardResourceCleanupState::Open)
            }),
            StandardResourceRegistryState::Open { .. }
            | StandardResourceRegistryState::Closing { .. }
            | StandardResourceRegistryState::Closed { .. } => true,
        }
    }

    fn coordinator_stopped(&self, generation: u64) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &mut *state {
            StandardResourceRegistryState::Open {
                generation: current,
                resources,
                coordinator_running,
                ..
            }
            | StandardResourceRegistryState::Closing {
                generation: current,
                resources,
                coordinator_running,
                ..
            }
            | StandardResourceRegistryState::Closed {
                generation: current,
                resources,
                coordinator_running,
                ..
            } if *current == generation => {
                for entry in resources.values_mut() {
                    if matches!(entry.cleanup_state, StandardResourceCleanupState::Closing) {
                        entry.cleanup_state = StandardResourceCleanupState::Queued;
                    }
                }
                *coordinator_running = false;
            }
            StandardResourceRegistryState::Open { .. }
            | StandardResourceRegistryState::Closing { .. }
            | StandardResourceRegistryState::Closed { .. } => {}
        }
        self.changed.notify_waiters();
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
