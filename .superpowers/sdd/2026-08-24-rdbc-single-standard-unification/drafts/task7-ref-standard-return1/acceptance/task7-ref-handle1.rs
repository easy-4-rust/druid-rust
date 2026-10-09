use crate::error::RdbcError;
use crate::resource::{Ref, ResourceIdentity, ResourceOperations};
use crate::types::{RdbcObject, RdbcTypeMap};
use std::fmt;
use std::sync::Arc;

/// SQL引用的共享值句柄；对应 Java: java.sql.Ref。
/// 只承载唯一标准Ref对象，不定义第二个驱动接口；克隆保持引用身份，不物化目标属性。
/// 驱动对象仅要求Send，互斥访问提供独占可变调用；不附加Java未定义的free或连接失效规则。
#[derive(Clone)]
pub struct RdbcRef {
    value: Arc<tokio::sync::Mutex<Box<dyn Ref>>>,
    identity: Arc<ResourceIdentity>,
    capabilities: ResourceOperations,
}

impl RdbcRef {
    pub(crate) fn from_standard(value: Box<dyn Ref>) -> Self {
        let capabilities = value.capabilities() & ResourceOperations::reference();
        Self {
            value: Arc::new(tokio::sync::Mutex::new(value)),
            identity: Arc::new(ResourceIdentity::local()),
            capabilities,
        }
    }

    fn require(&self, capability: ResourceOperations, operation: &'static str) -> Result<(), RdbcError> {
        if self.capabilities.contains(capability) { Ok(()) } else { Err(RdbcError::feature_not_supported(operation)) }
    }

    /// 返回引用目标的完整SQL类型名，无参数；返回原驱动错误。对应 Java: Ref#getBaseTypeName。
    pub async fn base_type_name(&self) -> Result<String, RdbcError> {
        self.require(ResourceOperations::READ, "Ref#getBaseTypeName")?;
        self.value.lock().await.base_type_name().await
    }

    /// 返回引用目标的完整SQL类型名，无参数；返回原驱动错误。对应 Java: Ref#getBaseTypeName。
    pub async fn get_base_type_name(&self) -> Result<String, RdbcError> { self.base_type_name().await }

    /// 按连接关联映射读取目标，无匹配时返回Struct；无参数，返回对象或原错误。对应 Java: Ref#getObject()。
    pub async fn object(&self) -> Result<RdbcObject, RdbcError> {
        self.require(ResourceOperations::READ, "Ref#getObject")?;
        self.value.lock().await.object().await
    }

    /// 按默认映射读取目标，无参数；返回对象或原错误。对应 Java: Ref#getObject()。
    pub async fn get_object(&self) -> Result<RdbcObject, RdbcError> { self.object().await }

    /// 使用参数type_map显式映射目标，返回对象或原错误。对应 Java: Ref#getObject(Map)。
    pub async fn object_with_type_map(&self, type_map: &RdbcTypeMap) -> Result<RdbcObject, RdbcError> {
        self.require(ResourceOperations::READ | ResourceOperations::TYPE_MAP, "Ref#getObject(Map)")?;
        self.value.lock().await.object_with_type_map(type_map).await
    }

    /// 使用参数type_map显式映射目标，返回对象或原错误。对应 Java: Ref#getObject(Map)。
    pub async fn get_object_with_type_map(&self, type_map: &RdbcTypeMap) -> Result<RdbcObject, RdbcError> { self.object_with_type_map(type_map).await }

    /// 将参数value原样交给驱动转换为SQL结构值；成功返回空值或原错误。对应 Java: Ref#setObject。
    pub async fn set_object(&self, value: RdbcObject) -> Result<(), RdbcError> {
        self.require(ResourceOperations::WRITE, "Ref#setObject")?;
        self.value.lock().await.set_object(value).await
    }

    /// 返回本句柄稳定身份，无参数；克隆共享同一身份（Rust扩展）。
    #[must_use]
    pub fn resource_id(&self) -> &str { self.identity.as_str() }

    /// 返回标准对象声明与Ref操作范围的交集，无参数（Rust扩展）。
    #[must_use]
    pub fn capabilities(&self) -> ResourceOperations { self.capabilities }
}

impl fmt::Debug for RdbcRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("RdbcRef").field("identity", &self.identity).field("capabilities", &self.capabilities).finish_non_exhaustive()
    }
}

impl PartialEq for RdbcRef {
    fn eq(&self, other: &Self) -> bool { Arc::ptr_eq(&self.value, &other.value) }
}
impl Eq for RdbcRef {}
