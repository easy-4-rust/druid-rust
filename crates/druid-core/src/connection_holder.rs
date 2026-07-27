//! 对应 Java 类：com.alibaba.druid.pool.DruidConnectionHolder

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::time::Instant;

/// 连接状态枚举。
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState { Idle = 0, Active = 1, Validating = 2, Closing = 3, Closed = 4, Error = 5 }

/// 连接持有者，跟踪连接的生命周期状态。
pub struct ConnectionHolder {
    pub id: u64,
    pub created_at: Instant,
    last_used: std::sync::Mutex<Instant>,
    pub use_count: AtomicU64,
    state: AtomicU8,
    pub last_fingerprint: std::sync::Mutex<Option<u64>>,
}

impl ConnectionHolder {
    pub fn new(id: u64) -> Self {
        let now = Instant::now();
        Self { id, created_at: now, last_used: std::sync::Mutex::new(now), use_count: AtomicU64::new(0), state: AtomicU8::new(ConnectionState::Idle as u8), last_fingerprint: std::sync::Mutex::new(None) }
    }

    pub fn state(&self) -> ConnectionState {
        match self.state.load(Ordering::Acquire) { 0 => ConnectionState::Idle, 1 => ConnectionState::Active, 2 => ConnectionState::Validating, 3 => ConnectionState::Closing, 4 => ConnectionState::Closed, _ => ConnectionState::Error }
    }

    pub fn try_transition(&self, from: ConnectionState, to: ConnectionState) -> bool {
        self.state.compare_exchange(from as u8, to as u8, Ordering::AcqRel, Ordering::Relaxed).is_ok()
    }

    pub fn mark_active(&self) -> bool {
        if self.try_transition(ConnectionState::Idle, ConnectionState::Active) {
            self.use_count.fetch_add(1, Ordering::Relaxed);
            *self.last_used.lock().unwrap() = Instant::now();
            true
        } else { false }
    }

    pub fn mark_idle(&self) -> bool {
        *self.last_used.lock().unwrap() = Instant::now();
        self.try_transition(ConnectionState::Active, ConnectionState::Idle)
    }

    pub fn is_alive(&self, idle_timeout: std::time::Duration) -> bool {
        let s = self.state();
        if s == ConnectionState::Closed || s == ConnectionState::Error { return false; }
        self.last_used.lock().unwrap().elapsed() < idle_timeout
    }

    pub fn held_duration(&self) -> std::time::Duration {
        self.last_used.lock().unwrap().elapsed()
    }
}
