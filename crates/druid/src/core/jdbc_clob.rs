//! JDBC Clob 平台对象与物理 SPI。
//!
//! 对应 Java 平台对象：`java.sql.Clob`。Clob 是驱动拥有的字符资源句柄，
//! 不能在 Druid 池化层简化成 Rust String。

use super::{DruidError, JavaString, JdbcInputStream, JdbcOutputStream, JdbcReader, JdbcWriter};
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// 物理 Clob 的完整 JDBC 操作契约。
///
/// 位置、长度和 offset 保留 Java 有符号类型，由驱动执行一基位置校验、
/// 字符计数和释放后错误。
pub trait PhysicalClob: fmt::Debug + Send + Sync {
    /// 返回具体驱动对象。
    fn as_any(&self) -> &dyn Any;

    /// 对应 Java：`Clob#length()`。
    fn length(&self) -> Result<i64, DruidError>;

    /// 对应 Java：`Clob#getSubString(long, int)`。
    fn get_sub_string(&self, position: i64, length: i32) -> Result<JavaString, DruidError>;

    /// 对应 Java：`Clob#getCharacterStream()`。
    fn get_character_stream(&self) -> Result<JdbcReader, DruidError>;

    /// 对应 Java：`Clob#getAsciiStream()`。
    fn get_ascii_stream(&self) -> Result<JdbcInputStream, DruidError>;

    /// 对应 Java：`Clob#position(String, long)`。
    fn position_string(&self, pattern: &JavaString, start: i64) -> Result<Option<i64>, DruidError>;

    /// 对应 Java：`Clob#position(Clob, long)`。
    fn position_clob(&self, pattern: &JdbcClob, start: i64) -> Result<Option<i64>, DruidError>;

    /// 对应 Java：`Clob#setString(long, String)`。
    fn set_string(&self, position: i64, value: &JavaString) -> Result<i32, DruidError>;

    /// 对应 Java：`Clob#setString(long, String, int, int)`。
    fn set_string_range(
        &self,
        position: i64,
        value: &JavaString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError>;

    /// 对应 Java：`Clob#setAsciiStream(long)`。
    fn set_ascii_stream(&self, position: i64) -> Result<JdbcOutputStream, DruidError>;

    /// 对应 Java：`Clob#setCharacterStream(long)`。
    fn set_character_stream(&self, position: i64) -> Result<JdbcWriter, DruidError>;

    /// 对应 Java：`Clob#truncate(long)`。
    fn truncate(&self, length: i64) -> Result<(), DruidError>;

    /// 对应 Java：`Clob#free()`。
    fn free(&self) -> Result<(), DruidError>;

    /// 返回 Clob 是否已经释放。
    fn is_freed(&self) -> bool;

    /// 对应 Java：`Clob#getCharacterStream(long, long)`。
    fn get_character_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<JdbcReader, DruidError>;
}

/// 对外 JDBC Clob 句柄。
///
/// Clone 保留物理对象身份，不隐式读取或比较字符内容。
#[derive(Clone)]
pub struct JdbcClob {
    physical: Arc<dyn PhysicalClob>,
}

impl JdbcClob {
    /// 包装物理 Clob Adapter。
    pub fn new(physical: Arc<dyn PhysicalClob>) -> Self {
        Self { physical }
    }

    /// 返回物理 Clob SPI。
    pub fn physical(&self) -> &dyn PhysicalClob {
        self.physical.as_ref()
    }

    /// 对应 Java：`Clob#length()`。
    pub fn length(&self) -> Result<i64, DruidError> {
        self.physical.length()
    }

    /// 对应 Java：`Clob#getSubString(long, int)`。
    pub fn get_sub_string(&self, position: i64, length: i32) -> Result<JavaString, DruidError> {
        self.physical.get_sub_string(position, length)
    }

    /// 对应 Java：`Clob#getCharacterStream()`。
    pub fn get_character_stream(&self) -> Result<JdbcReader, DruidError> {
        self.physical.get_character_stream()
    }

    /// 对应 Java：`Clob#getAsciiStream()`。
    pub fn get_ascii_stream(&self) -> Result<JdbcInputStream, DruidError> {
        self.physical.get_ascii_stream()
    }

    /// 对应 Java：`Clob#position(String, long)`。
    pub fn position_string(
        &self,
        pattern: &JavaString,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        self.physical.position_string(pattern, start)
    }

    /// 对应 Java：`Clob#position(Clob, long)`。
    pub fn position_clob(&self, pattern: &JdbcClob, start: i64) -> Result<Option<i64>, DruidError> {
        self.physical.position_clob(pattern, start)
    }

    /// 对应 Java：`Clob#setString(long, String)`。
    pub fn set_string(&self, position: i64, value: &JavaString) -> Result<i32, DruidError> {
        self.physical.set_string(position, value)
    }

    /// 对应 Java：`Clob#setString(long, String, int, int)`。
    pub fn set_string_range(
        &self,
        position: i64,
        value: &JavaString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        self.physical
            .set_string_range(position, value, offset, length)
    }

    /// 对应 Java：`Clob#setAsciiStream(long)`。
    pub fn set_ascii_stream(&self, position: i64) -> Result<JdbcOutputStream, DruidError> {
        self.physical.set_ascii_stream(position)
    }

    /// 对应 Java：`Clob#setCharacterStream(long)`。
    pub fn set_character_stream(&self, position: i64) -> Result<JdbcWriter, DruidError> {
        self.physical.set_character_stream(position)
    }

    /// 对应 Java：`Clob#truncate(long)`。
    pub fn truncate(&self, length: i64) -> Result<(), DruidError> {
        self.physical.truncate(length)
    }

    /// 对应 Java：`Clob#free()`。
    pub fn free(&self) -> Result<(), DruidError> {
        self.physical.free()
    }

    /// 返回 Clob 是否已经释放。
    pub fn is_freed(&self) -> bool {
        self.physical.is_freed()
    }

    /// 对应 Java：`Clob#getCharacterStream(long, long)`。
    pub fn get_character_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<JdbcReader, DruidError> {
        self.physical.get_character_stream_range(position, length)
    }
}

impl fmt::Debug for JdbcClob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JdbcClob")
            .field("physical", &self.physical)
            .field("freed", &self.is_freed())
            .finish()
    }
}

impl PartialEq for JdbcClob {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for JdbcClob {}
