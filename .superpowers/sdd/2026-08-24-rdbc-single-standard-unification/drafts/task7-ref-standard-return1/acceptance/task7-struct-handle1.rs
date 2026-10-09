use crate::error::RdbcError;
use crate::resource::{ResourceOperations, Struct};
use crate::types::{RdbcObject, RdbcTypeMap};
use std::fmt;
use std::sync::Arc;

/// SQL结构对象的共享值句柄；对应 Java: java.sql.Struct。
/// 承载唯一标准Struct，保留属性的延迟读取和引用身份；不要求驱动Sync，不新增free或连接所有者。
#[derive(Clone)]
pub struct RdbcStruct {
    value: Arc<tokio::sync::Mutex<Box<dyn Struct>>>,
    capabilities: ResourceOperations,
}
impl RdbcStruct {
    pub(crate) fn from_standard(value: Box<dyn Struct>) -> Self {
        let capabilities = value.capabilities() & (ResourceOperations::READ | ResourceOperations::TYPE_MAP);
        Self { value: Arc::new(tokio::sync::Mutex::new(value)), capabilities }
    }
    fn require(&self, capability: ResourceOperations, operation: &'static str) -> Result<(), RdbcError> {
        if self.capabilities.contains(capability) { Ok(()) } else { Err(RdbcError::feature_not_supported(operation)) }
    }
    /// 返回完整SQL类型名，无参数；数据库访问失败返回原错误。对应 Java: Struct#getSQLTypeName。
    pub async fn sql_type_name(&self) -> Result<String, RdbcError> {
        self.require(ResourceOperations::READ, "Struct#getSQLTypeName")?;
        self.value.lock().await.sql_type_name().await
    }
    /// 按声明顺序读取属性，无参数；使用连接关联映射，返回对象数组或原错误。对应 Java: Struct#getAttributes()。
    pub async fn attributes(&self) -> Result<Vec<RdbcObject>, RdbcError> {
        self.require(ResourceOperations::READ, "Struct#getAttributes")?;
        self.value.lock().await.attributes().await
    }
    /// 使用参数type_map读取有序属性，返回对象数组或原错误；不使用连接映射。对应 Java: Struct#getAttributes(Map)。
    pub async fn attributes_with_type_map(&self, type_map: &RdbcTypeMap) -> Result<Vec<RdbcObject>, RdbcError> {
        self.require(ResourceOperations::READ | ResourceOperations::TYPE_MAP, "Struct#getAttributes(Map)")?;
        self.value.lock().await.attributes_with_type_map(type_map).await
    }
    /// 返回标准对象声明与Struct操作范围的交集，无参数（Rust扩展）。
    #[must_use]
    pub fn capabilities(&self) -> ResourceOperations { self.capabilities }
}
impl fmt::Debug for RdbcStruct {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("RdbcStruct").field("capabilities", &self.capabilities).finish_non_exhaustive()
    }
}
impl PartialEq for RdbcStruct {
    fn eq(&self, other: &Self) -> bool { Arc::ptr_eq(&self.value, &other.value) }
}
impl Eq for RdbcStruct {}
