//! 对应 Java 类：com.alibaba.druid.wall.WallConfig
//! 来源文件：core/src/main/java/com/alibaba/druid/wall/WallConfig.java
//!
//! Wall 配置，对齐 Druid Java WallConfig 的 30+ 规则。

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
        }
    }
}

pub struct WallConfigBuilder(WallConfig);

impl WallConfig {
    pub fn builder() -> WallConfigBuilder {
        WallConfigBuilder(WallConfig::default())
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
    pub fn build(self) -> WallConfig {
        self.0
    }
}
