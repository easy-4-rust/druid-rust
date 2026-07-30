//! 对应 Java：`com.alibaba.druid.wall.spi.MySqlWallProvider`。

use crate::sql::{DbType, WallConfig, WallProvider};
use std::ops::Deref;

/// MySQL 及兼容协议的 Wall Provider。
///
/// 默认加载 Java Druid 的 MySQL deny/permit 规则，并固定使用 MySQL 方言。
pub struct MySqlWallProvider {
    provider: WallProvider,
}

impl MySqlWallProvider {
    pub const DEFAULT_CONFIG_DIR: &'static str = "META-INF/druid/wall/mysql";

    /// 使用 Java 默认规则创建 MySQL Provider。
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(WallConfig::with_config_dir(Self::DEFAULT_CONFIG_DIR))
    }

    /// 使用调用方配置创建 MySQL Provider。
    #[must_use]
    pub fn with_config(config: WallConfig) -> Self {
        let provider = WallProvider::new(config);
        provider.set_db_type(DbType::MySql);
        Self { provider }
    }

    /// 取出 canonical Provider。
    #[must_use]
    pub fn into_inner(self) -> WallProvider {
        self.provider
    }
}

impl Default for MySqlWallProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for MySqlWallProvider {
    type Target = WallProvider;

    fn deref(&self) -> &Self::Target {
        &self.provider
    }
}
