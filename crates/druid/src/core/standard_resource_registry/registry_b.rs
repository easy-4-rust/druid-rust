use super::*;

impl StandardResourceRegistry {
    pub(crate) async fn await_cleanup(
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

    pub(crate) fn spawn_coordinator(self: &Arc<Self>, generation: u64) {
        let registry = Arc::downgrade(self);
        if let Some(runtime) = &self.coordinator_runtime {
            runtime.spawn(async move {
                Self::run_coordinator(registry, generation).await;
            });
        } else {
            // 保留纯同步构造后首次进入 Tokio 的原路径。
            tokio::spawn(async move {
                Self::run_coordinator(registry, generation).await;
            });
        }
    }

    pub(crate) async fn run_coordinator(registry: Weak<Self>, generation: u64) {
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

    pub(crate) fn coordinator_stopped(&self, generation: u64) {
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
