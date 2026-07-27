//! 对应 Java 类：com.alibaba.druid.stat.JdbcSqlStat + SqlMerger
//!
//! SQL 合并统计：把参数化后的 SQL 模板作为 key，聚合执行统计。

use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// SQL 指纹（xxh3 哈希）。
pub fn fingerprint(sql_template: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    sql_template.hash(&mut hasher);
    hasher.finish()
}

/// 参数化 SQL 结果。
#[derive(Debug, Clone)]
pub struct ParameterizedSql {
    pub template: String,
    pub fingerprint: u64,
}

/// 简易 SQL 参数化：把字面量替换为 ?。
pub fn parameterize(sql: &str) -> ParameterizedSql {
    let mut template = String::with_capacity(sql.len());
    let mut in_string = false;
    let mut in_number = false;

    for ch in sql.chars() {
        match ch {
            '\'' if !in_string => {
                in_string = true;
                template.push('?');
            }
            '\'' if in_string => {
                in_string = false;
            }
            '0'..='9' if !in_string && !in_number => {
                in_number = true;
                template.push('?');
            }
            '0'..='9' if in_number => { /* skip digits */ }
            _ if in_number => {
                in_number = false;
                template.push(ch);
            }
            _ if in_string => { /* skip string content */ }
            _ => template.push(ch),
        }
    }

    let fp = fingerprint(&template);
    ParameterizedSql { template, fingerprint: fp }
}

/// 单条 SQL 的合并统计。
///
/// 对应 Druid Java 的 `JdbcSqlStat`，按 SQL 模板聚合。
#[derive(Debug)]
pub struct MergedSqlStat {
    pub sql: String,
    pub fingerprint: u64,
    pub execute_count: AtomicU64,
    pub total_time_ns: AtomicU64,
    pub max_time_ns: AtomicU64,
    pub error_count: AtomicU64,
    pub fetch_row_count: AtomicU64,
    pub running_count: AtomicU64,
    pub concurrent_max: AtomicU64,
}

impl MergedSqlStat {
    pub fn new(sql: String, fingerprint: u64) -> Self {
        Self {
            sql, fingerprint,
            execute_count: AtomicU64::new(0),
            total_time_ns: AtomicU64::new(0),
            max_time_ns: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            fetch_row_count: AtomicU64::new(0),
            running_count: AtomicU64::new(0),
            concurrent_max: AtomicU64::new(0),
        }
    }

    /// 记录一次执行。
    pub fn record(&self, elapsed: Duration, ok: bool) {
        let nanos = elapsed.as_nanos() as u64;
        self.execute_count.fetch_add(1, Ordering::Relaxed);
        self.total_time_ns.fetch_add(nanos, Ordering::Relaxed);

        // 原子 max 更新（fetch_max 无分支，消除 CAS match 分支）
        self.max_time_ns.fetch_max(nanos, Ordering::Relaxed);

        if !ok {
            self.error_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn execute_count(&self) -> u64 { self.execute_count.load(Ordering::Relaxed) }
    pub fn total_time_ms(&self) -> f64 { self.total_time_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0 }
    pub fn max_time_ms(&self) -> f64 { self.max_time_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0 }
    pub fn error_count(&self) -> u64 { self.error_count.load(Ordering::Relaxed) }
}

/// SQL 合并器。
///
/// 对应 Druid Java 的 `DruidStatService` 中的 SQL 合并逻辑。
pub struct SqlMerger {
    cache: dashmap::DashMap<u64, std::sync::Arc<MergedSqlStat>>,
}

impl SqlMerger {
    pub fn new() -> Self {
        Self { cache: dashmap::DashMap::new() }
    }

    /// 记录一条 SQL 执行。
    pub fn record(&self, sql: &str, elapsed: Duration, ok: bool) {
        let param = parameterize(sql);
        let stat = self.cache
            .entry(param.fingerprint)
            .or_insert_with(|| std::sync::Arc::new(MergedSqlStat::new(param.template, param.fingerprint)))
            .clone();
        stat.record(elapsed, ok);
    }

    /// 获取所有 SQL 统计。
    pub fn all_stats(&self) -> Vec<std::sync::Arc<MergedSqlStat>> {
        self.cache.iter().map(|entry| entry.value().clone()).collect()
    }

    /// 获取指定指纹的统计。
    pub fn get_stat(&self, fingerprint: u64) -> Option<std::sync::Arc<MergedSqlStat>> {
        self.cache.get(&fingerprint).map(|v| v.clone())
    }

    /// SQL 模板数量。
    pub fn len(&self) -> usize { self.cache.len() }
    pub fn is_empty(&self) -> bool { self.cache.is_empty() }
}

impl Default for SqlMerger {
    fn default() -> Self { Self::new() }
}
