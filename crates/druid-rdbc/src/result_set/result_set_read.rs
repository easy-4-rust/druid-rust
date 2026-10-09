use crate::types::RdbcCalendarArgument;

/// 保留强类型读取的重载参数，供唯一标准结果集向驱动分派。
/// 对应 Java: java.sql.ResultSet 的标量、BigDecimal、日期时间和资源 getter。
#[derive(Debug, Clone, PartialEq)]
pub enum ResultSetRead {
    /// 可空字符串读取，保留对应 Java getString 原生重载身份。
    String,
    /// 可空 national 字符串读取，保留对应 Java getNString 原生重载身份。
    NString,
    /// 布尔读取，保留对应 Java getBoolean 原生重载身份。
    Boolean,
    /// 64 位整数读取，保留对应 Java getLong 原生重载身份。
    Long,
    /// 32 位整数读取，保留对应 Java getInt 原生重载身份。
    Int,
    /// 16 位整数读取，保留对应 Java getShort 原生重载身份。
    Short,
    /// 8 位整数读取，保留对应 Java getByte 原生重载身份。
    Byte,
    /// 64 位浮点读取，保留对应 Java getDouble 原生重载身份。
    Double,
    /// 32 位浮点读取，保留对应 Java getFloat 原生重载身份。
    Float,
    /// 可空字节数组读取，保留对应 Java getBytes 原生重载身份。
    Bytes,

    /// Decimal 读取；None 为无 scale 重载，Some 保留原 scale。
    BigDecimal(Option<i32>),
    /// Date 读取，保留无 Calendar、空 Calendar 和指定 Calendar 的区别。
    Date(RdbcCalendarArgument),
    /// Time 读取，保留 Calendar 参数。
    Time(RdbcCalendarArgument),
    /// Timestamp 读取，保留 Calendar 参数。
    Timestamp(RdbcCalendarArgument),
    /// SQL Ref 读取。
    Ref,
    /// SQL Blob 读取。
    Blob,
    /// SQL Clob 读取。
    Clob,
    /// SQL Array 读取。
    Array,
    /// URL 读取。
    Url,
    /// SQL RowId 读取。
    RowId,
    /// SQL NClob 读取。
    NClob,
    /// SQLXML 读取。
    SqlXml,
}
