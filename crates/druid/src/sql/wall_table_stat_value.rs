use serde_json::{Map, Value};

/// Wall 表聚合统计快照。
///
/// 对应 Java: `com.alibaba.druid.wall.WallTableStatValue`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallTableStatValue {
    pub name: String,
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
    pub delete_data_count: u64,
    pub update_data_count: u64,
    pub insert_data_count: u64,
    pub fetch_row_count: u64,
    pub fetch_row_histogram: [u64; 6],
    pub update_data_histogram: [u64; 6],
    pub delete_data_histogram: [u64; 6],
}

impl WallTableStatValue {
    /// 返回 Java 管理页面使用的字段映射。
    #[must_use]
    pub fn to_map(&self) -> Map<String, Value> {
        let mut map = Map::new();
        map.insert("name".to_owned(), self.name.clone().into());
        map.insert("selectCount".to_owned(), self.select_count.into());
        map.insert("selectIntoCount".to_owned(), self.select_into_count.into());
        map.insert("insertCount".to_owned(), self.insert_count.into());
        map.insert("updateCount".to_owned(), self.update_count.into());
        map.insert("deleteCount".to_owned(), self.delete_count.into());
        map.insert("truncateCount".to_owned(), self.truncate_count.into());
        map.insert("createCount".to_owned(), self.create_count.into());
        map.insert("alterCount".to_owned(), self.alter_count.into());
        map.insert("dropCount".to_owned(), self.drop_count.into());
        map.insert("replaceCount".to_owned(), self.replace_count.into());
        map.insert("deleteDataCount".to_owned(), self.delete_data_count.into());
        map.insert("updateDataCount".to_owned(), self.update_data_count.into());
        map.insert("insertDataCount".to_owned(), self.insert_data_count.into());
        map.insert("fetchRowCount".to_owned(), self.fetch_row_count.into());
        map.insert(
            "fetchRowHistogram".to_owned(),
            self.fetch_row_histogram.into(),
        );
        map.insert(
            "updateDataHistogram".to_owned(),
            self.update_data_histogram.into(),
        );
        map.insert(
            "deleteDataHistogram".to_owned(),
            self.delete_data_histogram.into(),
        );
        map
    }
}
