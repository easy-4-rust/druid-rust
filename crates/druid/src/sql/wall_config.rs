//! 对应 Java 类：com.alibaba.druid.wall.WallConfig
//! 来源文件：core/src/main/java/com/alibaba/druid/wall/WallConfig.java
//!
//! Wall 配置，对齐 Druid Java WallConfig 的 30+ 规则。

use super::{TenantCallBack, WallUpdateCheckHandler};
use indexmap::{IndexMap, IndexSet};
use parking_lot::RwLock;
use std::sync::Arc;

/// Wall 配置，每个 boolean 默认值与 Druid Java 一致。
pub struct WallConfig {
    pub select_allow: bool,
    pub select_all_column_allow: bool,
    pub select_into_allow: bool,
    pub insert_allow: bool,
    pub update_allow: bool,
    pub delete_allow: bool,
    pub drop_table_allow: bool,
    pub truncate_allow: bool,
    pub alter_table_allow: bool,
    pub create_table_allow: bool,
    pub commit_allow: bool,
    pub rollback_allow: bool,
    pub use_allow: bool,
    pub show_allow: bool,
    pub describe_allow: bool,
    pub start_transaction_allow: bool,
    pub set_allow: bool,
    pub update_must_have_where: bool,
    pub delete_must_have_where: bool,
    pub select_where_alway_true_check: bool,
    pub select_having_alway_true_check: bool,
    pub update_where_alway_true_check: bool,
    pub delete_where_alway_true_check: bool,
    pub condition_and_alway_true_allow: bool,
    pub condition_and_alway_false_allow: bool,
    pub condition_double_const_allow: bool,
    pub condition_like_true_allow: bool,
    pub case_condition_const_allow: bool,
    pub multi_statement_allow: bool,
    pub hint_allow: bool,
    pub none_base_statement_allow: bool,
    pub limit_zero_allow: bool,
    pub comment_allow: bool,
    pub variant_check: bool,
    pub must_parameterized: bool,
    pub do_privileged_allow: bool,
    pub metadata_allow: bool,
    pub wrap_allow: bool,
    pub deny_tables: Vec<String>,
    pub deny_functions: Vec<String>,
    pub deny_schemas: Vec<String>,
    pub deny_variants: Vec<String>,
    pub select_white_list: bool,
    pub function_white_list: bool,
    pub schema_white_list: bool,
    pub tenant_column: String,
    pub tenant_table_pattern: String,
    tenant_call_back: RwLock<Option<Arc<dyn TenantCallBack>>>,
    update_check_columns: RwLock<IndexMap<String, IndexSet<String>>>,
    update_check_handler: RwLock<Option<Arc<dyn WallUpdateCheckHandler>>>,
}

impl Default for WallConfig {
    fn default() -> Self {
        Self {
            select_allow: true,
            select_all_column_allow: true,
            select_into_allow: true,
            insert_allow: true,
            update_allow: true,
            delete_allow: true,
            drop_table_allow: false,
            truncate_allow: false,
            alter_table_allow: true,
            create_table_allow: true,
            commit_allow: true,
            rollback_allow: true,
            use_allow: true,
            show_allow: true,
            describe_allow: true,
            start_transaction_allow: true,
            set_allow: true,
            update_must_have_where: true,
            delete_must_have_where: true,
            select_where_alway_true_check: true,
            select_having_alway_true_check: true,
            update_where_alway_true_check: true,
            delete_where_alway_true_check: true,
            condition_and_alway_true_allow: false,
            condition_and_alway_false_allow: false,
            condition_double_const_allow: false,
            condition_like_true_allow: true,
            case_condition_const_allow: true,
            multi_statement_allow: false,
            hint_allow: true,
            none_base_statement_allow: true,
            limit_zero_allow: false,
            comment_allow: true,
            variant_check: true,
            must_parameterized: false,
            do_privileged_allow: false,
            metadata_allow: true,
            wrap_allow: true,
            deny_tables: Vec::new(),
            deny_functions: Vec::new(),
            deny_schemas: Vec::new(),
            deny_variants: Vec::new(),
            select_white_list: false,
            function_white_list: false,
            schema_white_list: false,
            tenant_column: String::new(),
            tenant_table_pattern: String::new(),
            tenant_call_back: RwLock::new(None),
            update_check_columns: RwLock::new(IndexMap::new()),
            update_check_handler: RwLock::new(None),
        }
    }
}

pub struct WallConfigBuilder(WallConfig);

impl WallConfig {
    /// 创建 builder。
    #[must_use]
    pub fn builder() -> WallConfigBuilder {
        WallConfigBuilder(WallConfig::default())
    }

    /// 增加 `table.column` UPDATE 检查配置。
    ///
    /// 对应 Java：`WallConfig#addUpdateCheckColumns(String)`。格式不是严格两个
    /// 片段时静默忽略；表名和列名去除一层 SQL 引号并转换为小写。
    pub fn add_update_check_columns(&self, column_info: &str) {
        let mut items = column_info.split('.');
        let Some(table) = items.next() else {
            return;
        };
        let Some(column) = items.next() else {
            return;
        };
        if items.next().is_some() {
            return;
        }
        let table = normalize_identifier(table);
        let column = normalize_identifier(column);
        self.update_check_columns
            .write()
            .entry(table)
            .or_default()
            .insert(column);
    }

    /// 返回表是否配置了 UPDATE 检查列。
    ///
    /// 对应 Java：`WallConfig#isUpdateCheckTable(String)`。
    #[must_use]
    pub fn is_update_check_table(&self, table_name: &str) -> bool {
        self.update_check_columns
            .read()
            .contains_key(&normalize_identifier(table_name))
    }

