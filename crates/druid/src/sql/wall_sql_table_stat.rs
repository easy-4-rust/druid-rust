/// 单条 SQL 对某张表的操作计数。
///
/// 对应 Java: `com.alibaba.druid.wall.WallSqlTableStat`。该对象是解析结果，
/// 不使用原子字段；共享聚合由 `WallTableStat` 承担。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WallSqlTableStat {
    pub select_count: u64,
    pub select_into_count: u64,
    pub insert_count: u64,
    pub update_count: u64,
    pub delete_count: u64,
    pub truncate_count: u64,
    pub create_count: u64,
    pub alter_count: u64,
    pub drop_count: u64,
    pub replace_count: u64,
    pub show_count: u64,
    pub sample: Option<String>,
}

impl WallSqlTableStat {
    /// 记录一次 SELECT。
    pub fn increment_select_count(&mut self) {
        self.select_count += 1;
    }

    /// 记录一次 INSERT。
    pub fn increment_insert_count(&mut self) {
        self.insert_count += 1;
    }

    /// 记录一次 UPDATE。
    pub fn increment_update_count(&mut self) {
        self.update_count += 1;
    }

    /// 记录一次 DELETE。
    pub fn increment_delete_count(&mut self) {
        self.delete_count += 1;
    }
}
