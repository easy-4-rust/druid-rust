//! JDBC `SQLXML` 平台资源。
//!
//! 对应 Java 平台对象：`java.sql.SQLXML`。

use super::{
    DruidError, JavaString, JdbcInputStream, JdbcOutputStream, JdbcReader, JdbcWriter,
    JdbcXmlRepresentationType, JdbcXmlResult, JdbcXmlSource,
};
use std::fmt;
use std::sync::Arc;

/// 物理 JDBC `SQLXML` SPI，覆盖 Java 的九个操作族。
pub trait PhysicalSqlXml: fmt::Debug + Send + Sync {
    /// 释放 XML 资源。
    fn free(&self) -> Result<(), DruidError>;

    /// 返回是否已经释放。
    fn is_freed(&self) -> bool;

    /// 返回二进制输入流。
    fn binary_stream(&self) -> Result<JdbcInputStream, DruidError>;

    /// 返回用于写入 XML 的二进制输出流。
    fn set_binary_stream(&self) -> Result<JdbcOutputStream, DruidError>;

    /// 返回字符 Reader。
    fn character_stream(&self) -> Result<JdbcReader, DruidError>;

    /// 返回用于写入 XML 的 Writer。
    fn set_character_stream(&self) -> Result<JdbcWriter, DruidError>;

    /// 返回 XML 字符串。
    fn string(&self) -> Result<JavaString, DruidError>;

    /// 设置 XML 字符串。
    fn set_string(&self, value: &JavaString) -> Result<(), DruidError>;

    /// 返回指定表示类型的 XML Source。
    fn source(
        &self,
        representation: &JdbcXmlRepresentationType,
    ) -> Result<JdbcXmlSource, DruidError>;

    /// 返回指定表示类型的 XML Result。
    fn result(
        &self,
        representation: &JdbcXmlRepresentationType,
    ) -> Result<JdbcXmlResult, DruidError>;
}

/// 不泄漏具体驱动类型的 JDBC `SQLXML` 句柄。
#[derive(Clone)]
pub struct JdbcSqlXml {
    physical: Arc<dyn PhysicalSqlXml>,
}

impl JdbcSqlXml {
    /// 包装物理 `SQLXML`。
    pub fn new(physical: Arc<dyn PhysicalSqlXml>) -> Self {
        Self { physical }
    }

    /// 释放 XML 资源。
    pub fn free(&self) -> Result<(), DruidError> {
        self.physical.free()
    }

    /// 返回是否已释放。
    pub fn is_freed(&self) -> bool {
        self.physical.is_freed()
    }

    /// 返回二进制输入流。
    pub fn binary_stream(&self) -> Result<JdbcInputStream, DruidError> {
        self.physical.binary_stream()
    }

    /// 返回二进制输出流。
    pub fn set_binary_stream(&self) -> Result<JdbcOutputStream, DruidError> {
        self.physical.set_binary_stream()
    }

    /// 返回字符 Reader。
    pub fn character_stream(&self) -> Result<JdbcReader, DruidError> {
        self.physical.character_stream()
    }

    /// 返回字符 Writer。
    pub fn set_character_stream(&self) -> Result<JdbcWriter, DruidError> {
        self.physical.set_character_stream()
    }

    /// 返回 XML 字符串。
    pub fn string(&self) -> Result<JavaString, DruidError> {
        self.physical.string()
    }

    /// 设置 XML 字符串。
    pub fn set_string(&self, value: &JavaString) -> Result<(), DruidError> {
        self.physical.set_string(value)
    }

    /// 返回 XML Source。
    pub fn source(
        &self,
        representation: &JdbcXmlRepresentationType,
    ) -> Result<JdbcXmlSource, DruidError> {
        self.physical.source(representation)
    }

    /// 返回 XML Result。
    pub fn result(
        &self,
        representation: &JdbcXmlRepresentationType,
    ) -> Result<JdbcXmlResult, DruidError> {
        self.physical.result(representation)
    }

    /// 返回物理 `SQLXML` SPI。
    pub fn physical(&self) -> &dyn PhysicalSqlXml {
        self.physical.as_ref()
    }
}

impl fmt::Debug for JdbcSqlXml {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JdbcSqlXml")
            .field("physical", &self.physical)
            .field("freed", &self.is_freed())
            .finish()
    }
}

impl PartialEq for JdbcSqlXml {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for JdbcSqlXml {}
