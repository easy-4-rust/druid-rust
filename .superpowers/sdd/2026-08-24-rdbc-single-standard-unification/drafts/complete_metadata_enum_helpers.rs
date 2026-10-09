impl ColumnNullability {
    /// 返回 JDBC 可空性常量；无参数，返回 0（不可空）、1（可空）或 2（未知）。
    /// 对应 Java: java.sql.ResultSetMetaData 的 columnNoNulls/columnNullable/columnNullableUnknown。
    #[must_use]
    pub const fn rdbc_code(self) -> i32 {
        match self {
            Self::NoNulls => 0,
            Self::Nullable => 1,
            Self::Unknown => 2,
        }
    }
}

impl ResultSetColumnType {
    /// 返回此类型族的 JDBC 类型代码；无参数，Jdbc 保留驱动原代码，Named 返回 OTHER。
    /// 对应 Java: java.sql.Types；不推断未提供的整数宽度或厂商类型。
    #[must_use]
    pub const fn rdbc_type(&self) -> i32 {
        match self {
            Self::Unknown | Self::Named(_) => 1_111,
            Self::Boolean => 16,
            Self::Integer => -5,
            Self::Float => 8,
            Self::Decimal => 3,
            Self::Date => 91,
            Self::Time => 92,
            Self::Timestamp => 93,
            Self::Text => 12,
            Self::Binary => -3,
            Self::Jdbc(code) => *code,
        }
    }

    /// 返回类型族的默认 SQL 名称；无参数，Named 借用原厂商名，Jdbc 返回 OTHER。
    /// 对应 Java: java.sql.Types；实际驱动名称仍由 ResultSetMetaData#getColumnTypeName 提供。
    #[must_use]
    pub fn type_name(&self) -> &str {
        match self {
            Self::Unknown | Self::Jdbc(_) => "OTHER",
            Self::Boolean => "BOOLEAN",
            Self::Integer => "BIGINT",
            Self::Float => "DOUBLE",
            Self::Decimal => "DECIMAL",
            Self::Date => "DATE",
            Self::Time => "TIME",
            Self::Timestamp => "TIMESTAMP",
            Self::Text => "VARCHAR",
            Self::Binary => "VARBINARY",
            Self::Named(name) => name,
        }
    }

    /// 返回类型族默认 Java 类名；无参数，无法确定的类型返回 java.lang.Object。
    /// 对应 Java: java.sql.ResultSetMetaData#getColumnClassName；不覆盖列描述符的真实类名。
    #[must_use]
    pub const fn class_name(&self) -> &'static str {
        match self {
            Self::Unknown | Self::Jdbc(_) | Self::Named(_) => "java.lang.Object",
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

    /// 返回类型族默认是否为有符号数值；无参数，未知或厂商类型默认 false。
    /// 对应 Java: java.sql.ResultSetMetaData#isSigned；实际列属性由驱动描述符决定。
    #[must_use]
    pub const fn is_signed(&self) -> bool {
        matches!(self, Self::Integer | Self::Float | Self::Decimal)
    }
}
