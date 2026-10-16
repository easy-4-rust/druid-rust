//! Callable 执行代次与当前 Agent 请求 ID 的共享状态。

use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

struct SlotState {
    generation: u64,
    executing: bool,
    request_id: Option<u64>,
    closed: bool,
}

/// 仅向当前 Callable 执行代次暴露正在运行的请求 ID。
/// 对应 Java: java.sql.Statement#cancel 的活动执行身份。
pub(crate) struct JdbcAgentCancellationSlot {
    state: Mutex<SlotState>,
    changed: Notify,
}

impl JdbcAgentCancellationSlot {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(SlotState {
                generation: 0,
                executing: false,
                request_id: None,
                closed: false,
            }),
            changed: Notify::new(),
        })
    }

    pub(crate) fn control_generation(&self) -> Option<u64> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed {
            None
        } else if state.executing {
            Some(state.generation)
        } else {
            state.generation.checked_add(1)
        }
    }

    pub(crate) fn begin_execution(self: &Arc<Self>) -> JdbcAgentExecutionGuard {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.generation = state.generation.saturating_add(1);
        state.executing = true;
        state.request_id = None;
        drop(state);
        self.changed.notify_waiters();
        JdbcAgentExecutionGuard {
            slot: Arc::clone(self),
        }
    }

    pub(crate) fn begin_request(self: &Arc<Self>, request_id: u64) -> JdbcAgentRequestGuard {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.request_id = Some(request_id);
        drop(state);
        self.changed.notify_waiters();
        JdbcAgentRequestGuard {
            slot: Arc::clone(self),
            request_id,
        }
    }

    pub(crate) async fn target_request(&self, generation: u64) -> Option<u64> {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            {
                let state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if state.closed || !state.executing || state.generation != generation {
                    return None;
                }
                if let Some(request_id) = state.request_id {
                    return Some(request_id);
                }
            }
            changed.await;
        }
    }

    pub(crate) fn close(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.closed = true;
        state.executing = false;
        state.request_id = None;
        drop(state);
        self.changed.notify_waiters();
    }

    fn finish_request(&self, request_id: u64) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.request_id == Some(request_id) {
            state.request_id = None;
        }
        drop(state);
        self.changed.notify_waiters();
    }

    fn finish_execution(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.executing = false;
        state.request_id = None;
        drop(state);
        self.changed.notify_waiters();
    }
}

/// 执行 Future 离开时结束本次代次；异常丢弃时保留当前请求用于上层清理。
pub(crate) struct JdbcAgentExecutionGuard {
    slot: Arc<JdbcAgentCancellationSlot>,
}

impl Drop for JdbcAgentExecutionGuard {
    fn drop(&mut self) {
        self.slot.finish_execution();
    }
}

/// 请求 Future 被取消时也清理活动请求 ID，防止旧控制对象误读。
pub(crate) struct JdbcAgentRequestGuard {
    slot: Arc<JdbcAgentCancellationSlot>,
    request_id: u64,
}

impl Drop for JdbcAgentRequestGuard {
    fn drop(&mut self) {
        self.slot.finish_request(self.request_id);
    }
}
