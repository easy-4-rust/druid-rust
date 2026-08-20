//! Druid 事务可观测信息。
//!
//! 对应 Java：
//! `com.alibaba.druid.util.TransactionInfo` 与兼容子类
//! `com.alibaba.druid.proxy.rdbc.TransactionInfo`。

use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// 保存一次逻辑事务的身份、SQL 摘要和起止时间。
///
/// Java 调用方可直接取得可变 `List<String>`；Rust 使用短临界区保护同一可观察
/// 列表，并由池化连接限制最多记录十条 SQL。
#[derive(Debug)]
pub struct TransactionInfo {
    id: u64,
    sql_list: RwLock<Vec<String>>,
    start_time_millis: u64,
    end_time_millis: AtomicU64,
}

impl TransactionInfo {
    /// 创建指定数据源事务 ID 的信息对象。
    ///
    /// 对应 Java：`TransactionInfo#TransactionInfo(long)`。
    #[must_use]
    pub fn new(id: u64) -> Self {
        Self {
            id,
            sql_list: RwLock::new(Vec::with_capacity(4)),
            start_time_millis: now_millis(),
            end_time_millis: AtomicU64::new(0),
        }
    }

    /// 返回数据源分配的事务 ID。
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// 返回按执行顺序保存的 SQL 快照。
    #[must_use]
    pub fn sql_list(&self) -> Vec<String> {
        self.sql_list.read().clone()
    }

    /// 在尚未达到上限时记录一条 SQL。
    ///
    /// 对应 Java：`DruidPooledConnection#transactionRecord`。
    pub fn record_sql(&self, sql: &str, max_record_sql_count: usize) {
        let mut sql_list = self.sql_list.write();
        if sql_list.len() < max_record_sql_count {
            sql_list.push(sql.to_owned());
        }
    }

    /// 返回事务开始时间的 Unix epoch 毫秒值。
    #[must_use]
    pub const fn start_time_millis(&self) -> u64 {
        self.start_time_millis
    }

    /// 返回事务结束时间；尚未结束时为 0。
    #[must_use]
    pub fn end_time_millis(&self) -> u64 {
        self.end_time_millis.load(Ordering::Acquire)
    }

    /// 仅在首次结束时记录当前时间。
    ///
    /// 对应 Java：`TransactionInfo#setEndTimeMillis()`。
    pub fn set_end_time_millis_now(&self) {
        let _ = self.end_time_millis.compare_exchange(
            0,
            now_millis(),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    /// 显式覆盖结束时间。
    ///
    /// 对应 Java：`TransactionInfo#setEndTimeMillis(long)`。
    pub fn set_end_time_millis(&self, end_time_millis: u64) {
        self.end_time_millis
            .store(end_time_millis, Ordering::Release);
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
