//! 对应 Java：`com.alibaba.druid.wall.spi.SQLiteWallProvider`。

use crate::sql::{DbType, WallConfig, WallProvider};
use std::ops::Deref;

/// `SQLite` Wall Provider。
///
/// Java 使用 `MySQL` parser/export visitor 承载 SQLite，再使用独立
/// `SQLiteWallVisitor` 收紧方言规则；Rust 固定 `SQLite` parser 方言。
pub struct SQLiteWallProvider {
    provider: WallProvider,
}

impl SQLiteWallProvider {
    pub const DEFAULT_CONFIG_DIR: &'static str = "META-INF/druid/wall/sqlite";

    /// 使用 Java 默认目录创建 `SQLite` Provider。
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(WallConfig::with_config_dir(Self::DEFAULT_CONFIG_DIR))
    }

    /// 使用调用方配置创建 `SQLite` Provider。
    #[must_use]
    pub fn with_config(config: WallConfig) -> Self {
        let provider = WallProvider::new(config);
        provider.set_db_type(DbType::SQLite);
        Self { provider }
    }

    /// 取出 canonical Provider。
    #[must_use]
    pub fn into_inner(self) -> WallProvider {
        self.provider
    }
}

impl Default for SQLiteWallProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for SQLiteWallProvider {
    type Target = WallProvider;

    fn deref(&self) -> &Self::Target {
        &self.provider
    }
}
