//! 对应 Java 类：com.alibaba.druid.wall.WallConfig
//! 来源文件：core/src/main/java/com/alibaba/druid/wall/WallConfig.java
//!
//! Wall 配置，对齐 Druid Java `WallConfig` 的 `30+` 规则。

use super::{TenantCallBack, WallUpdateCheckHandler};
use indexmap::{IndexMap, IndexSet};
use parking_lot::RwLock;
use std::collections::BTreeSet;
use std::sync::Arc;

/// Wall 配置，每个 boolean 默认值与 Druid Java 一致。
///
/// 字段位按 Java `WallConfig` 一一保留（40+ 布尔位），不做拆分。
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone)]
pub struct WallConfig {
    pub none_base_statement_allow: bool,
    pub call_allow: bool,
    pub select_allow: bool,
    pub select_all_column_allow: bool,
    pub select_into_allow: bool,
    pub select_into_outfile_allow: bool,
    pub select_union_check: bool,
    pub select_minus_check: bool,
    pub select_except_check: bool,
    pub select_intersect_check: bool,
    pub insert_allow: bool,
    pub update_allow: bool,
    pub delete_allow: bool,
    pub drop_table_allow: bool,
    pub truncate_allow: bool,
    pub alter_table_allow: bool,
    pub create_table_allow: bool,
    pub rename_table_allow: bool,
    pub lock_table_allow: bool,
    pub block_allow: bool,
    pub commit_allow: bool,
    pub rollback_allow: bool,
    pub use_allow: bool,
    pub show_allow: bool,
    pub describe_allow: bool,
    pub start_transaction_allow: bool,
    pub set_allow: bool,
    pub merge_allow: bool,
    pub minus_allow: bool,
    pub intersect_allow: bool,
    pub replace_allow: bool,
    /// Rust 兼容字段；精确对应 Java `updateWhereNoneCheck`。
    pub update_must_have_where: bool,
    /// Rust 兼容字段；精确对应 Java `deleteWhereNoneCheck`。
    pub delete_must_have_where: bool,
    pub select_where_alway_true_check: bool,
    pub select_having_alway_true_check: bool,
    pub update_where_alway_true_check: bool,
    pub delete_where_alway_true_check: bool,
    pub condition_and_alway_true_allow: bool,
    pub condition_and_alway_false_allow: bool,
    pub condition_double_const_allow: bool,
    pub condition_like_true_allow: bool,
    pub condition_op_xor_allow: bool,
    pub condition_op_bitwise_allow: bool,
    pub case_condition_const_allow: bool,
    pub multi_statement_allow: bool,
    pub hint_allow: bool,
    pub limit_zero_allow: bool,
    pub comment_allow: bool,
    pub strict_syntax_check: bool,
    pub const_arithmetic_allow: bool,
    pub schema_check: bool,
    pub table_check: bool,
    pub function_check: bool,
    pub object_check: bool,
    pub variant_check: bool,
    pub must_parameterized: bool,
    pub do_privileged_allow: bool,
    pub metadata_allow: bool,
    pub wrap_allow: bool,
    pub complete_insert_values_check: bool,
    pub insert_values_check_size: i32,
    pub select_limit: i32,
    pub dir: Option<String>,
    pub inited: bool,
    pub deny_tables: Vec<String>,
    pub deny_functions: Vec<String>,
    pub deny_schemas: Vec<String>,
    pub deny_variants: Vec<String>,
    pub deny_objects: Vec<String>,
    pub permit_tables: Vec<String>,
    pub permit_functions: Vec<String>,
    pub permit_schemas: Vec<String>,
    pub permit_variants: Vec<String>,
    pub read_only_tables: Vec<String>,
    pub select_white_list: bool,
    pub function_white_list: bool,
    pub schema_white_list: bool,
    pub tenant_column: String,
    pub tenant_table_pattern: String,
    tenant_call_back: Arc<RwLock<Option<Arc<dyn TenantCallBack>>>>,
    update_check_columns: Arc<RwLock<IndexMap<String, IndexSet<String>>>>,
    update_check_handler: Arc<RwLock<Option<Arc<dyn WallUpdateCheckHandler>>>>,
}

