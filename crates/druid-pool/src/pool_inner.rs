//! 对应 Java 类：com.alibaba.druid.pool.DruidDataSource（内部状态）
//!
//! 连接池内部状态：空闲队列、活跃计数、等待通知。

use druid_core::{DruidError, PhysicalConnection, PhysicalConnectionFactory};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Notify;

/// 空闲连接条目。
pub(crate) struct IdleConn {
    pub conn: Box<dyn PhysicalConnection>,
    pub id: u64,
    pub created_at: Instant,
    pub last_used: Instant,
}

/// 连接池内部状态。
pub struct PoolInner {
    pub(crate) factory: Arc<dyn PhysicalConnectionFactory>,
    pub(crate) config: crate::config::PoolInnerConfig,
    pub(crate) idle: parking_lot::Mutex<VecDeque<IdleConn>>,
    pub(crate) notify: Notify,
    pub(crate) active_count: AtomicUsize,
    pub(crate) total_count: AtomicUsize,
    pub(crate) next_id: AtomicU64,
    pub(crate) closed: AtomicBool,
    // 统计
    pub(crate) create_count: AtomicU64,
    pub(crate) close_count: AtomicU64,
    pub(crate) connect_count: AtomicU64,
    pub(crate) connect_error_count: AtomicU64,
    pub(crate) recycle_count: AtomicU64,
}

impl PoolInner {
    pub fn new(
        factory: Arc<dyn PhysicalConnectionFactory>,
        config: crate::config::PoolInnerConfig,
    ) -> Self {
        Self {
            factory,
            config,
            idle: parking_lot::Mutex::new(VecDeque::new()),
            notify: Notify::new(),
            active_count: AtomicUsize::new(0),
            total_count: AtomicUsize::new(0),
            next_id: AtomicU64::new(1),
            closed: AtomicBool::new(false),
            create_count: AtomicU64::new(0),
            close_count: AtomicU64::new(0),
            connect_count: AtomicU64::new(0),
            connect_error_count: AtomicU64::new(0),
            recycle_count: AtomicU64::new(0),
        }
    }

    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn can_grow(&self) -> bool {
        self.total_count.load(Ordering::Acquire) < self.config.max_open
    }

    pub fn should_evict(&self) -> bool {
        let idle_count = self.idle.lock().len();
        idle_count > self.config.min_idle
    }

    /// 创建新连接。
    pub async fn create_connection(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        let reserved = self
            .total_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < self.config.max_open).then_some(current + 1)
            })
            .is_ok();
        if !reserved {
            return Err(DruidError::PoolExhausted);
        }

        match self.factory.create().await {
            Ok(mut conn) => {
                if self.closed.load(Ordering::Acquire) {
                    let _ = self.factory.close(&mut conn).await;
                    self.total_count.fetch_sub(1, Ordering::AcqRel);
                    self.close_count.fetch_add(1, Ordering::Relaxed);
                    return Err(DruidError::PoolClosed);
                }
                self.create_count.fetch_add(1, Ordering::Relaxed);
                Ok(conn)
            }
            Err(e) => {
                self.total_count.fetch_sub(1, Ordering::Relaxed);
                self.connect_error_count.fetch_add(1, Ordering::Relaxed);
                Err(e)
            }
        }
    }

    /// 归还连接到空闲队列。
    pub fn return_connection(
        &self,
        conn: Box<dyn PhysicalConnection>,
        id: u64,
        created_at: Instant,
    ) {
        let was_active = self
            .active_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current > 0).then_some(current - 1)
            })
            .is_ok();
        if !was_active {
            self.destroy_connection(conn);
            return;
        }

        self.recycle_count.fetch_add(1, Ordering::Relaxed);

        if self.closed.load(Ordering::Acquire) || conn.is_closed() {
            self.destroy_connection(conn);
            return;
        }

        let returned = {
            let mut queue = self.idle.lock();
            if queue.len() >= self.config.max_idle {
                Err(conn)
            } else {
                queue.push_back(IdleConn {
                    conn,
                    id,
                    created_at,
                    last_used: Instant::now(),
                });
                Ok(())
            }
        };

        if let Err(conn) = returned {
            self.destroy_connection(conn);
        } else {
            self.notify.notify_one();
        }
    }

    /// 销毁连接。
    pub fn destroy_connection(&self, mut conn: Box<dyn PhysicalConnection>) {
        let _ = self
            .total_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current > 0).then_some(current - 1)
            });
        self.close_count.fetch_add(1, Ordering::Relaxed);
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = conn.close().await;
            });
        }
    }

    /// 关闭池。
    pub async fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        let idle: Vec<IdleConn> = {
            let mut queue = self.idle.lock();
            queue.drain(..).collect()
        };
        for mut item in idle {
            let _ = self.factory.close(&mut item.conn).await;
            let _ = self
                .total_count
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    (current > 0).then_some(current - 1)
                });
            self.close_count.fetch_add(1, Ordering::Relaxed);
        }
        self.notify.notify_waiters();
    }
}
