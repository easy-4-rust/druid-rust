use crate::types::{
    RdbcArray, RdbcBigDecimal as BigDecimal, RdbcBlob, RdbcCharacterLength, RdbcClob,
    RdbcInputStream, RdbcNClob, RdbcNaiveDate as NaiveDate, RdbcNaiveDateTime as NaiveDateTime,
    RdbcNaiveTime as NaiveTime, RdbcObject, RdbcReader, RdbcRef, RdbcRowId, RdbcSqlXml,
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
    /// 可空 Ref；SQL NULL 仍保留 updateRef family。
    Ref(Option<RdbcRef>),
    /// 可空 Blob。
    Blob(Option<RdbcBlob>),
    /// 可空 Clob。
    Clob(Option<RdbcClob>),
    /// 可空 Array。
    Array(Option<RdbcArray>),
    /// 可空 RowId。
    RowId(Option<RdbcRowId>),
    /// 可空 NClob。
    NClob(Option<RdbcNClob>),
    /// 可空 SQLXML。
    SqlXml(Option<RdbcSqlXml>),
    /// Blob stream 更新，独立于 updateBinaryStream。
    BlobStream {
        /// 可空调用者 stream。
        stream: Option<RdbcInputStream>,
        /// 原长度重载。
        length: RdbcStreamLength,
    },
    /// Clob Reader 更新，独立于 updateCharacterStream。
    ClobReader {
        /// 可空调用者 Reader。
        reader: Option<RdbcReader>,
        /// 原长度重载。
        length: RdbcCharacterLength,
    },
    /// NClob Reader 更新，独立于 updateNCharacterStream。
    NClobReader {
        /// 可空调用者 Reader。
        reader: Option<RdbcReader>,
        /// 原长度重载。
        length: RdbcCharacterLength,
    },
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

impl ResultSetUpdate {
    /// 返回 LOB 流更新对应的默认不支持操作名；其他更新返回 `None`。
    /// 参数 `by_label` 区分原列名与下标重载，不读取或消费流资源。
    /// 对应 Java: ResultSet#updateBlob、updateClob、updateNClob。
    pub(crate) fn unsupported_lob_stream_operation(&self, by_label: bool) -> Option<&'static str> {
        match (self, by_label) {
            (Self::BlobStream { .. }, false) => Some("result_set_update_blob_stream"),
            (Self::BlobStream { .. }, true) => Some("result_set_update_blob_stream_by_label"),
            (Self::ClobReader { .. }, false) => Some("result_set_update_clob_reader"),
            (Self::ClobReader { .. }, true) => Some("result_set_update_clob_reader_by_label"),
            (Self::NClobReader { .. }, false) => Some("result_set_update_n_clob_reader"),
            (Self::NClobReader { .. }, true) => Some("result_set_update_n_clob_reader_by_label"),
            _ => None,
        }
    }
}
