use super::{WallFunctionStatValue, WallSqlFunctionStat};
use std::sync::atomic::{AtomicU64, Ordering};

/// Wall 函数全局聚合统计。
///
/// 对应 Java: `com.alibaba.druid.wall.WallFunctionStat`。
#[derive(Debug, Default)]
pub struct WallFunctionStat {
    invoke_count: AtomicU64,
}

impl WallFunctionStat {
    /// 记录一次调用。
    pub fn increment_invoke_count(&self) {
        self.invoke_count.fetch_add(1, Ordering::AcqRel);
    }

    /// 合并单 SQL 统计。
    pub fn add_sql_function_stat(&self, stat: WallSqlFunctionStat) {
        self.invoke_count
            .fetch_add(stat.invoke_count, Ordering::AcqRel);
    }

    /// 返回快照；reset 时原子取走计数。
    #[must_use]
    pub fn stat_value(&self, name: String, reset: bool) -> WallFunctionStatValue {
        let invoke_count = if reset {
            self.invoke_count.swap(0, Ordering::AcqRel)
        } else {
            self.invoke_count.load(Ordering::Acquire)
        };
        WallFunctionStatValue { name, invoke_count }
    }
}
