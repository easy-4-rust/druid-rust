use crate::connection::Connection;
use crate::error::RdbcError;
use std::collections::BTreeMap;

/// Driver connection properties；有序映射保证 deterministic diagnostics。
pub type ConnectionProperties = BTreeMap<String, String>;

/// JDBC `DriverPropertyInfo` 的 immutable Rust descriptor。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverPropertyInfo {
    /// property name。
    pub name: String,
    /// optional configured value。
    pub value: Option<String>,
    /// 面向调用方的 property 说明。
    pub description: Option<String>,
    /// 是否必填。
    pub required: bool,
    /// 可选 choices；空集合表示 driver 未限制候选值。
    pub choices: Vec<String>,
}

impl DriverPropertyInfo {
    /// 创建 JDBC property descriptor，并保留 Java `DriverPropertyInfo` 的默认值语义。
    pub fn new(name: impl Into<String>, value: Option<String>) -> Self {
        Self {
            name: name.into(),
            value,
            description: None,
            required: false,
            choices: Vec::new(),
        }
    }
}

/// JDBC `Driver` 的 URL negotiation 标准。
#[async_trait::async_trait]
pub trait Driver: Send + Sync + 'static {
    /// 返回 registry 与 diagnostics 使用的稳定 driver 名称。
    fn name(&self) -> &str;

    /// 判断是否声明接受 URL；该入口用于驱动查找，不作为连接尝试的前置过滤。
    /// 对应 Java：`Driver#acceptsURL`。
    fn accepts_url(&self, url: &str) -> bool;

    /// 尝试建立连接；不理解 URL 时返回 `Ok(None)`。
    ///
    /// 注册表按顺序调用连接入口，不先调用 [`Self::accepts_url`]；SQL 异常允许尝试后续驱动，
    /// 其他 RDBC 错误立即返回。对应 Java：`Driver#connect`。
    async fn connect(
        &self,
        url: &str,
        properties: &ConnectionProperties,
    ) -> Result<Option<Box<dyn Connection>>, RdbcError>;

    /// 返回 driver property descriptors。
    fn property_info(
        &self,
        _url: &str,
        _properties: &ConnectionProperties,
    ) -> Result<Vec<DriverPropertyInfo>, RdbcError> {
        Ok(Vec::new())
    }

    /// 返回 major version。
    fn major_version(&self) -> u32 {
        0
    }

    /// 返回 minor version。
    fn minor_version(&self) -> u32 {
        0
    }

    /// 返回驱动是否声明通过完整 RDBC API 一致性测试。
    ///
    /// 该声明不是 Druid 对数据库产品的认证证据；默认与 JDBC `Driver` 一致为 `false`。
    fn rdbc_compliant(&self) -> bool {
        false
    }

    /// 返回驱动日志父命名空间。对应 Java `Driver#getParentLogger`。
    fn parent_logger(&self) -> Result<&str, RdbcError> {
        Ok("druid::rdbc")
    }

    /// Java getter 命名入口，精确委托 [`Self::parent_logger`]。
    fn get_parent_logger(&self) -> Result<&str, RdbcError> {
        self.parent_logger()
    }
}
