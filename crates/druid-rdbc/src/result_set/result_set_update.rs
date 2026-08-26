use crate::types::{
    RdbcBigDecimal as BigDecimal, RdbcCharacterLength, RdbcInputStream, RdbcNaiveDate as NaiveDate,
    RdbcNaiveDateTime as NaiveDateTime, RdbcNaiveTime as NaiveTime, RdbcObject, RdbcReader,
    RdbcStreamLength,
};

/// ResultSet 单列更新值；每个 variant 保留一个 JDBC setter family 的 identity。
#[derive(Debug, Clone, PartialEq)]
pub enum ResultSetUpdate {
    /// SQL NULL。
    Null,
    /// Boolean value。
    Boolean(bool),
    /// Byte value。
    Byte(i8),
    /// Short value。
    Short(i16),
    /// Int value。
    Int(i32),
    /// Long value。
    Long(i64),
    /// Float value。
    Float(f32),
    /// Double value。
    Double(f64),
    /// 可空 arbitrary-precision decimal。
    BigDecimal(Option<BigDecimal>),
    /// 可空 String。
    String(Option<String>),
    /// 可空 bytes。
    Bytes(Option<Vec<u8>>),
    /// 可空 Date。
    Date(Option<NaiveDate>),
    /// 可空 Time。
    Time(Option<NaiveTime>),
    /// 可空 Timestamp。
    Timestamp(Option<NaiveDateTime>),
    /// 通用 object。
    Object(RdbcObject),
    /// 带 scale-or-length 的通用 object。
    ObjectWithScaleOrLength {
        /// 新值。
        value: RdbcObject,
        /// JDBC scaleOrLength。
        scale_or_length: i32,
    },
    /// 可空 national String。
    NString(Option<String>),
    /// ASCII stream 与长度重载 identity。
    AsciiStream {
        /// 可空 stream handle。
        stream: Option<RdbcInputStream>,
        /// 未指定、int 或 long length。
        length: RdbcStreamLength,
    },
    /// Binary stream 与长度重载 identity。
    BinaryStream {
        /// 可空 stream handle。
        stream: Option<RdbcInputStream>,
        /// 未指定、int 或 long length。
        length: RdbcStreamLength,
    },
    /// Character Reader 与长度重载 identity。
    CharacterStream {
        /// 可空 reader handle。
        reader: Option<RdbcReader>,
        /// 未指定、int 或 long length。
        length: RdbcCharacterLength,
    },
    /// National-character Reader 与长度重载 identity。
    NCharacterStream {
        /// 可空 reader handle。
        reader: Option<RdbcReader>,
        /// 未指定或 long length。
        length: RdbcCharacterLength,
    },
}
