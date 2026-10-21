use super::*;

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
        self.close_registered_inner(registrations, Some(admission))
            .await
    }

    /// 关闭已登记资源；参数为同一 registry 的注册令牌，返回该批清理结果。
    /// 父级 Closing 时仍可加入清理等待，不申请新的创建 admission。
    pub(crate) async fn close_registered(
        self: &Arc<Self>,
        registrations: &[StandardResourceRegistration],
    ) -> Result<(), RdbcError> {
        // 先校验整批归属，避免相同 generation/id 的跨连接令牌清理本地资源。
        if registrations
            .iter()
            .any(|registration| !registration.belongs_to(self))
        {
            return Err(RdbcError::ConnectionDiscarded);
        }
        self.close_registered_inner(registrations, None).await
    }

    pub(crate) async fn close_registered_inner(
        self: &Arc<Self>,
        registrations: &[StandardResourceRegistration],
        admission: Option<&StandardResourceAdmission>,
    ) -> Result<(), RdbcError> {
        let receivers = self.queue_registered_cleanup(registrations, admission);
        Self::await_cleanup(receivers).await
    }

    /// 同步移交已登记资源的关闭请求；参数为同一 registry 的令牌，无返回值。
    /// 不等待驱动或重试，具体结果继续由 registry 与父级 close/drain 保管。
    pub(crate) fn request_registered_cleanup(
        self: &Arc<Self>,
        registration: &StandardResourceRegistration,
    ) {
        if !registration.belongs_to(self) {
            return;
        }
        // 与显式 close 共用同一入队段；丢弃等待者不会取消已监管的 actor。
        drop(self.queue_registered_cleanup(std::slice::from_ref(registration), None));
    }

    pub(crate) fn queue_registered_cleanup(
        self: &Arc<Self>,
        registrations: &[StandardResourceRegistration],
        admission: Option<&StandardResourceAdmission>,
    ) -> Vec<oneshot::Receiver<Result<(), RdbcError>>> {
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
        if let Some(admission) = admission {
            admission.finish();
        }
        if let Some(generation) = spawn_generation {
            self.spawn_coordinator(generation);
        }
        if !receivers.is_empty() {
            self.changed.notify_waiters();
        }
        receivers
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
}
