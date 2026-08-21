//! 对应 Java 类：`com.alibaba.druid.pool.ha.node.ZookeeperNodeInfo`。

/// 写入 `ZooKeeper` 临时节点的单个数据库端点信息。
///
/// 对应 Java: `com.alibaba.druid.pool.ha.node.ZookeeperNodeInfo`。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZookeeperNodeInfo {
    prefix: String,
    host: Option<String>,
    port: Option<u16>,
    database: Option<String>,
    username: Option<String>,
    password: Option<String>,
}

impl ZookeeperNodeInfo {
    /// 创建空节点信息。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置属性前缀；非空前缀自动补末尾点号。
    pub fn set_prefix(&mut self, prefix: Option<&str>) {
        let Some(prefix) = prefix.filter(|prefix| !prefix.trim().is_empty()) else {
            return;
        };
        self.prefix = if prefix.ends_with('.') {
            prefix.to_owned()
        } else {
            format!("{prefix}.")
        };
    }

    /// 返回规范化前缀。
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// 设置主机名。
    pub fn set_host(&mut self, host: Option<String>) {
        self.host = host;
    }

    /// 返回主机名。
    #[must_use]
    pub fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }

    /// 设置端口。
    pub fn set_port(&mut self, port: Option<u16>) {
        self.port = port;
    }

    /// 返回端口。
    #[must_use]
    pub const fn port(&self) -> Option<u16> {
        self.port
    }

    /// 设置数据库名、Oracle `ServiceName` 或 SID。
    pub fn set_database(&mut self, database: Option<String>) {
        self.database = database;
    }

    /// 返回数据库标识。
    #[must_use]
    pub fn database(&self) -> Option<&str> {
        self.database.as_deref()
    }

    /// 设置用户名。
    pub fn set_username(&mut self, username: Option<String>) {
        self.username = username;
    }

    /// 返回用户名。
    #[must_use]
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    /// 设置密码。
    pub fn set_password(&mut self, password: Option<String>) {
        self.password = password;
    }

    /// 返回密码；调用方不得写入日志。
    #[must_use]
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }
}
