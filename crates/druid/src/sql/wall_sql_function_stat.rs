/// 单条 SQL 内函数调用计数。
///
/// 对应 Java: `com.alibaba.druid.wall.WallSqlFunctionStat`。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WallSqlFunctionStat {
    pub invoke_count: u64,
}

impl WallSqlFunctionStat {
    /// 记录一次函数调用。
    pub fn increment_invoke_count(&mut self) {
        self.invoke_count += 1;
    }
}
