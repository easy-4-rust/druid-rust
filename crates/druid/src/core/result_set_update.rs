//! `ResultSet` 标量与流更新重载的无损描述。
//!
//! 对应 Java：`java.sql.ResultSet` 的 `updateNull`、标量 `updateXxx`、
//! `updateObject`、`updateNString` 及 ASCII/Binary/Character stream 更新族。
//! Java 依靠方法重载区分值类型和长度参数；Rust 用本枚举在物理 SPI 边界保留
//! 相同的重载身份，禁止提前物化流或把不同 setter 压缩成无类型值。

use super::{JdbcCharacterLength, JdbcInputStream, JdbcObject, JdbcReader, JdbcStreamLength};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

/// `ResultSet` 单列更新请求。
///
/// 每个枚举分支都对应一个 Java setter 族；其中 `Option` 精确保留 Java
/// 引用参数的 `null`，长度枚举区分未指定、`int` 与 `long` 重载。
#[derive(Debug, Clone, PartialEq)]
pub enum ResultSetUpdate {
    /// `ResultSet#updateNull`。
    Null,
    /// `ResultSet#updateBoolean`。
    Boolean(bool),
    /// `ResultSet#updateByte`。
    Byte(i8),
    /// `ResultSet#updateShort`。
    Short(i16),
    /// `ResultSet#updateInt`。
    Int(i32),
    /// `ResultSet#updateLong`。
    Long(i64),
    /// `ResultSet#updateFloat`。
    Float(f32),
    /// `ResultSet#updateDouble`。
    Double(f64),
    /// `ResultSet#updateBigDecimal`；`None` 对应 Java `null`。
    BigDecimal(Option<BigDecimal>),
    /// `ResultSet#updateString`；`None` 对应 Java `null`。
    String(Option<String>),
    /// `ResultSet#updateBytes`；`None` 对应 Java `null`。
    Bytes(Option<Vec<u8>>),
    /// `ResultSet#updateDate`；`None` 对应 Java `null`。
    Date(Option<NaiveDate>),
    /// `ResultSet#updateTime`；`None` 对应 Java `null`。
    Time(Option<NaiveTime>),
    /// `ResultSet#updateTimestamp`；`None` 对应 Java `null`。
    Timestamp(Option<NaiveDateTime>),
    /// `ResultSet#updateObject(Object)`。
    Object(JdbcObject),
    /// `ResultSet#updateObject(Object, int)`。
    ObjectWithScaleOrLength {
        /// Java 参数 `x`。
        value: JdbcObject,
        /// Java 参数 `scaleOrLength`。
        scale_or_length: i32,
    },
    /// `ResultSet#updateNString`；`None` 对应 Java `null`。
    NString(Option<String>),
    /// `ResultSet#updateAsciiStream`，保留流对象和长度重载。
    AsciiStream {
        /// Java 参数 `x`。
        stream: Option<JdbcInputStream>,
        /// 未指定、`int` 或 `long` 长度。
        length: JdbcStreamLength,
    },
    /// `ResultSet#updateBinaryStream`，保留流对象和长度重载。
    BinaryStream {
        /// Java 参数 `x`。
        stream: Option<JdbcInputStream>,
        /// 未指定、`int` 或 `long` 长度。
        length: JdbcStreamLength,
    },
    /// `ResultSet#updateCharacterStream`，保留 Reader 和长度重载。
    CharacterStream {
        /// Java 参数 `x`。
        reader: Option<JdbcReader>,
        /// 未指定、`int` 或 `long` 长度。
        length: JdbcCharacterLength,
    },
    /// `ResultSet#updateNCharacterStream`，保留 Reader 和长度重载。
    NCharacterStream {
        /// Java 参数 `x`。
        reader: Option<JdbcReader>,
        /// 未指定或 `long` 长度。
        length: JdbcCharacterLength,
    },
}
