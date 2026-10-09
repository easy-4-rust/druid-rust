use super::sql_xml_value::SqlXmlValue;
use super::standard_resource_value::StandardResourceValue;
use crate::error::RdbcError;
use crate::resource::{
    RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcWriter, RdbcXmlRepresentationType,
    RdbcXmlResult, RdbcXmlSource, ResourceOperations, SqlXml,
};
use crate::types::RdbcString;
use std::{fmt, sync::Arc};
/// SQLXML共享值句柄；对应 Java: java.sql.SQLXML。
/// 克隆共享同一标准对象、一次性读写状态、身份与释放确认；只要求驱动Send。
#[derive(Clone)]
pub struct RdbcSqlXml {
    value: Arc<StandardResourceValue<SqlXmlValue>>,
}
impl RdbcSqlXml {
    /// 接管参数value唯一标准SQLXML，返回未提前读取或写入的共享句柄。
    pub(crate) fn from_standard(value: Box<dyn SqlXml>) -> Self {
        let capabilities = value.capabilities() & ResourceOperations::sql_xml();
        Self {
            value: Arc::new(StandardResourceValue::new(
                Box::new(SqlXmlValue::new(value)),
                capabilities,
                "SQLXML",
            )),
        }
    }
    async fn payload(
        &self,
        read: bool,
        capability: ResourceOperations,
        operation: &'static str,
    ) -> Result<tokio::sync::MutexGuard<'_, Box<SqlXmlValue>>, RdbcError> {
        let mut payload = self.value.lock(capability, operation).await?;
        // 生命周期及能力已在锁前后检查；仅真正取得驱动独占权后消费一次性机会。
        payload.admit(read)?;
        Ok(payload)
    }
    /// 返回原驱动二进制读取流，无参数；不提前读取或编码。 对应 Java: SQLXML#getBinaryStream。
    pub async fn binary_stream(&self) -> Result<RdbcInputStream, RdbcError> {
        self.payload(
            true,
            ResourceOperations::READ | ResourceOperations::STREAM,
            "SQLXML#getBinaryStream",
        )
        .await?
        .driver
        .binary_stream()
        .await
    }
    /// 返回原驱动二进制写入流，无参数；绑定前由调用者完成流关闭。 对应 Java: SQLXML#setBinaryStream。
    pub async fn set_binary_stream(&self) -> Result<RdbcOutputStream, RdbcError> {
        self.payload(
            false,
            ResourceOperations::WRITE | ResourceOperations::STREAM,
            "SQLXML#setBinaryStream",
        )
        .await?
        .driver
        .set_binary_stream()
        .await
    }
    /// 返回保留UTF-16的原字符读取流，无参数。 对应 Java: SQLXML#getCharacterStream。
    pub async fn character_stream(&self) -> Result<RdbcReader, RdbcError> {
        self.payload(
            true,
            ResourceOperations::READ | ResourceOperations::STREAM,
            "SQLXML#getCharacterStream",
        )
        .await?
        .driver
        .character_stream()
        .await
    }
    /// 返回原驱动字符写入器，无参数；调用者负责完成关闭。 对应 Java: SQLXML#setCharacterStream。
    pub async fn set_character_stream(&self) -> Result<RdbcWriter, RdbcError> {
        self.payload(
            false,
            ResourceOperations::WRITE | ResourceOperations::STREAM,
            "SQLXML#setCharacterStream",
        )
        .await?
        .driver
        .set_character_stream()
        .await
    }
    /// 返回原XML文本并消费读取机会，无参数；不替换孤立代理项。 对应 Java: SQLXML#getString。
    pub async fn string(&self) -> Result<RdbcString, RdbcError> {
        self.payload(true, ResourceOperations::empty(), "SQLXML#getString")
            .await?
            .driver
            .string()
            .await
    }
    /// 设置参数value的原UTF-16文本并消费写入机会，返回成功或原错误。 对应 Java: SQLXML#setString。
    pub async fn set_string(&self, value: &RdbcString) -> Result<(), RdbcError> {
        self.payload(false, ResourceOperations::WRITE, "SQLXML#setString")
            .await?
            .driver
            .set_string(value)
            .await
    }
    /// 按参数representation返回原Source；None保留Java null的厂商默认请求。 对应 Java: SQLXML#getSource。
    pub async fn source(
        &self,
        representation: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlSource, RdbcError> {
        self.payload(true, ResourceOperations::READ, "SQLXML#getSource")
            .await?
            .driver
            .source(representation)
            .await
    }
    /// 按参数representation返回原Result；None保留Java null，不改写框架身份。 对应 Java: SQLXML#setResult。
    pub async fn result(
        &self,
        representation: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlResult, RdbcError> {
        self.payload(false, ResourceOperations::WRITE, "SQLXML#setResult")
            .await?
            .driver
            .result(representation)
            .await
    }
    /// 对应Java getter命名的同一binary_stream入口，无参数；返回原驱动值并共享一次性读取机会。
    pub async fn get_binary_stream(&self) -> Result<RdbcInputStream, RdbcError> {
        self.binary_stream().await
    }
    /// 对应Java getter命名的同一character_stream入口，无参数；返回原驱动值并共享一次性读取机会。
    pub async fn get_character_stream(&self) -> Result<RdbcReader, RdbcError> {
        self.character_stream().await
    }
    /// 对应Java getter命名的同一string入口，无参数；返回原驱动值并共享一次性读取机会。
    pub async fn get_string(&self) -> Result<RdbcString, RdbcError> {
        self.string().await
    }
    /// 幂等释放，无参数；失败或取消不误报成功，后续free重试确认。对应 Java: SQLXML#free。
    pub async fn free(&self) -> Result<(), RdbcError> {
        self.value
            .free("SQLXML#free", |payload| payload.driver.free())
            .await
    }
    /// 返回是否已确认成功释放，无参数（Rust扩展）。
    #[must_use]
    pub fn is_freed(&self) -> bool {
        self.value.is_freed()
    }
    /// 返回稳定共享身份，无参数（Rust扩展）。
    #[must_use]
    pub fn resource_id(&self) -> &str {
        self.value.resource_id()
    }
    /// 返回驱动声明与SQLXML标准操作的交集，无参数（Rust扩展）。
    #[must_use]
    pub fn capabilities(&self) -> ResourceOperations {
        self.value.capabilities()
    }
}
impl fmt::Debug for RdbcSqlXml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RdbcSqlXml")
            .field("value", &self.value)
            .finish_non_exhaustive()
    }
}
impl PartialEq for RdbcSqlXml {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }
}
impl Eq for RdbcSqlXml {}
