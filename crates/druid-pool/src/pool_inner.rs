//! 对应 Java 类：com.alibaba.druid.pool.DruidDataSource（内部状态）
//!
//! 连接池内部状态：空闲队列、活跃计数、等待通知。

use druid_core::{Connection, ConnectionFactory, DruidError, FilterChain};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Notify;

/// 空闲连接条目。
pub(crate) struct IdleConn {
    pub conn: Box<dyn Connection>,
    pub id: u64,
    pub created_at: Instant,
    pub last_used: Instant,
}

/// 连接池内部状态。
pub struct PoolInner {
    pub factory: Arc<dyn ConnectionFactory>,
    pub config: crate::config::PoolInnerConfig,
    pub idle: parking_lot::Mutex<VecDeque<IdleConn>>,
    pub notify: Notify,
    pub active_count: AtomicUsize,
    pub total_count: AtomicUsize,
    pub next_id: AtomicU64,
    pub closed: AtomicBool,
    // 统计
    pub create_count: AtomicU64,
    pub close_count: AtomicU64,
    pub connect_count: AtomicU64,
    pub connect_error_count: AtomicU64,
    pub recycle_count: AtomicU64,
}

impl PoolInner {
    pub fn new(factory: Arc<dyn ConnectionFactory>, config: crate::config::PoolInnerConfig) -> Self {
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
        self.total_count.load(Ordering::Relaxed) < self.config.max_open
    }

    pub fn should_evict(&self) -> bool {
        let idle_count = self.idle.lock().len();
        idle_count > self.config.min_idle
    }

    /// 创建新连接。
    pub async fn create_connection(&self) -> Result<Box<dyn Connection>, DruidError> {
        self.total_count.fetch_add(1, Ordering::Relaxed);
        match self.factory.create().await {
            Ok(conn) => {
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
    pub fn return_connection(&self, conn: Box<dyn Connection>, id: u64) {
        if self.closed.load(Ordering::Relaxed) {
            self.destroy_connection(conn);
            return;
        }
        self.active_count.fetch_sub(1, Ordering::Relaxed);
        self.recycle_count.fetch_add(1, Ordering::Relaxed);

        let idle_count = self.idle.lock().len();
        if idle_count >= self.config.max_idle {
            // 超过 max_idle，销毁
            self.destroy_connection(conn);
        } else {
            let mut queue = self.idle.lock();
            queue.push_back(IdleConn {
                conn,
                id,
                created_at: Instant::now(),
                last_used: Instant::now(),
            });
            drop(queue);
            self.notify.notify_one();
        }
    }

    /// 销毁连接。
    pub fn destroy_connection(&self, mut conn: Box<dyn Connection>) {
        self.total_count.fetch_sub(1, Ordering::Relaxed);
        self.close_count.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(async move {
            let _ = conn.close().await;
        });
    }

    /// 关闭池。
    pub async fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        let idle: Vec<IdleConn> = {
            let mut queue = self.idle.lock();
            queue.drain(..).collect()
        };
        for item in idle {
            self.destroy_connection(item.conn);
        }
    }
}
