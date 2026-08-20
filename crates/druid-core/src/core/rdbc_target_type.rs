//! RDBC `getObject(..., Class<T>)` 的目标类型描述。
//!
//! 对应 Java 平台对象：`java.lang.Class`。Rust 不使用 JVM 反射类对象，因此以
//! 稳定枚举保留调用方要求的目标类型；驱动 Adapter 负责执行实际转换。

/// `getObject` typed 重载的目标类型。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RdbcTargetType {
    /// `java.lang.String`。
    String,
    /// `java.lang.Boolean`。
    Boolean,
    /// `java.lang.Byte`。
    Byte,
    /// `java.lang.Short`。
    Short,
    /// `java.lang.Integer`。
    Integer,
    /// `java.lang.Long`。
    Long,
    /// `java.lang.Float`。
    Float,
    /// `java.lang.Double`。
    Double,
    /// `byte[]`。
    Bytes,
    /// `java.math.BigDecimal`。
    BigDecimal,
    /// `java.sql.Date`。
    Date,
    /// `java.sql.Time`。
    Time,
    /// `java.sql.Timestamp`。
    Timestamp,
    /// `java.sql.Blob`。
    Blob,
    /// `java.sql.Clob`。
    Clob,
    /// `java.sql.NClob`。
    NClob,
    /// `java.sql.Array`。
    Array,
    /// `java.sql.Ref`。
    Ref,
    /// `java.sql.RowId`。
    RowId,
    /// `java.sql.SQLXML`。
    SqlXml,
    /// `java.net.URL`。
    Url,
    /// 驱动或应用定义的 Java 类名。
    Custom(String),
}
