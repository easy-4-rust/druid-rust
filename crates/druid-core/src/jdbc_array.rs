//! JDBC `Array` 平台资源。
//!
//! 对应 Java 平台对象：`java.sql.Array`。

use crate::{CallableOutputValue, CallableTypeMap, DruidError, JdbcResultSet};
use std::fmt;
use std::sync::Arc;

/// 物理 JDBC `Array` SPI，保留值、范围、类型映射、结果集与释放重载。
pub trait PhysicalArray: fmt::Debug + Send + Sync {
    /// 返回数组元素 SQL 类型名。
    fn base_type_name(&self) -> Result<String, DruidError>;

    /// 返回数组元素 `java.sql.Types` 编号。
    fn base_type(&self) -> Result<i32, DruidError>;

    /// 使用默认映射读取全部元素。
    fn values(&self) -> Result<Vec<CallableOutputValue>, DruidError>;

    /// 使用显式类型映射读取全部元素。
    fn values_with_type_map(
        &self,
        type_map: &CallableTypeMap,
    ) -> Result<Vec<CallableOutputValue>, DruidError>;

    /// 从 Java 1-based index 读取 `count` 个元素。
    fn values_range(&self, index: i64, count: i32) -> Result<Vec<CallableOutputValue>, DruidError>;

    /// 使用显式类型映射读取指定范围。
    fn values_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        type_map: &CallableTypeMap,
    ) -> Result<Vec<CallableOutputValue>, DruidError>;

    /// 使用默认映射返回全部元素结果集。
    fn result_set(&self) -> Result<JdbcResultSet, DruidError>;

    /// 使用显式类型映射返回全部元素结果集。
    fn result_set_with_type_map(
        &self,
        type_map: &CallableTypeMap,
    ) -> Result<JdbcResultSet, DruidError>;

    /// 返回指定范围结果集。
    fn result_set_range(&self, index: i64, count: i32) -> Result<JdbcResultSet, DruidError>;

    /// 使用显式类型映射返回指定范围结果集。
    fn result_set_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        type_map: &CallableTypeMap,
    ) -> Result<JdbcResultSet, DruidError>;

    /// 释放数组资源。
    fn free(&self) -> Result<(), DruidError>;

    /// 返回数组是否已释放。
    fn is_freed(&self) -> bool;
}

/// 不泄漏具体驱动类型的 JDBC `Array` 句柄。
#[derive(Clone)]
pub struct JdbcArray {
    physical: Arc<dyn PhysicalArray>,
}

impl JdbcArray {
    /// 包装物理数组。
    pub fn new(physical: Arc<dyn PhysicalArray>) -> Self {
        Self { physical }
    }

    /// 返回数组元素 SQL 类型名。
    pub fn base_type_name(&self) -> Result<String, DruidError> {
        self.physical.base_type_name()
    }

    /// 返回数组元素 SQL 类型编号。
    pub fn base_type(&self) -> Result<i32, DruidError> {
        self.physical.base_type()
    }

    /// 读取全部元素。
    pub fn values(&self) -> Result<Vec<CallableOutputValue>, DruidError> {
        self.physical.values()
    }

    /// 使用显式类型映射读取全部元素。
    pub fn values_with_type_map(
        &self,
        type_map: &CallableTypeMap,
    ) -> Result<Vec<CallableOutputValue>, DruidError> {
        self.physical.values_with_type_map(type_map)
    }

    /// 读取指定范围。
    pub fn values_range(
        &self,
        index: i64,
        count: i32,
    ) -> Result<Vec<CallableOutputValue>, DruidError> {
        self.physical.values_range(index, count)
    }

    /// 使用显式类型映射读取指定范围。
    pub fn values_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        type_map: &CallableTypeMap,
    ) -> Result<Vec<CallableOutputValue>, DruidError> {
        self.physical
            .values_range_with_type_map(index, count, type_map)
    }

    /// 返回全部元素结果集。
    pub fn result_set(&self) -> Result<JdbcResultSet, DruidError> {
        self.physical.result_set()
    }

    /// 使用显式类型映射返回全部元素结果集。
    pub fn result_set_with_type_map(
        &self,
        type_map: &CallableTypeMap,
    ) -> Result<JdbcResultSet, DruidError> {
        self.physical.result_set_with_type_map(type_map)
    }

    /// 返回指定范围结果集。
    pub fn result_set_range(&self, index: i64, count: i32) -> Result<JdbcResultSet, DruidError> {
        self.physical.result_set_range(index, count)
    }

    /// 使用显式类型映射返回指定范围结果集。
    pub fn result_set_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        type_map: &CallableTypeMap,
    ) -> Result<JdbcResultSet, DruidError> {
        self.physical
            .result_set_range_with_type_map(index, count, type_map)
    }

    /// 释放数组。
    pub fn free(&self) -> Result<(), DruidError> {
        self.physical.free()
    }

    /// 返回是否已释放。
    pub fn is_freed(&self) -> bool {
        self.physical.is_freed()
    }

    /// 返回物理数组 SPI。
    pub fn physical(&self) -> &dyn PhysicalArray {
        self.physical.as_ref()
    }
}

impl fmt::Debug for JdbcArray {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JdbcArray")
            .field("physical", &self.physical)
            .field("freed", &self.is_freed())
            .finish()
    }
}

impl PartialEq for JdbcArray {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for JdbcArray {}
