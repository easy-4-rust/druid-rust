//! 对应 Java：`com.alibaba.druid.wall.spi.CKWallProvider`。

use crate::sql::{DbType, WallConfig, WallProvider};
use std::ops::Deref;

/// ClickHouse Wall Provider。
pub struct CkWallProvider {
    provider: WallProvider,
}

impl CkWallProvider {
    pub const DEFAULT_CONFIG_DIR: &'static str = "META-INF/druid/wall/clickhouse";

    /// 使用 Java 默认规则创建 ClickHouse Provider。
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(WallConfig::with_config_dir(Self::DEFAULT_CONFIG_DIR))
    }

    /// 使用调用方配置创建 ClickHouse Provider。
    #[must_use]
    pub fn with_config(config: WallConfig) -> Self {
        let provider = WallProvider::new(config);
        provider.set_db_type(DbType::ClickHouse);
        Self { provider }
    }

    /// 取出 canonical Provider。
    #[must_use]
    pub fn into_inner(self) -> WallProvider {
        self.provider
    }
}

impl Default for CkWallProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for CkWallProvider {
    type Target = WallProvider;

    fn deref(&self) -> &Self::Target {
        &self.provider
    }
}
