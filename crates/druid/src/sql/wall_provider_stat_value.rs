//! 对应 Java：`com.alibaba.druid.wall.WallProviderStatValue`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/wall/WallProviderStatValue.java`。

use super::{WallFunctionStatValue, WallSqlStatValue, WallTableStatValue};
use serde_json::{Map, Value};

/// Wall provider 的不可变管理快照。
///
/// 包含 provider 累计计数，以及当前有执行数据的表、函数和白/黑名单 SQL。
/// Java 的监控注解属于 JVM 持久化宿主，不迁移为 Rust 类型；字段和 `toMap`
/// 管理协议保持一致。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WallProviderStatValue {
    pub name: Option<String>,
    pub check_count: u64,
    pub hard_check_count: u64,
    pub violation_count: u64,
    pub white_list_hit_count: u64,
    pub black_list_hit_count: u64,
    pub syntax_error_count: u64,
    pub violation_effect_row_count: u64,
    pub tables: Vec<WallTableStatValue>,
    pub functions: Vec<WallFunctionStatValue>,
    pub white_list: Vec<WallSqlStatValue>,
    pub black_list: Vec<WallSqlStatValue>,
}

impl WallProviderStatValue {
    /// 转换为 Java Druid 管理接口使用的字段映射。
    #[must_use]
    pub fn to_map(&self) -> Map<String, Value> {
        let mut info = Map::new();
        info.insert("checkCount".to_owned(), self.check_count.into());
        info.insert("hardCheckCount".to_owned(), self.hard_check_count.into());
        info.insert("violationCount".to_owned(), self.violation_count.into());
        info.insert(
            "violationEffectRowCount".to_owned(),
            self.violation_effect_row_count.into(),
        );
        info.insert(
            "blackListHitCount".to_owned(),
            self.black_list_hit_count.into(),
        );
        info.insert(
            "blackListSize".to_owned(),
            u64::try_from(self.black_list.len())
                .unwrap_or(u64::MAX)
                .into(),
        );
        info.insert(
            "whiteListHitCount".to_owned(),
            self.white_list_hit_count.into(),
        );
        info.insert(
            "whiteListSize".to_owned(),
            u64::try_from(self.white_list.len())
                .unwrap_or(u64::MAX)
                .into(),
        );
        info.insert(
            "syntaxErrorCount".to_owned(),
            self.syntax_error_count.into(),
        );
        info.insert(
            "tables".to_owned(),
            self.tables
                .iter()
                .map(|value| Value::Object(value.to_map()))
                .collect::<Vec<_>>()
                .into(),
        );
        info.insert(
            "functions".to_owned(),
            self.functions
                .iter()
                .map(|value| Value::Object(value.to_map()))
                .collect::<Vec<_>>()
                .into(),
        );
        info.insert(
            "blackList".to_owned(),
            self.black_list
                .iter()
                .map(|value| Value::Object(value.to_map()))
                .collect::<Vec<_>>()
                .into(),
        );
        info.insert(
            "whiteList".to_owned(),
            self.white_list
                .iter()
                .map(|value| Value::Object(value.to_map()))
                .collect::<Vec<_>>()
                .into(),
        );
        info
    }
}
