//! `CallableStatement` OUT 参数描述。
//!
//! 对应 Java：`CallableStatement#registerOutParameter(...)` 的参数集合。

/// OUT 参数的 RDBC SQL 类型及可选 scale/typeName。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableOutParameter {
    sql_type: i32,
    scale: Option<i32>,
    type_name: Option<String>,
}

impl CallableOutParameter {
    /// 创建只包含 SQL 类型的 OUT 参数。
    pub fn new(sql_type: i32) -> Self {
        Self {
            sql_type,
            scale: None,
            type_name: None,
        }
    }

    /// 创建带小数位数的 OUT 参数。
    pub fn with_scale(sql_type: i32, scale: i32) -> Self {
        Self {
            sql_type,
            scale: Some(scale),
            type_name: None,
        }
    }

    /// 创建带数据库类型名的 OUT 参数。
    pub fn with_type_name(sql_type: i32, type_name: impl Into<String>) -> Self {
        Self {
            sql_type,
            scale: None,
            type_name: Some(type_name.into()),
        }
    }

    /// 返回 RDBC SQL 类型常量。
    pub fn sql_type(&self) -> i32 {
        self.sql_type
    }

    /// 返回可选 scale。
    pub fn scale(&self) -> Option<i32> {
        self.scale
    }

    /// 返回可选数据库类型名。
    pub fn type_name(&self) -> Option<&str> {
        self.type_name.as_deref()
    }
}
