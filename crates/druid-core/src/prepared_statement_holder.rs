//! 预编译语句持有者。
//!
//! 对应 Java：`com.alibaba.druid.pool.PreparedStatementHolder`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/pool/PreparedStatementHolder.java`。

use crate::{PhysicalPreparedStatement, PreparedStatementKey};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::Arc;

/// 保存缓存键、物理语句和复用状态。
pub struct PreparedStatementHolder {
    key: PreparedStatementKey,
    statement: Arc<dyn PhysicalPreparedStatement>,
    hit_count: AtomicU64,
    fetch_row_peak: AtomicI32,
    default_row_prefetch: AtomicI32,
    row_prefetch: AtomicI32,
    enter_oracle_implicit_cache: AtomicBool,
    in_use_count: AtomicU64,
    pooling: AtomicBool,
}

impl std::fmt::Debug for PreparedStatementHolder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedStatementHolder")
            .field("key", &self.key)
            .field("hit_count", &self.hit_count())
            .field("fetch_row_peak", &self.fetch_row_peak())
            .field("in_use_count", &self.in_use_count())
            .field("pooling", &self.is_pooling())
            .finish()
    }
}

impl PreparedStatementHolder {
    /// 创建语句持有者。
    ///
    /// 对应 Java：`PreparedStatementHolder(PreparedStatementKey, PreparedStatement)`。
    pub fn new(key: PreparedStatementKey, statement: Arc<dyn PhysicalPreparedStatement>) -> Self {
        Self {
            key,
            statement,
            hit_count: AtomicU64::new(0),
            fetch_row_peak: AtomicI32::new(-1),
            default_row_prefetch: AtomicI32::new(-1),
            row_prefetch: AtomicI32::new(-1),
            enter_oracle_implicit_cache: AtomicBool::new(false),
            in_use_count: AtomicU64::new(0),
            pooling: AtomicBool::new(false),
        }
    }

    /// 返回缓存键。
    pub fn key(&self) -> &PreparedStatementKey {
        &self.key
    }

    /// 返回物理语句。
    pub fn statement(&self) -> &Arc<dyn PhysicalPreparedStatement> {
        &self.statement
    }

    /// 返回是否进入 Oracle implicit cache。
    pub fn is_enter_oracle_implicit_cache(&self) -> bool {
        self.enter_oracle_implicit_cache.load(Ordering::Acquire)
    }

    /// 设置 Oracle implicit cache 状态。
    pub fn set_enter_oracle_implicit_cache(&self, value: bool) {
        self.enter_oracle_implicit_cache
            .store(value, Ordering::Release);
    }

    /// 返回默认行预取数。
    pub fn default_row_prefetch(&self) -> i32 {
        self.default_row_prefetch.load(Ordering::Relaxed)
    }

    /// 设置默认行预取数。
    pub fn set_default_row_prefetch(&self, value: i32) {
        self.default_row_prefetch.store(value, Ordering::Relaxed);
    }

    /// 返回当前行预取数。
    pub fn row_prefetch(&self) -> i32 {
        self.row_prefetch.load(Ordering::Relaxed)
    }

    /// 设置当前行预取数。
    pub fn set_row_prefetch(&self, value: i32) {
        self.row_prefetch.store(value, Ordering::Relaxed);
    }

    /// 返回历史最大 fetch 行数。
    pub fn fetch_row_peak(&self) -> i32 {
        self.fetch_row_peak.load(Ordering::Relaxed)
    }

    /// 仅在新值更大时更新 fetch 行数峰值。
    pub fn set_fetch_row_peak(&self, value: i32) {
        let _ = self
            .fetch_row_peak
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (value > current).then_some(value)
            });
    }

    /// 增加缓存命中次数。
    pub fn increment_hit_count(&self) {
        self.hit_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 返回缓存命中次数。
    pub fn hit_count(&self) -> u64 {
        self.hit_count.load(Ordering::Relaxed)
    }

    /// 返回语句是否正在被 pooled wrapper 使用。
    pub fn is_in_use(&self) -> bool {
        self.in_use_count() > 0
    }

    /// 增加使用计数。
    pub fn increment_in_use_count(&self) {
        self.in_use_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 减少使用计数；已为零时保持零。
    pub fn decrement_in_use_count(&self) {
        let _ = self
            .in_use_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                if current > 0 {
                    Some(current - 1)
                } else {
                    None
                }
            });
    }

    /// 返回当前使用计数。
    pub fn in_use_count(&self) -> u64 {
        self.in_use_count.load(Ordering::Relaxed)
    }

    /// 返回语句是否登记在缓存中。
    pub fn is_pooling(&self) -> bool {
        self.pooling.load(Ordering::Acquire)
    }

    /// 设置缓存登记状态。
    pub fn set_pooling(&self, value: bool) {
        self.pooling.store(value, Ordering::Release);
    }
}
