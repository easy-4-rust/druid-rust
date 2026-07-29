//! PreparedStatement 缓存统计。
//!
//! 对应 Java：
//! `com.alibaba.druid.pool.DruidAbstractDataSource` 中的 PreparedStatement
//! 计数字段和原子更新器。

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// 数据源级 PreparedStatement 统计。
#[derive(Debug, Default)]
pub struct PreparedStatementCacheStats {
    prepared_statement_count: AtomicU64,
    closed_prepared_statement_count: AtomicU64,
    cached_prepared_statement_count: AtomicI64,
    cached_prepared_statement_delete_count: AtomicU64,
    cached_prepared_statement_hit_count: AtomicU64,
    cached_prepared_statement_miss_count: AtomicU64,
}

impl PreparedStatementCacheStats {
    /// 重置累计计数；当前缓存数量保持不变。
    ///
    /// 对应 Java 数据源 `resetStat()`：不能把仍在缓存中的 statement 伪造为零。
    pub fn reset(&self) {
        self.prepared_statement_count.store(0, Ordering::Release);
        self.closed_prepared_statement_count
            .store(0, Ordering::Release);
        self.cached_prepared_statement_delete_count
            .store(0, Ordering::Release);
        self.cached_prepared_statement_hit_count
            .store(0, Ordering::Release);
        self.cached_prepared_statement_miss_count
            .store(0, Ordering::Release);
    }

    /// 记录创建一个新的物理 PreparedStatement。
    pub fn record_prepare(&self) {
        self.prepared_statement_count
            .fetch_add(1, Ordering::Relaxed);
    }

    /// 记录关闭一个物理 PreparedStatement。
    pub fn record_close(&self) {
        self.closed_prepared_statement_count
            .fetch_add(1, Ordering::Relaxed);
    }

    /// 记录一个从未命中过的新语句进入缓存。
    pub fn record_cache_insert(&self) {
        self.cached_prepared_statement_count
            .fetch_add(1, Ordering::Relaxed);
    }

    /// 记录缓存语句被实际关闭并删除。
    ///
    /// Java `closePreapredStatement` 无条件递减当前缓存数；因此首次创建后尚未
    /// 入缓存就执行失败的语句会得到 `-1`。这里使用有符号计数保留该可观察
    /// 兼容语义，而不是静默饱和到零。
    pub fn record_cache_delete(&self) {
        self.cached_prepared_statement_count
            .fetch_sub(1, Ordering::Relaxed);
        self.cached_prepared_statement_delete_count
            .fetch_add(1, Ordering::Relaxed);
        self.record_close();
    }

    /// 记录缓存命中。
    pub fn record_hit(&self) {
        self.cached_prepared_statement_hit_count
            .fetch_add(1, Ordering::Relaxed);
    }

    /// 记录缓存未命中。
    pub fn record_miss(&self) {
        self.cached_prepared_statement_miss_count
            .fetch_add(1, Ordering::Relaxed);
    }

    /// 返回物理 PreparedStatement 创建总数。
    pub fn prepared_statement_count(&self) -> u64 {
        self.prepared_statement_count.load(Ordering::Relaxed)
    }

    /// 返回物理 PreparedStatement 关闭总数。
    pub fn closed_prepared_statement_count(&self) -> u64 {
        self.closed_prepared_statement_count.load(Ordering::Relaxed)
    }

    /// 返回当前缓存的物理 PreparedStatement 数。
    pub fn cached_prepared_statement_count(&self) -> i64 {
        self.cached_prepared_statement_count.load(Ordering::Relaxed)
    }

    /// 返回从缓存删除并关闭的语句数。
    pub fn cached_prepared_statement_delete_count(&self) -> u64 {
        self.cached_prepared_statement_delete_count
            .load(Ordering::Relaxed)
    }

    /// 返回缓存命中次数。
    pub fn cached_prepared_statement_hit_count(&self) -> u64 {
        self.cached_prepared_statement_hit_count
            .load(Ordering::Relaxed)
    }

    /// 返回缓存未命中次数。
    pub fn cached_prepared_statement_miss_count(&self) -> u64 {
        self.cached_prepared_statement_miss_count
            .load(Ordering::Relaxed)
    }

    /// 返回缓存访问次数，即 hit + miss。
    pub fn cached_prepared_statement_access_count(&self) -> u64 {
        self.cached_prepared_statement_hit_count()
            .saturating_add(self.cached_prepared_statement_miss_count())
    }
}
