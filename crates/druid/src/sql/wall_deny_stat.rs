use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// Wall 拒绝计数。
///
/// 对应 Java：`com.alibaba.druid.wall.WallDenyStat`。
#[derive(Debug, Default)]
pub struct WallDenyStat {
    deny_count: AtomicU64,
    last_deny_time_millis: AtomicI64,
    reset_count: AtomicU64,
}

impl WallDenyStat {
    /// 增加拒绝次数并记录当前 Unix 毫秒。
    pub fn increment_and_get_deny_count(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| {
                i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
            });
        self.last_deny_time_millis.store(now, Ordering::Release);
        self.deny_count.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// 返回拒绝次数。
    #[must_use]
    pub fn deny_count(&self) -> u64 {
        self.deny_count.load(Ordering::Acquire)
    }

    /// 返回最后拒绝时间的 Unix 毫秒；尚未拒绝时为 `None`。
    #[must_use]
    pub fn last_deny_time_millis(&self) -> Option<i64> {
        let value = self.last_deny_time_millis.load(Ordering::Acquire);
        (value > 0).then_some(value)
    }

    /// 清空拒绝状态并增加 reset 计数。
    pub fn reset(&self) {
        self.last_deny_time_millis.store(0, Ordering::Release);
        self.deny_count.store(0, Ordering::Release);
        self.reset_count.fetch_add(1, Ordering::AcqRel);
    }

    /// 返回 reset 次数。
    #[must_use]
    pub fn reset_count(&self) -> u64 {
        self.reset_count.load(Ordering::Acquire)
    }
}
