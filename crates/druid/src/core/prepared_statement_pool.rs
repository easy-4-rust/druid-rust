//! 单物理连接 `PreparedStatement` 缓存。
//!
//! 对应 Java：`com.alibaba.druid.pool.PreparedStatementPool`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/pool/PreparedStatementPool.java`。

use super::{PreparedStatementCacheStats, PreparedStatementHolder, PreparedStatementKey};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// 单物理连接的 access-order LRU `PreparedStatement` 缓存。
pub struct PreparedStatementPool {
    map: HashMap<PreparedStatementKey, Arc<PreparedStatementHolder>>,
    access_order: VecDeque<PreparedStatementKey>,
    max_size: usize,
    share_prepared_statements: bool,
    use_oracle_implicit_cache: bool,
    stats: Arc<PreparedStatementCacheStats>,
}

#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for PreparedStatementPool {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedStatementPool")
            .field("size", &self.size())
            .field("max_size", &self.max_size)
            .field("share_prepared_statements", &self.share_prepared_statements)
            .field("use_oracle_implicit_cache", &self.use_oracle_implicit_cache)
            .finish()
    }
}

impl PreparedStatementPool {
    /// 创建 `PreparedStatement` 缓存。
    ///
    /// Java 在配置容量小于等于零时只把 `HashMap` 初始容量改为 16，实际 LRU
    /// 上限仍使用配置值；Rust 无需暴露 `HashMap` 初始容量，但保留实际上限语义。
    pub fn new(
        max_size: usize,
        share_prepared_statements: bool,
        use_oracle_implicit_cache: bool,
        stats: Arc<PreparedStatementCacheStats>,
    ) -> Self {
        Self {
            map: HashMap::with_capacity(max_size.max(16)),
            access_order: VecDeque::with_capacity(max_size.max(16)),
            max_size,
            share_prepared_statements,
            use_oracle_implicit_cache,
            stats,
        }
    }

    /// 按缓存键获取语句。
    ///
    /// 命中会更新 access-order；非共享模式下语句正在使用时返回 `None`，但与
    /// Java 一致不增加 miss。真正命中时增加 holder hit 和数据源 hit；不存在
    /// 时增加 miss。
    pub fn get(&mut self, key: &PreparedStatementKey) -> Option<Arc<PreparedStatementHolder>> {
        let holder = self.map.get(key).cloned();
        if let Some(holder) = holder {
            self.touch(key);
            if holder.is_in_use() && !self.share_prepared_statements {
                return None;
            }
            holder.increment_hit_count();
            self.stats.record_hit();
            if holder.is_enter_oracle_implicit_cache() {
                holder.set_enter_oracle_implicit_cache(false);
            }
            Some(holder)
        } else {
            self.stats.record_miss();
            None
        }
    }

    /// 将语句放入缓存，并按 access-order LRU 淘汰超限条目。
    pub fn put(&mut self, holder: Arc<PreparedStatementHolder>) {
        if holder.statement().is_closed() {
            return;
        }

        holder.set_enter_oracle_implicit_cache(self.use_oracle_implicit_cache);
        let key = holder.key().clone();
        let old_holder = self.map.insert(key.clone(), holder.clone());
        self.touch(&key);

        if old_holder
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, &holder))
        {
            return;
        }

        if let Some(old_holder) = old_holder {
            old_holder.set_pooling(false);
            self.close_removed_statement(&old_holder);
        } else if holder.hit_count() == 0 {
            self.stats.record_cache_insert();
        }

        holder.set_pooling(true);
        while self.map.len() > self.max_size {
            let Some(eldest_key) = self.access_order.pop_front() else {
                break;
            };
            if let Some(eldest) = self.map.remove(&eldest_key) {
                eldest.set_pooling(false);
                self.close_removed_statement(&eldest);
            }
        }
    }

    /// 从缓存删除指定 holder。
    ///
    /// 与 Java 一致按 key 删除 map 中对象，但关闭参数传入的 holder。
    pub fn remove(&mut self, holder: &Arc<PreparedStatementHolder>) {
        self.map.remove(holder.key());
        self.remove_from_order(holder.key());
        holder.set_pooling(false);
        self.close_removed_statement(holder);
    }

    /// 清空缓存。
    ///
    /// 正在使用的语句只取消 pooling，不立即关闭；其 wrapper 后续关闭时可按
    /// Java 配置重新进入缓存。
    pub fn clear(&mut self) {
        let holders = self
            .map
            .drain()
            .map(|(_, holder)| holder)
            .collect::<Vec<_>>();
        self.access_order.clear();
        for holder in holders {
            holder.set_pooling(false);
            self.close_removed_statement(&holder);
        }
    }

    /// 返回缓存条目数。
    pub fn size(&self) -> usize {
        self.map.len()
    }

    /// 返回缓存是否为空。
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// 返回缓存中是否仍有被逻辑 wrapper 使用的语句。
    ///
    /// Java recycle 会关闭 statement trace 中的未关闭语句；Rust wrapper 不长期
    /// 借用连接，因此连接归还前用该状态触发保守 cache 失效。
    pub fn has_in_use_statement(&self) -> bool {
        self.map.values().any(|holder| holder.is_in_use())
    }

    /// 返回从最旧到最新的 access-order key，用于差分测试和管理面快照。
    pub fn keys_in_lru_order(&self) -> Vec<PreparedStatementKey> {
        self.access_order.iter().cloned().collect()
    }

    fn close_removed_statement(&self, holder: &Arc<PreparedStatementHolder>) {
        if holder.is_in_use() {
            return;
        }
        if holder.is_enter_oracle_implicit_cache() {
            holder.set_enter_oracle_implicit_cache(false);
        }
        let _ = holder.statement().close();
        self.stats.record_cache_delete();
    }

    fn touch(&mut self, key: &PreparedStatementKey) {
        self.remove_from_order(key);
        self.access_order.push_back(key.clone());
    }

    fn remove_from_order(&mut self, key: &PreparedStatementKey) {
        if let Some(index) = self
            .access_order
            .iter()
            .position(|candidate| candidate == key)
        {
            self.access_order.remove(index);
        }
    }
}
