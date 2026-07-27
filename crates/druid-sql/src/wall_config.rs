//! 对应 Java 类：com.alibaba.druid.wall.WallConfig
//!
//! Wall 配置，替代 Druid Java WallConfig 的 30+ 配置项。

/// Wall 配置。
#[derive(Debug, Clone)]
pub struct WallConfig {
    /// SELECT * 允许
    pub select_all_allow: bool,
    /// DELETE 允许
    pub delete_allow: bool,
    /// UPDATE 允许
    pub update_allow: bool,
    /// INSERT 允许
    pub insert_allow: bool,
    /// DROP TABLE 允许
    pub drop_table_allow: bool,
    /// ALTER TABLE 允许
    pub alter_table_allow: bool,
    /// CREATE TABLE 允许
    pub create_table_allow: bool,
    /// TRUNCATE 允许
    pub truncate_allow: bool,
    /// UPDATE 必须带 WHERE
    pub update_must_have_where: bool,
    /// DELETE 必须带 WHERE
    pub delete_must_have_where: bool,
    /// 多语句允许
    pub multi_statement_allow: bool,
    /// 注释允许
    pub comment_allow: bool,
    /// 表黑名单
    pub deny_tables: Vec<String>,
    /// 函数黑名单
    pub deny_functions: Vec<String>,
}

impl Default for WallConfig {
    fn default() -> Self {
        Self {
            select_all_allow: true,
            delete_allow: true,
            update_allow: true,
            insert_allow: true,
            drop_table_allow: false,      // 默认拒绝 DROP
            alter_table_allow: true,
            create_table_allow: true,
            truncate_allow: false,        // 默认拒绝 TRUNCATE
            update_must_have_where: true, // UPDATE 必须有 WHERE
            delete_must_have_where: true, // DELETE 必须有 WHERE
            multi_statement_allow: false,
            comment_allow: true,
            deny_tables: Vec::new(),
            deny_functions: Vec::new(),
        }
    }
}

/// WallConfig Builder。
pub struct WallConfigBuilder(WallConfig);

impl WallConfig {
    pub fn builder() -> WallConfigBuilder { WallConfigBuilder(WallConfig::default()) }
}

impl WallConfigBuilder {
    pub fn select_all_allow(mut self, v: bool) -> Self { self.0.select_all_allow = v; self }
    pub fn delete_allow(mut self, v: bool) -> Self { self.0.delete_allow = v; self }
    pub fn update_allow(mut self, v: bool) -> Self { self.0.update_allow = v; self }
    pub fn insert_allow(mut self, v: bool) -> Self { self.0.insert_allow = v; self }
    pub fn drop_table_allow(mut self, v: bool) -> Self { self.0.drop_table_allow = v; self }
    pub fn truncate_allow(mut self, v: bool) -> Self { self.0.truncate_allow = v; self }
    pub fn update_must_have_where(mut self, v: bool) -> Self { self.0.update_must_have_where = v; self }
    pub fn delete_must_have_where(mut self, v: bool) -> Self { self.0.delete_must_have_where = v; self }
    pub fn deny_table(mut self, t: impl Into<String>) -> Self { self.0.deny_tables.push(t.into()); self }
    pub fn deny_function(mut self, f: impl Into<String>) -> Self { self.0.deny_functions.push(f.into()); self }
    pub fn build(self) -> WallConfig { self.0 }
}
