//! 对应 Java 类：java.sql.Driver

use super::error::DruidError;
use super::physical_connection::PhysicalConnection;
use std::collections::HashMap;

/// RDBC 物理驱动协议，对应 Java: `java.sql.Driver`。
///
/// `DriverManager` 按注册顺序调用 `accepts_url`，再由首个接受 URL 的驱动建立
/// 物理连接。驱动还应报告配置属性、版本、RDBC 合规性与父 logger；凭据不得进入
/// Debug、日志或错误文本。
#[async_trait::async_trait]
pub trait Driver: Send + Sync {
    /// 返回用于诊断和池状态展示的稳定驱动名称。
    fn name(&self) -> &str;
    /// 判断驱动是否理解 `url`；不得因此建立连接。
    ///
    /// 对应 Java: `Driver#acceptsURL`。格式非法或协议不匹配时返回 `false`。
    fn accepts_url(&self, _url: &str) -> bool {
        true
    }
    /// 返回为指定 URL 建连可能需要的属性说明。
    ///
    /// `info` 包含调用方已提供的属性；返回项可指出必填项和候选值，但不得发起实际
    /// 建连。对应 Java: `Driver#getPropertyInfo`。
    fn property_info(&self, _url: &str, _info: &HashMap<String, String>) -> Vec<DriverProperty> {
        Vec::new()
    }
    /// 对应 Java `getPropertyInfo` 的标准命名入口。
    fn get_property_info(&self, url: &str, info: &HashMap<String, String>) -> Vec<DriverProperty> {
        self.property_info(url, info)
    }
    /// 返回驱动主版本号；对应 Java `getMajorVersion`。
    fn major_version(&self) -> i32 {
        0
    }
    /// 对应 Java `getMajorVersion`。
    fn get_major_version(&self) -> i32 {
        self.major_version()
    }
    /// 返回驱动次版本号；对应 Java `getMinorVersion`。
    fn minor_version(&self) -> i32 {
        0
    }
    /// 对应 Java `getMinorVersion`。
    fn get_minor_version(&self) -> i32 {
        self.minor_version()
    }
    /// 返回驱动是否声明通过完整 RDBC API 一致性测试。
    ///
    /// 该声明不是 Druid 对数据库产品的认证证据。对应 Java `rdbcCompliant`。
    fn rdbc_compliant(&self) -> bool {
        false
    }
    /// 返回驱动日志父命名空间。对应 Java: `Driver#getParentLogger`。
    fn parent_logger(&self) -> Result<&str, DruidError> {
        Ok("druid::rdbc")
    }
    /// 对应 Java `getParentLogger`。
    fn get_parent_logger(&self) -> Result<&str, DruidError> {
        self.parent_logger()
    }
    /// 使用已被当前驱动接受的 `url` 建立未池化物理连接。
    ///
    /// 返回连接或数据库访问错误；调用方应先执行 `accepts_url`。
    async fn connect(&self, url: &str) -> Result<Box<dyn PhysicalConnection>, DruidError>;
    /// 使用 URL 与本次调用的用户名、密码建立未池化物理连接。
    ///
    /// 默认实现忽略显式凭据并转发到 `connect`；需要认证的驱动必须覆盖此方法。
    async fn connect_with_auth(
        &self,
        url: &str,
        username: &str,
        password: &str,
    ) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        let _ = (username, password);
        self.connect(url).await
    }

    /// 使用属性集建立未池化物理连接。
    ///
    /// 默认实现识别 `user` 和 `password`，其余属性由具体驱动覆盖处理。对应 Java:
    /// `Driver#connect(String, Properties)`。
    async fn connect_with_properties(
        &self,
        url: &str,
        info: &HashMap<String, String>,
    ) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        match (info.get("user"), info.get("password")) {
            (Some(username), Some(password)) => {
                self.connect_with_auth(url, username, password).await
            }
            _ => self.connect(url).await,
        }
    }
}

/// 不依赖 RDBC 门面的驱动连接属性说明。
///
/// 对应 Java: `java.sql.DriverPropertyInfo`，供配置工具发现必填项、说明和候选值。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriverProperty {
    /// 属性名。
    pub name: String,
    /// 当前值；未提供时为 `None`。
    pub value: Option<String>,
    /// 面向使用者的属性说明。
    pub description: Option<String>,
    /// 建连前是否必须提供该属性。
    pub required: bool,
    /// 可选值；空集合表示驱动未限制或无法枚举。
    pub choices: Vec<String>,
}

impl DriverProperty {
    /// 创建驱动属性。对应 Java: `DriverPropertyInfo(String, String)`。
    #[must_use]
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
