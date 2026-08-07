//! RDBC `getObject(..., Map<String, Class<?>>)` 类型映射。
//!
//! 对应 Java 平台对象：`java.util.Map<String, Class<?>>`。

use super::RdbcTargetType;
use std::collections::HashMap;

/// RDBC 用户定义类型名称到目标类的映射。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RdbcTypeMap {
    mappings: HashMap<String, RdbcTargetType>,
}

impl RdbcTypeMap {
    /// 创建空类型映射。
    pub fn new() -> Self {
        Self::default()
    }

    /// 从已有映射创建对象。
    pub fn from_mappings(mappings: HashMap<String, RdbcTargetType>) -> Self {
        Self { mappings }
    }

    /// 插入或替换一个 SQL 类型映射。
    ///
    /// # 参数
    /// - `sql_type_name`：Java Map 的 SQL 类型名 key。
    /// - `target_type`：Java `Class<?>` 对应的目标类型。
    pub fn insert(
        &mut self,
        sql_type_name: impl Into<String>,
        target_type: RdbcTargetType,
    ) -> Option<RdbcTargetType> {
        self.mappings.insert(sql_type_name.into(), target_type)
    }

    /// 返回指定 SQL 类型名的目标类型。
    pub fn get(&self, sql_type_name: &str) -> Option<&RdbcTargetType> {
        self.mappings.get(sql_type_name)
    }

    /// 返回全部映射。
    pub fn mappings(&self) -> &HashMap<String, RdbcTargetType> {
        &self.mappings
    }

    /// 返回映射是否为空。
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// 返回映射数量。
    pub fn len(&self) -> usize {
        self.mappings.len()
    }
}
