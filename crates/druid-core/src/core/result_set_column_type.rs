//! RDBC 结果集列类型。
//!
//! 对应 Java 平台常量：`java.sql.Types`。该类型只表达当前 Druid Adapter
//! 能无损区分的类型族，不猜测驱动未提供的整数宽度或 vendor type。

/// 结果集列的通用类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultSetColumnType {
    /// 驱动没有提供类型信息，对应 `Types.OTHER`。
    Unknown,
    /// SQL BOOLEAN。
    Boolean,
    /// SQL 整数；当前通用值使用有符号 64 位表示。
    Integer,
    /// SQL 浮点数；当前通用值使用双精度表示。
    Float,
    /// SQL DECIMAL/NUMERIC。
    Decimal,
    /// SQL DATE。
    Date,
    /// SQL TIME。
    Time,
    /// SQL TIMESTAMP/DATETIME。
    Timestamp,
    /// SQL 字符串。
    Text,
    /// SQL 二进制。
    Binary,
}

impl ResultSetColumnType {
    /// 返回 `java.sql.Types` 数值。
    pub const fn rdbc_type(self) -> i32 {
        match self {
            Self::Unknown => 1_111,
            Self::Boolean => 16,
            Self::Integer => -5,
            Self::Float => 8,
            Self::Decimal => 3,
            Self::Date => 91,
            Self::Time => 92,
            Self::Timestamp => 93,
            Self::Text => 12,
            Self::Binary => -3,
        }
    }

    /// 返回标准 SQL 类型名。
    pub const fn type_name(self) -> &'static str {
        match self {
            Self::Unknown => "OTHER",
            Self::Boolean => "BOOLEAN",
            Self::Integer => "BIGINT",
            Self::Float => "DOUBLE",
            Self::Decimal => "DECIMAL",
            Self::Date => "DATE",
            Self::Time => "TIME",
            Self::Timestamp => "TIMESTAMP",
            Self::Text => "VARCHAR",
            Self::Binary => "VARBINARY",
        }
    }

    /// 返回 Java typed getter 对应的默认类名。
    pub const fn class_name(self) -> &'static str {
        match self {
            Self::Unknown => "java.lang.Object",
            Self::Boolean => "java.lang.Boolean",
            Self::Integer => "java.lang.Long",
            Self::Float => "java.lang.Double",
            Self::Decimal => "java.math.BigDecimal",
            Self::Date => "java.sql.Date",
            Self::Time => "java.sql.Time",
            Self::Timestamp => "java.sql.Timestamp",
            Self::Text => "java.lang.String",
            Self::Binary => "[B",
        }
    }

    /// 返回该类型是否为有符号数值。
    pub const fn is_signed(self) -> bool {
        matches!(self, Self::Integer | Self::Float | Self::Decimal)
    }
}
