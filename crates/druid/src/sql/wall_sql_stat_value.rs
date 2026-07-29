use serde::Serialize;

/// 单条 Wall SQL 的不可变统计快照。
///
/// 对应 Java：`com.alibaba.druid.wall.WallSqlStatValue`。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WallSqlStatValue {
    pub sql: String,
    pub sql_hash: u64,
    pub sql_sample: String,
    pub sql_sample_hash: u64,
    pub execute_count: u64,
    pub execute_error_count: u64,
    pub fetch_row_count: u64,
    pub update_count: u64,
    pub syntax_error: bool,
    pub violation_message: Option<String>,
}

impl WallSqlStatValue {
    /// 生成与 Java `toMap()` 相同的稀疏管理字段。
    #[must_use]
    pub fn to_map(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut map = serde_json::Map::new();
        map.insert("sql".to_owned(), self.sql.clone().into());
        if self.sql != self.sql_sample {
            map.insert("sample".to_owned(), self.sql_sample.clone().into());
        }
        map.insert("executeCount".to_owned(), self.execute_count.into());
        if self.execute_error_count > 0 {
            map.insert(
                "executeErrorCount".to_owned(),
                self.execute_error_count.into(),
            );
        }
        if self.fetch_row_count > 0 {
            map.insert("fetchRowCount".to_owned(), self.fetch_row_count.into());
        }
        if self.update_count > 0 {
            map.insert("updateCount".to_owned(), self.update_count.into());
        }
        if let Some(message) = &self.violation_message {
            map.insert("violationMessage".to_owned(), message.clone().into());
        }
        map
    }
}
