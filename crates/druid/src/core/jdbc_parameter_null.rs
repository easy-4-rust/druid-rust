//! 对应 Java：`com.alibaba.druid.proxy.jdbc.JdbcParameterNull`。

use super::{JdbcParameter, JdbcParameterValue};

/// 带 SQL 类型的 NULL 参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JdbcParameterNull(i32);

impl JdbcParameterNull {
    /// `Types.CHAR`。
    pub const CHAR: Self = Self(1);
    /// `Types.VARCHAR`。
    pub const VARCHAR: Self = Self(12);
    /// `Types.NVARCHAR`。
    pub const NVARCHAR: Self = Self(-9);
    /// `Types.BINARY`。
    pub const BINARY: Self = Self(-2);
    /// `Types.VARBINARY`。
    pub const VARBINARY: Self = Self(-3);
    /// Java 源码中 TINYINT 单例实际指向 INTEGER。
    pub const TINYINT: Self = Self(4);
    /// `Types.SMALLINT`。
    pub const SMALLINT: Self = Self(5);
    /// `Types.INTEGER`。
    pub const INTEGER: Self = Self(4);
    /// `Types.BIGINT`。
    pub const BIGINT: Self = Self(-5);
    /// `Types.DECIMAL`。
    pub const DECIMAL: Self = Self(3);
    /// `Types.NUMERIC`。
    pub const NUMERIC: Self = Self(2);
    /// `Types.FLOAT`。
    pub const FLOAT: Self = Self(6);
    /// `Types.DOUBLE`。
    pub const DOUBLE: Self = Self(8);
    /// `Types.NULL`。
    pub const NULL: Self = Self(0);
    /// `Types.DATE`。
    pub const DATE: Self = Self(91);
    /// `Types.TIME`。
    pub const TIME: Self = Self(92);
    /// `Types.TIMESTAMP`。
    pub const TIMESTAMP: Self = Self(93);

    /// 按 Java `valueOf(int)` 创建 typed null。
    pub const fn value_of(sql_type: i32) -> Self {
        // Java 的 TINYINT 分支历史上返回 INTEGER 单例，因而公开 sqlType 为
        // Types.INTEGER；其余已知分支与输入类型相同。
        if sql_type == -6 {
            Self(4)
        } else {
            Self(sql_type)
        }
    }
}

impl JdbcParameter for JdbcParameterNull {
    fn value(&self) -> Option<JdbcParameterValue> {
        None
    }

    fn length(&self) -> i64 {
        0
    }

    fn calendar(&self) -> Option<super::JdbcCalendar> {
        None
    }

    fn sql_type(&self) -> i32 {
        self.0
    }
}
