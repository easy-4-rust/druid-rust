use super::shared_result_set_borrow::SharedResultSetBorrow;
use crate::error::RdbcError;
use crate::result_set::ResultSet;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

/// 标准共享结果集的后端拥有槽位与异步等待队列。
/// 对应 Java: java.sql.ResultSet；原共享具体句柄的 Rust 互斥借用适配。
pub(crate) struct SharedResultSetState {
    slot: Mutex<SharedResultSetSlot>,
}

// 后端只能处于槽位或一个拥有式借用中；等待者不持有或复制驱动对象。
struct SharedResultSetSlot {
    backend: Option<Box<dyn ResultSet>>,
    next_waiter: u64,
    waiters: BTreeMap<u64, Arc<Waker>>,
}

impl SharedResultSetState {
    pub(crate) fn new(backend: Box<dyn ResultSet>) -> Self {
        Self {
            slot: Mutex::new(SharedResultSetSlot {
                backend: Some(backend),
                next_waiter: 0,
                waiters: BTreeMap::new(),
            }),
        }
    }

    pub(crate) fn try_borrow(self: &Arc<Self>) -> Result<SharedResultSetBorrow, RdbcError> {
        let mut slot = self.slot.lock().map_err(|_| Self::poisoned())?;
        // 已排队的异步关闭优先，同步 getter 不能不断抢占导致其饥饿。
        if !slot.waiters.is_empty() {
            return Err(Self::borrowed());
        }
        let backend = slot.backend.take().ok_or_else(Self::borrowed)?;
        drop(slot);
        Ok(SharedResultSetBorrow::new(Arc::clone(self), backend))
    }

    pub(crate) fn poll_borrow(
        self: &Arc<Self>,
        waiter: &mut Option<u64>,
        context: &mut Context<'_>,
    ) -> Poll<Result<SharedResultSetBorrow, RdbcError>> {
        // 外部 Waker 的 clone/drop/wake 发生在状态锁之外。
        let waker = Arc::new(context.waker().clone());
        let mut slot = match self.slot.lock() {
            Ok(slot) => slot,
            Err(_) => return Poll::Ready(Err(Self::poisoned())),
        };
        let first = slot.waiters.keys().next().copied();
        let eligible = match *waiter {
            Some(id) => first == Some(id),
            None => first.is_none(),
        };
        if eligible && slot.backend.is_some() {
            let backend = slot
                .backend
                .take()
                .expect("available backend checked under lock");
            let old_waker = waiter.take().and_then(|id| slot.waiters.remove(&id));
            drop(slot);
            drop(old_waker);
            return Poll::Ready(Ok(SharedResultSetBorrow::new(Arc::clone(self), backend)));
        }
        let id = if let Some(id) = *waiter {
            id
        } else {
            let Some(next) = slot.next_waiter.checked_add(1) else {
                return Poll::Ready(Err(RdbcError::Other(
                    "shared result set waiter identity exhausted".to_owned(),
                )));
            };
            let id = slot.next_waiter;
            slot.next_waiter = next;
            *waiter = Some(id);
            id
        };
        let old_waker = slot.waiters.insert(id, waker);
        drop(slot);
        drop(old_waker);
        Poll::Pending
    }

    pub(crate) fn restore(&self, backend: Box<dyn ResultSet>) {
        let mut slot = self
            .slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        slot.backend = Some(backend);
        let wakers = slot.waiters.values().cloned().collect::<Vec<_>>();
        drop(slot);
        // 先归还后端再唤醒，避免执行器立即重新 poll 时丢失可用状态。
        for waker in wakers {
            waker.wake_by_ref();
        }
    }

    pub(crate) fn cancel_waiter(&self, waiter: u64) {
        let mut slot = self
            .slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let cancelled = slot.waiters.remove(&waiter);
        let next = if slot.backend.is_some() {
            slot.waiters.values().next().cloned()
        } else {
            None
        };
        drop(slot);
        drop(cancelled);
        if let Some(waker) = next {
            waker.wake_by_ref();
        }
    }

    fn borrowed() -> RdbcError {
        RdbcError::Other("shared result set is borrowed".to_owned())
    }

    fn poisoned() -> RdbcError {
        RdbcError::Other("shared result set state is poisoned".to_owned())
    }
}
