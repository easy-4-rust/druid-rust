use super::{WallSqlTableStat, WallTableStatValue};
use std::array;
use std::sync::atomic::{AtomicU64, Ordering};

/// Wall 表全局聚合统计。
///
/// 对应 Java: `com.alibaba.druid.wall.WallTableStat`。操作次数、影响行数以及
/// Java 六档直方图都使用原子字段，允许 Filter 并发更新。
#[derive(Debug)]
pub struct WallTableStat {
    select_count: AtomicU64,
    select_into_count: AtomicU64,
    insert_count: AtomicU64,
    update_count: AtomicU64,
    delete_count: AtomicU64,
    truncate_count: AtomicU64,
    create_count: AtomicU64,
    alter_count: AtomicU64,
    drop_count: AtomicU64,
    replace_count: AtomicU64,
    delete_data_count: AtomicU64,
    update_data_count: AtomicU64,
    insert_data_count: AtomicU64,
    fetch_row_count: AtomicU64,
    fetch_row_histogram: [AtomicU64; 6],
    update_data_histogram: [AtomicU64; 6],
    delete_data_histogram: [AtomicU64; 6],
}

impl Default for WallTableStat {
    fn default() -> Self {
        Self {
            select_count: AtomicU64::new(0),
            select_into_count: AtomicU64::new(0),
            insert_count: AtomicU64::new(0),
            update_count: AtomicU64::new(0),
            delete_count: AtomicU64::new(0),
            truncate_count: AtomicU64::new(0),
            create_count: AtomicU64::new(0),
            alter_count: AtomicU64::new(0),
            drop_count: AtomicU64::new(0),
            replace_count: AtomicU64::new(0),
            delete_data_count: AtomicU64::new(0),
            update_data_count: AtomicU64::new(0),
            insert_data_count: AtomicU64::new(0),
            fetch_row_count: AtomicU64::new(0),
            fetch_row_histogram: array::from_fn(|_| AtomicU64::new(0)),
            update_data_histogram: array::from_fn(|_| AtomicU64::new(0)),
            delete_data_histogram: array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

impl WallTableStat {
    /// 合并一次 SQL 解析得到的表操作。
    pub fn add_sql_table_stat(&self, stat: &WallSqlTableStat) {
        add(&self.select_count, stat.select_count);
        add(&self.select_into_count, stat.select_into_count);
        add(&self.insert_count, stat.insert_count);
        add(&self.update_count, stat.update_count);
        add(&self.delete_count, stat.delete_count);
        add(&self.truncate_count, stat.truncate_count);
        add(&self.create_count, stat.create_count);
        add(&self.alter_count, stat.alter_count);
        add(&self.drop_count, stat.drop_count);
        add(&self.replace_count, stat.replace_count);
    }

    /// 累加 SELECT 返回行数并进入 Java 六档直方图。
    pub fn add_fetch_row_count(&self, delta: u64) {
        add_with_histogram(&self.fetch_row_count, &self.fetch_row_histogram, delta);
    }

    /// 累加 UPDATE 影响行数。
    pub fn add_update_data_count(&self, delta: u64) {
        add_with_histogram(&self.update_data_count, &self.update_data_histogram, delta);
    }

    /// 累加 DELETE 影响行数。
    pub fn add_delete_data_count(&self, delta: u64) {
        add_with_histogram(&self.delete_data_count, &self.delete_data_histogram, delta);
    }

    /// 累加 INSERT 影响行数；Java 不为 INSERT 维护直方图。
    pub fn add_insert_data_count(&self, delta: u64) {
        add(&self.insert_data_count, delta);
    }

    /// 获取快照；reset 时逐字段原子取走。
    #[must_use]
    pub fn stat_value(&self, name: String, reset: bool) -> WallTableStatValue {
        let load = |value: &AtomicU64| {
            if reset {
                value.swap(0, Ordering::AcqRel)
            } else {
                value.load(Ordering::Acquire)
            }
        };
        let histogram = |values: &[AtomicU64; 6]| array::from_fn(|index| load(&values[index]));
        WallTableStatValue {
            name,
            select_count: load(&self.select_count),
            select_into_count: load(&self.select_into_count),
            insert_count: load(&self.insert_count),
            update_count: load(&self.update_count),
            delete_count: load(&self.delete_count),
            truncate_count: load(&self.truncate_count),
            create_count: load(&self.create_count),
            alter_count: load(&self.alter_count),
            drop_count: load(&self.drop_count),
            replace_count: load(&self.replace_count),
            delete_data_count: load(&self.delete_data_count),
            update_data_count: load(&self.update_data_count),
            insert_data_count: load(&self.insert_data_count),
            fetch_row_count: load(&self.fetch_row_count),
            fetch_row_histogram: histogram(&self.fetch_row_histogram),
            update_data_histogram: histogram(&self.update_data_histogram),
            delete_data_histogram: histogram(&self.delete_data_histogram),
        }
    }
}

fn add(value: &AtomicU64, delta: u64) {
    if delta > 0 {
        value.fetch_add(delta, Ordering::AcqRel);
    }
}

fn add_with_histogram(total: &AtomicU64, histogram: &[AtomicU64; 6], delta: u64) {
    total.fetch_add(delta, Ordering::AcqRel);
    let bucket = match delta {
        0 => 0,
        1..=9 => 1,
        10..=99 => 2,
        100..=999 => 3,
        1000..=9999 => 4,
        _ => 5,
    };
    histogram[bucket].fetch_add(1, Ordering::AcqRel);
}
