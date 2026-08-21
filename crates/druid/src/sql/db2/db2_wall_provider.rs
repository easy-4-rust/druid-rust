//! 对应 Java：`com.alibaba.druid.wall.spi.DB2WallProvider`。

use crate::sql::{DbType, WallConfig, WallProvider};
use std::ops::Deref;

/// DB2 Wall Provider。
pub struct Db2WallProvider {
    provider: WallProvider,
}

impl Db2WallProvider {
    pub const DEFAULT_CONFIG_DIR: &'static str = "META-INF/druid/wall/db2";

    /// 使用 Java 默认目录创建 DB2 Provider。
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(WallConfig::with_config_dir(Self::DEFAULT_CONFIG_DIR))
    }

    /// 使用调用方配置创建 DB2 Provider。
    #[must_use]
    pub fn with_config(config: WallConfig) -> Self {
        let provider = WallProvider::new(config);
        provider.set_db_type(DbType::Db2);
        Self { provider }
    }

    /// 取出 canonical Provider。
    #[must_use]
    pub fn into_inner(self) -> WallProvider {
        self.provider
    }
}

impl Default for Db2WallProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for Db2WallProvider {
    type Target = WallProvider;

    fn deref(&self) -> &Self::Target {
        &self.provider
    }
}