impl Default for WallConfig {
    fn default() -> Self {
        Self {
            none_base_statement_allow: false,
            call_allow: true,
            select_allow: true,
            select_all_column_allow: true,
            select_into_allow: true,
            select_into_outfile_allow: false,
            select_union_check: true,
            select_minus_check: true,
            select_except_check: true,
            select_intersect_check: true,
            insert_allow: true,
            update_allow: true,
            delete_allow: true,
            drop_table_allow: true,
            truncate_allow: true,
            alter_table_allow: true,
            create_table_allow: true,
            rename_table_allow: true,
            lock_table_allow: true,
            block_allow: true,
            commit_allow: true,
            rollback_allow: true,
            use_allow: true,
            show_allow: true,
            describe_allow: true,
            start_transaction_allow: true,
            set_allow: true,
            merge_allow: true,
            minus_allow: true,
            intersect_allow: true,
            replace_allow: true,
            update_must_have_where: false,
            delete_must_have_where: false,
            select_where_alway_true_check: true,
            select_having_alway_true_check: true,
            update_where_alway_true_check: true,
            delete_where_alway_true_check: true,
            condition_and_alway_true_allow: true,
            condition_and_alway_false_allow: false,
            condition_double_const_allow: false,
            condition_like_true_allow: true,
            condition_op_xor_allow: false,
            condition_op_bitwise_allow: true,
            case_condition_const_allow: false,
            multi_statement_allow: false,
            hint_allow: true,
            limit_zero_allow: false,
            comment_allow: false,
            strict_syntax_check: true,
            const_arithmetic_allow: true,
            schema_check: true,
            table_check: true,
            function_check: true,
            object_check: true,
            variant_check: true,
            must_parameterized: false,
            do_privileged_allow: false,
            metadata_allow: true,
            wrap_allow: true,
            complete_insert_values_check: false,
            insert_values_check_size: 3,
            select_limit: -1,
            dir: None,
            inited: false,
            deny_tables: Vec::new(),
            deny_functions: Vec::new(),
            deny_schemas: Vec::new(),
            deny_variants: Vec::new(),
            deny_objects: Vec::new(),
            permit_tables: Vec::new(),
            permit_functions: Vec::new(),
            permit_schemas: Vec::new(),
            permit_variants: Vec::new(),
            read_only_tables: Vec::new(),
            select_white_list: false,
            function_white_list: false,
            schema_white_list: false,
            tenant_column: String::new(),
            tenant_table_pattern: String::new(),
            tenant_call_back: Arc::new(RwLock::new(None)),
            update_check_columns: Arc::new(RwLock::new(IndexMap::new())),
            update_check_handler: Arc::new(RwLock::new(None)),
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

    /// 使用 Java Druid 内置规则目录创建配置。
    ///
    /// 对应 Java：`new WallConfig(String)`。目录末尾 `/` 会被移除，随后按
    /// Java 固定顺序合并 deny、readonly 与 permit 文件。Rust 没有
    /// ClassLoader；crate 自带资源就是 canonical classpath 基线。
    #[must_use]
    pub fn with_config_dir(dir: impl Into<String>) -> Self {
        let mut config = Self::default();
        config.load_config(dir);
        config
    }

    /// 重新加载指定内置规则目录。
    ///
    /// 与 Java 一样，本方法只向集合追加并去重，不先清空调用方已有规则。
    pub fn load_config(&mut self, dir: impl Into<String>) {
        let dir = dir.into().trim_end_matches('/').to_owned();
        self.dir = Some(dir.clone());
        load_resource(&mut self.deny_variants, &format!("{dir}/deny-variant.txt"));
        load_resource(&mut self.deny_schemas, &format!("{dir}/deny-schema.txt"));
        load_resource(
            &mut self.deny_functions,
            &format!("{dir}/deny-function.txt"),
        );
        load_resource(&mut self.deny_tables, &format!("{dir}/deny-table.txt"));
        load_resource(&mut self.deny_objects, &format!("{dir}/deny-object.txt"));
        load_resource(
            &mut self.read_only_tables,
            &format!("{dir}/readonly-table.txt"),
        );
        load_resource(
            &mut self.permit_functions,
            &format!("{dir}/permit-function.txt"),
        );
        load_resource(&mut self.permit_tables, &format!("{dir}/permit-table.txt"));
        load_resource(
            &mut self.permit_schemas,
            &format!("{dir}/permit-schema.txt"),
        );
        load_resource(
            &mut self.permit_variants,
            &format!("{dir}/permit-variant.txt"),
        );
        self.inited = true;
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
    #[must_use]
    pub fn select_allow(mut self, v: bool) -> Self {
        self.0.select_allow = v;
        self
    }
    #[must_use]
    pub fn insert_allow(mut self, v: bool) -> Self {
        self.0.insert_allow = v;
        self
    }
    #[must_use]
    pub fn update_allow(mut self, v: bool) -> Self {
        self.0.update_allow = v;
        self
    }
    #[must_use]
    pub fn delete_allow(mut self, v: bool) -> Self {
        self.0.delete_allow = v;
        self
    }
    #[must_use]
    pub fn drop_table_allow(mut self, v: bool) -> Self {
        self.0.drop_table_allow = v;
        self
    }
    #[must_use]
    pub fn truncate_allow(mut self, v: bool) -> Self {
        self.0.truncate_allow = v;
        self
    }
    #[must_use]
    pub fn alter_table_allow(mut self, v: bool) -> Self {
        self.0.alter_table_allow = v;
        self
    }
    #[must_use]
    pub fn update_must_have_where(mut self, v: bool) -> Self {
        self.0.update_must_have_where = v;
        self
    }
    #[must_use]
    pub fn delete_must_have_where(mut self, v: bool) -> Self {
        self.0.delete_must_have_where = v;
        self
    }
    #[must_use]
    pub fn multi_statement_allow(mut self, v: bool) -> Self {
        self.0.multi_statement_allow = v;
        self
    }
    #[must_use]
    pub fn comment_allow(mut self, v: bool) -> Self {
        self.0.comment_allow = v;
        self
    }
    #[must_use]
    pub fn variant_check(mut self, v: bool) -> Self {
        self.0.variant_check = v;
        self
    }
    #[must_use]
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
    /// 设置是否允许 `START TRANSACTION`（对应 Java `startTransactionAllow`）。
    #[must_use]
    pub fn start_transaction_allow(mut self, v: bool) -> Self {
        self.0.start_transaction_allow = v;
        self
    }
    /// 设置是否允许 `SELECT INTO`（对应 Java `selectIntoAllow`）。
    #[must_use]
    pub fn select_into_allow(mut self, v: bool) -> Self {
        self.0.select_into_allow = v;
        self
    }
    /// 设置是否允许 SQL hint（对应 Java `hintAllow`）。
    #[must_use]
    pub fn hint_allow(mut self, v: bool) -> Self {
        self.0.hint_allow = v;
        self
    }
    /// 设置是否允许 `SELECT *`（对应 Java `selectAllColumnAllow`）。
    #[must_use]
    pub fn select_all_column_allow(mut self, v: bool) -> Self {
        self.0.select_all_column_allow = v;
        self
    }
    /// 设置 SQL 是否必须参数化（对应 Java `mustParameterized`）。
    #[must_use]
    pub fn must_parameterized(mut self, v: bool) -> Self {
        self.0.must_parameterized = v;
        self
    }
    /// 设置是否允许 `USE` 语句（对应 Java `useAllow`）。
    #[must_use]
    pub fn use_allow(mut self, v: bool) -> Self {
        self.0.use_allow = v;
        self
    }
    /// 设置是否允许 `SHOW` 语句族（对应 Java `showAllow`）。
    #[must_use]
    pub fn show_allow(mut self, v: bool) -> Self {
        self.0.show_allow = v;
        self
    }
    /// 设置是否允许 `DESC`/`DESCRIBE` 语句（对应 Java `describeAllow`）。
    #[must_use]
    pub fn describe_allow(mut self, v: bool) -> Self {
        self.0.describe_allow = v;
        self
    }
    /// 设置是否允许 `CALL` 语句（对应 Java `callAllow`）。
    #[must_use]
    pub fn call_allow(mut self, v: bool) -> Self {
        self.0.call_allow = v;
        self
    }
    /// 设置是否允许 `INTERSECT` 集合运算（对应 Java `intersectAllow`）。
    #[must_use]
    pub fn intersect_allow(mut self, v: bool) -> Self {
        self.0.intersect_allow = v;
        self
    }
    /// 设置是否允许 AND 链非首位恒真条件（对应 Java `conditionAndAlwayTrueAllow`）。
    #[must_use]
    pub fn condition_and_alway_true_allow(mut self, v: bool) -> Self {
        self.0.condition_and_alway_true_allow = v;
        self
    }
    /// 设置是否允许 AND 链相邻双常量（对应 Java `conditionDoubleConstAllow`）。
    #[must_use]
    pub fn condition_double_const_allow(mut self, v: bool) -> Self {
        self.0.condition_double_const_allow = v;
        self
    }
    /// 设置是否允许 `CASE WHEN` 常量条件（对应 Java `caseConditionConstAllow`）。
    #[must_use]
    pub fn case_condition_const_allow(mut self, v: bool) -> Self {
        self.0.case_condition_const_allow = v;
        self
    }
    /// 设置是否允许条件中的常量算术（对应 Java `constArithmeticAllow`）。
    #[must_use]
    pub fn const_arithmetic_allow(mut self, v: bool) -> Self {
        self.0.const_arithmetic_allow = v;
        self
    }
    /// 设置是否允许条件中的位运算（对应 Java `conditionOpBitwiseAllow`）。
    #[must_use]
    pub fn condition_op_bitwise_allow(mut self, v: bool) -> Self {
        self.0.condition_op_bitwise_allow = v;
        self
    }
    /// 追加禁止的数据库变量（对应 Java `denyVariants` 集合元素）。
    #[must_use]
    pub fn deny_variant(mut self, variant: impl Into<String>) -> Self {
        insert_rule(&mut self.0.deny_variants, variant.into());
        self
    }
    /// 追加只读表（对应 Java `readOnlyTables` 集合元素）。
    #[must_use]
    pub fn read_only_table(mut self, table: impl Into<String>) -> Self {
        insert_rule(&mut self.0.read_only_tables, table.into());
        self
    }
    #[must_use]
    pub fn deny_table(mut self, t: impl Into<String>) -> Self {
        insert_rule(&mut self.0.deny_tables, t.into());
        self
    }
    #[must_use]
    pub fn deny_function(mut self, f: impl Into<String>) -> Self {
        insert_rule(&mut self.0.deny_functions, f.into());
        self
    }
    #[must_use]
    pub fn deny_schema(mut self, s: impl Into<String>) -> Self {
        insert_rule(&mut self.0.deny_schemas, s.into());
        self
    }
    #[must_use]
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

fn insert_rule(target: &mut Vec<String>, rule: impl AsRef<str>) {
    let rule = rule.as_ref().trim().to_lowercase();
    if !rule.is_empty() && !target.contains(&rule) {
        target.push(rule);
        target.sort_unstable();
    }
}

fn load_resource(target: &mut Vec<String>, resource: &str) {
    let Some(content) = bundled_resource(resource) else {
        return;
    };
    let mut names = target.iter().cloned().collect::<BTreeSet<_>>();
    names.extend(
        content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_lowercase),
    );
    *target = names.into_iter().collect();
}

fn bundled_resource(resource: &str) -> Option<&'static str> {
    const ROOT: &str = "META-INF/druid/wall/";
    let resource = resource.strip_prefix('/').unwrap_or(resource);
    let relative = resource.strip_prefix(ROOT)?;
    match relative {
        "clickhouse/deny-function.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/clickhouse/deny-function.txt"
        )),
        "clickhouse/deny-schema.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/clickhouse/deny-schema.txt"
        )),
        "mysql/deny-function.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/mysql/deny-function.txt"
        )),
        "mysql/deny-schema.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/mysql/deny-schema.txt"
        )),
        "mysql/deny-variant.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/mysql/deny-variant.txt"
        )),
        "mysql/permit-function.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/mysql/permit-function.txt"
        )),
        "mysql/permit-variant.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/mysql/permit-variant.txt"
        )),
        "oracle/deny-function.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/oracle/deny-function.txt"
        )),
        "oracle/deny-object.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/oracle/deny-object.txt"
        )),
        "oracle/deny-schema.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/oracle/deny-schema.txt"
        )),
        "oracle/deny-table.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/oracle/deny-table.txt"
        )),
        "oracle/deny-variant.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/oracle/deny-variant.txt"
        )),
        "oracle/permit-function.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/oracle/permit-function.txt"
        )),
        "postgres/deny-function.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/postgres/deny-function.txt"
        )),
        "postgres/deny-table.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/postgres/deny-table.txt"
        )),
        "sqlserver/deny-function.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/sqlserver/deny-function.txt"
        )),
        "sqlserver/deny-object.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/sqlserver/deny-object.txt"
        )),
        "sqlserver/deny-schema.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/sqlserver/deny-schema.txt"
        )),
        "sqlserver/deny-table.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/sqlserver/deny-table.txt"
        )),
        "sqlserver/deny-variant.txt" => Some(include_str!(
            "../../resources/META-INF/druid/wall/sqlserver/deny-variant.txt"
        )),
        _ => None,
    }
}