    /// 返回表的有序 UPDATE 检查列。
    ///
    /// 对应 Java：`WallConfig#getUpdateCheckTable(String)`；Java 使用
    /// `LinkedHashSet`，因此 Rust 使用 `IndexSet` 保留首列选择语义。
    #[must_use]
    pub fn update_check_table(&self, table_name: &str) -> Option<IndexSet<String>> {
        self.update_check_columns
            .read()
            .get(&normalize_identifier(table_name))
            .cloned()
    }

    /// 返回 UPDATE 检查器。
    ///
    /// 对应 Java：`WallConfig#getUpdateCheckHandler()`。
    #[must_use]
    pub fn update_check_handler(&self) -> Option<Arc<dyn WallUpdateCheckHandler>> {
        self.update_check_handler.read().clone()
    }

    /// 设置 UPDATE 检查器。
    ///
    /// 对应 Java：`WallConfig#setUpdateCheckHandler(WallUpdateCheckHandler)`。
    pub fn set_update_check_handler(
        &self,
        update_check_handler: Option<Arc<dyn WallUpdateCheckHandler>>,
    ) {
        *self.update_check_handler.write() = update_check_handler;
    }

    /// 返回多租户回调。
    ///
    /// 对应 Java：`WallConfig#getTenantCallBack()`。
    #[must_use]
    pub fn tenant_call_back(&self) -> Option<Arc<dyn TenantCallBack>> {
        self.tenant_call_back.read().clone()
    }

    /// 设置多租户回调。
    ///
    /// 对应 Java：`WallConfig#setTenantCallBack(TenantCallBack)`。
    pub fn set_tenant_call_back(&self, tenant_call_back: Option<Arc<dyn TenantCallBack>>) {
        *self.tenant_call_back.write() = tenant_call_back;
    }
}

impl WallConfigBuilder {
    pub fn select_allow(mut self, v: bool) -> Self {
        self.0.select_allow = v;
        self
    }
    pub fn insert_allow(mut self, v: bool) -> Self {
        self.0.insert_allow = v;
        self
    }
    pub fn update_allow(mut self, v: bool) -> Self {
        self.0.update_allow = v;
        self
    }
    pub fn delete_allow(mut self, v: bool) -> Self {
        self.0.delete_allow = v;
        self
    }
    pub fn drop_table_allow(mut self, v: bool) -> Self {
        self.0.drop_table_allow = v;
        self
    }
    pub fn truncate_allow(mut self, v: bool) -> Self {
        self.0.truncate_allow = v;
        self
    }
    pub fn update_must_have_where(mut self, v: bool) -> Self {
        self.0.update_must_have_where = v;
        self
    }
    pub fn delete_must_have_where(mut self, v: bool) -> Self {
        self.0.delete_must_have_where = v;
        self
    }
    pub fn multi_statement_allow(mut self, v: bool) -> Self {
        self.0.multi_statement_allow = v;
        self
    }
    pub fn comment_allow(mut self, v: bool) -> Self {
        self.0.comment_allow = v;
        self
    }
    pub fn variant_check(mut self, v: bool) -> Self {
        self.0.variant_check = v;
        self
    }
    pub fn limit_zero_allow(mut self, v: bool) -> Self {
        self.0.limit_zero_allow = v;
        self
    }
    /// 设置是否允许 `WallProvider::do_privileged` 绕过 Wall 检查。
    #[must_use]
    pub fn do_privileged_allow(mut self, v: bool) -> Self {
        self.0.do_privileged_allow = v;
        self
    }
    pub fn deny_table(mut self, t: impl Into<String>) -> Self {
        self.0.deny_tables.push(t.into());
        self
    }
    pub fn deny_function(mut self, f: impl Into<String>) -> Self {
        self.0.deny_functions.push(f.into());
        self
    }
    pub fn deny_schema(mut self, s: impl Into<String>) -> Self {
        self.0.deny_schemas.push(s.into());
        self
    }
    pub fn tenant_column(mut self, v: impl Into<String>) -> Self {
        self.0.tenant_column = v.into();
        self
    }
    /// 设置租户表 Servlet 风格匹配表达式。
    #[must_use]
    pub fn tenant_table_pattern(mut self, v: impl Into<String>) -> Self {
        self.0.tenant_table_pattern = v.into();
        self
    }
    /// 设置多租户回调。
    #[must_use]
    pub fn tenant_call_back(self, tenant_call_back: Arc<dyn TenantCallBack>) -> Self {
        self.0.set_tenant_call_back(Some(tenant_call_back));
        self
    }
    /// 增加 `table.column` UPDATE 检查配置。
    #[must_use]
    pub fn update_check_column(self, column_info: impl AsRef<str>) -> Self {
        self.0.add_update_check_columns(column_info.as_ref());
        self
    }
    /// 设置 UPDATE 检查器。
    #[must_use]
    pub fn update_check_handler(
        self,
        update_check_handler: Arc<dyn WallUpdateCheckHandler>,
    ) -> Self {
        self.0.set_update_check_handler(Some(update_check_handler));
        self
    }
    /// 完成配置构造。
    #[must_use]
    pub fn build(self) -> WallConfig {
        self.0
    }
}

fn normalize_identifier(identifier: &str) -> String {
    identifier
        .trim()
        .trim_matches(['`', '"', '[', ']'])
        .to_lowercase()
}
