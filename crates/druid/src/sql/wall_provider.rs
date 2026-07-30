use super::{
    DbType, SqlUtils, Wall, WallCheckResult, WallConfig, WallContext, WallDenyStat,
    WallFunctionStat, WallFunctionStatValue, WallProviderStatValue, WallSqlFunctionStat,
    WallSqlStat, WallSqlStatValue, WallSqlTableStat, WallTableStat, WallTableStatValue,
    WallViolation, WallVisitorUtils,
};
use crate::core::{DruidError, Value};
use dashmap::DashMap;
use moka::sync::Cache;
use parking_lot::RwLock;
use sqlparser::ast::{
    visit_expressions, visit_relations, Expr, ObjectName, ObjectType, Statement, TableFactor,
};
use sqlparser::parser::Parser;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::future::Future;
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

thread_local! {
    static TENANT_VALUE_THREAD: RefCell<Option<Value>> = const { RefCell::new(None) };
    static PRIVILEGED_THREAD: Cell<bool> = const { Cell::new(false) };
}

tokio::task_local! {
    static TENANT_VALUE_TASK: Value;
    static PRIVILEGED_TASK: bool;
}

const WHITE_SQL_MAX_SIZE: u64 = 1024;
const BLACK_SQL_MAX_SIZE: u64 = 256;

struct WallContextCleanup(bool);

impl Drop for WallContextCleanup {
    fn drop(&mut self) {
        if self.0 {
            WallContext::clear_context();
        }
    }
}

struct PrivilegedThreadReset<'a> {
    slot: &'a Cell<bool>,
    original: bool,
}

impl Drop for PrivilegedThreadReset<'_> {
    fn drop(&mut self) {
        self.slot.set(self.original);
    }
}

/// SQL Wall canonical provider。
///
/// 对应 Java：`com.alibaba.druid.wall.WallProvider`。白/黑名单采用有界并发
/// LRU cache；检查计数、命中计数、语法错误和违规计数保持原子更新语义。
pub struct WallProvider {
    name: RwLock<Option<String>>,
    wall: Wall,
    white_list: Cache<String, Arc<WallSqlStat>>,
    black_list: Cache<String, Arc<WallSqlStat>>,
    table_stats: DashMap<String, Arc<WallTableStat>>,
    function_stats: DashMap<String, Arc<WallFunctionStat>>,
    check_count: AtomicU64,
    hard_check_count: AtomicU64,
    white_list_hit_count: AtomicU64,
    black_list_hit_count: AtomicU64,
    syntax_error_count: AtomicU64,
    violation_count: AtomicU64,
    violation_effect_row_count: AtomicU64,
    /// 注释拒绝统计；保留 Java 公共对象语义。
    pub comment_denied_stat: WallDenyStat,
}

impl WallProvider {
    /// 使用指定规则创建 provider。
    #[must_use]
    pub fn new(config: WallConfig) -> Self {
        Self {
            name: RwLock::new(None),
            wall: Wall::new(config),
            white_list: Cache::new(WHITE_SQL_MAX_SIZE),
            black_list: Cache::new(BLACK_SQL_MAX_SIZE),
            table_stats: DashMap::new(),
            function_stats: DashMap::new(),
            check_count: AtomicU64::new(0),
            hard_check_count: AtomicU64::new(0),
            white_list_hit_count: AtomicU64::new(0),
            black_list_hit_count: AtomicU64::new(0),
            syntax_error_count: AtomicU64::new(0),
            violation_count: AtomicU64::new(0),
            violation_effect_row_count: AtomicU64::new(0),
            comment_denied_stat: WallDenyStat::default(),
        }
    }

    /// 返回 provider 名称。
    #[must_use]
    pub fn name(&self) -> Option<String> {
        self.name.read().clone()
    }

    /// 设置 provider 名称。
    pub fn set_name(&self, name: Option<String>) {
        *self.name.write() = name;
    }

    /// 返回 Wall 配置。
    #[must_use]
    pub fn config(&self) -> &WallConfig {
        self.wall.config()
    }

    /// 设置当前同步线程的租户值。
    ///
    /// 对应 Java：`WallProvider#setTenantValue(Object)`。在 Tokio 异步任务中应
    /// 优先使用 [`Self::scope_tenant_value`]，避免任务跨 worker 后丢失上下文。
    pub fn set_tenant_value(value: Option<Value>) {
        TENANT_VALUE_THREAD.with(|tenant_value| {
            *tenant_value.borrow_mut() = value;
        });
    }

    /// 返回当前任务租户值；未设置任务值时回退到 Java 兼容线程值。
    ///
    /// 对应 Java：`WallProvider#getTenantValue()`。
    #[must_use]
    pub fn tenant_value() -> Option<Value> {
        TENANT_VALUE_TASK
            .try_with(Clone::clone)
            .ok()
            .or_else(|| TENANT_VALUE_THREAD.with(|tenant_value| tenant_value.borrow().clone()))
    }

    /// 在指定 Future 生命周期内绑定可跨 Tokio worker 迁移的租户值。
    ///
    /// 这是 Java `ThreadLocal` 在 Rust async 运行时中的语义适配；作用域退出后自动
    /// 恢复外层值，不污染线程池中的后续任务。
    pub async fn scope_tenant_value<F>(value: Value, future: F) -> F::Output
    where
        F: Future,
    {
        TENANT_VALUE_TASK.scope(value, future).await
    }

    /// 返回当前同步线程或异步任务是否处于 privileged 作用域。
    ///
    /// 对应 Java：`WallProvider#ispPrivileged()`。
    #[must_use]
    pub fn is_privileged() -> bool {
        PRIVILEGED_TASK
            .try_with(|privileged| *privileged)
            .unwrap_or_else(|_| PRIVILEGED_THREAD.with(Cell::get))
    }

    /// 在同步闭包期间启用 privileged，并在正常返回或 panic 时恢复外层值。
    ///
    /// 对应 Java：`WallProvider#doPrivileged(PrivilegedAction)`。
    pub fn do_privileged<T>(action: impl FnOnce() -> T) -> T {
        PRIVILEGED_THREAD.with(|slot| {
            let original = slot.replace(true);
            let _reset = PrivilegedThreadReset { slot, original };
            action()
        })
    }

    /// 在 Future 生命周期内启用可跨 Tokio worker 传播的 privileged。
    ///
    /// 作用域退出或 Future 被取消时由 Tokio task-local 自动恢复外层值。
    pub async fn scope_privileged<F>(future: F) -> F::Output
    where
        F: Future,
    {
        PRIVILEGED_TASK.scope(true, future).await
    }

    /// 设置 SQL 解析方言。
    pub fn set_db_type(&self, db_type: DbType) {
        self.wall.set_db_type(db_type);
    }

    /// 返回 SQL 解析方言。
    #[must_use]
    pub fn db_type(&self) -> DbType {
        self.wall.db_type()
    }

    /// 检查 SQL 并返回共享统计对象和结构化违规。
    #[must_use]
    pub fn check(&self, sql: &str) -> WallCheckResult {
        self.try_check(sql)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    /// 检查并按 Wall Visitor 规则改写 SQL。
    ///
    /// 与 Java 可能抛出 `IllegalStateException` 的 `check` 相比，本入口把不支持
    /// 的 tenant value 类型映射为显式 `DruidError`，供 Filter 调用链传播。
    pub fn try_check(&self, sql: &str) -> Result<WallCheckResult, DruidError> {
        self.try_check_with_tenant_rewrite(sql, true)
    }

    /// 检查已在 prepare 边界完成 tenant 改写的 SQL，不重复追加租户列。
    pub(crate) fn try_check_without_tenant_rewrite(
        &self,
        sql: &str,
    ) -> Result<WallCheckResult, DruidError> {
        self.try_check_with_tenant_rewrite(sql, false)
    }

    fn try_check_with_tenant_rewrite(
        &self,
        sql: &str,
        rewrite_tenant: bool,
    ) -> Result<WallCheckResult, DruidError> {
        let clear_context = WallContext::current().is_none();
        let context = WallContext::create_if_not_exists(self.db_type());
        let _cleanup = WallContextCleanup(clear_context);
        let result = self.check_internal(sql, rewrite_tenant)?;
        if let Some(sql_stat) = result.sql_stat() {
            let mut context = context.lock();
            context.set_sql_stat(Some(Arc::clone(sql_stat)));
            context.replace_sql_stats(
                sql_stat.table_stats().clone(),
                sql_stat.function_stats().clone(),
            );
            context.set_wall_update_check_items(result.update_check_items().map(<[_]>::to_vec));
        } else {
            let mut context = context.lock();
            context.set_sql_stat(None);
            context.replace_sql_stats(HashMap::new(), HashMap::new());
            context.set_wall_update_check_items(None);
        }
        Ok(result)
    }

    fn check_internal(
        &self,
        sql: &str,
        rewrite_tenant: bool,
    ) -> Result<WallCheckResult, DruidError> {
        self.check_count.fetch_add(1, Ordering::Relaxed);
        if self.config().do_privileged_allow && Self::is_privileged() {
            return Ok(WallCheckResult::privileged(sql.to_owned()));
        }
        // Java 在 tenantTablePattern 非空时绕过白/黑名单，否则缓存命中会跳过
        // Visitor 改写并返回未追加 tenant 列的原 SQL。
        let cache_enabled = self.config().update_check_handler().is_none()
            && self.config().tenant_table_pattern.is_empty();
        if cache_enabled {
            if let Some(stat) = self.white_list.get(sql) {
                self.white_list_hit_count.fetch_add(1, Ordering::Relaxed);
                stat.increment_execute_count();
                if stat.is_syntax_error() {
                    self.syntax_error_count.fetch_add(1, Ordering::Relaxed);
                }
                self.record_stats(&stat);
                return Ok(WallCheckResult::new(
                    sql.to_owned(),
                    Vec::new(),
                    Vec::new(),
                    false,
                    stat,
                ));
            }
            if let Some(stat) = self.black_list.get(sql) {
                self.black_list_hit_count.fetch_add(1, Ordering::Relaxed);
                self.violation_count.fetch_add(1, Ordering::Relaxed);
                stat.increment_execute_count();
                if stat.is_syntax_error() {
                    self.syntax_error_count.fetch_add(1, Ordering::Relaxed);
                }
                self.record_stats(&stat);
                return Ok(WallCheckResult::new(
                    sql.to_owned(),
                    Vec::new(),
                    stat.violations().to_vec(),
                    stat.violations()
                        .iter()
                        .any(|violation| matches!(violation, WallViolation::SyntaxError(_))),
                    stat,
                ));
            }
        }

        self.hard_check_count.fetch_add(1, Ordering::Relaxed);
        let dialect = SqlUtils::dialect(self.db_type());
        let parsed = Parser::parse_sql(dialect.as_ref(), sql);
        let mut statements = parsed.clone().unwrap_or_default();
        let sql_modified = rewrite_tenant
            && WallVisitorUtils::rewrite_for_multi_tenant(&mut statements, self.config())?;
        let (violations, update_check_items) = match &parsed {
            Ok(statements) => self.wall.check_parsed(sql, statements),
            Err(error) => (vec![WallViolation::SyntaxError(error.to_string())], None),
        };
        let syntax_error = parsed.is_err()
            || violations
                .iter()
                .any(|violation| matches!(violation, WallViolation::SyntaxError(_)));
        let (table_stats, function_stats) = collect_sql_stats(&statements);
        let stat = Arc::new(WallSqlStat::new_with_stats(
            sql.to_owned(),
            violations.clone(),
            syntax_error,
            table_stats,
            function_stats,
        ));
        stat.increment_execute_count();
        self.record_stats(&stat);
        if cache_enabled && violations.is_empty() {
            self.white_list.insert(sql.to_owned(), Arc::clone(&stat));
        } else {
            if !violations.is_empty() {
                self.violation_count.fetch_add(1, Ordering::Relaxed);
                if syntax_error {
                    self.syntax_error_count.fetch_add(1, Ordering::Relaxed);
                }
            }
            if cache_enabled {
                self.black_list.insert(sql.to_owned(), Arc::clone(&stat));
            }
        }
        let result_sql = if sql_modified {
            SqlUtils::to_sql_string(&statements)
        } else {
            sql.to_owned()
        };
        let mut result =
            WallCheckResult::new(result_sql, statements, violations, syntax_error, stat);
        result.set_update_check_items(update_check_items);
        Ok(result)
    }

    /// 查询白/黑名单中的 SQL 统计。
    #[must_use]
    pub fn sql_stat(&self, sql: &str) -> Option<Arc<WallSqlStat>> {
        self.white_list
            .get(sql)
            .or_else(|| self.black_list.get(sql))
    }

    /// 累加违规 SQL 实际影响行数。
    pub fn add_violation_effect_row_count(&self, delta: u64) {
        self.violation_effect_row_count
            .fetch_add(delta, Ordering::Relaxed);
    }

    /// 将执行结果的行数写回 SQL 涉及的表。
    pub fn record_effect_rows(&self, sql: &str, rows_affected: u64, row_count: Option<u64>) {
        let Some(sql_stat) = self.sql_stat(sql) else {
            return;
        };
        self.record_effect_rows_for_stat(&sql_stat, rows_affected, row_count);
    }

    pub(crate) fn record_effect_rows_for_stat(
        &self,
        sql_stat: &WallSqlStat,
        rows_affected: u64,
        row_count: Option<u64>,
    ) {
        for (name, operation) in sql_stat.table_stats() {
            let Some(table_stat) = self.table_stats.get(name) else {
                continue;
            };
            if operation.insert_count > 0 {
                table_stat.add_insert_data_count(rows_affected);
            }
            if operation.update_count > 0 {
                table_stat.add_update_data_count(rows_affected);
            }
            if operation.delete_count > 0 {
                table_stat.add_delete_data_count(rows_affected);
            }
            if operation.select_count > 0 {
                table_stat.add_fetch_row_count(row_count.unwrap_or_default());
            }
        }
    }

    /// 返回表统计快照。
    #[must_use]
    pub fn table_stat_values(&self, reset: bool) -> Vec<WallTableStatValue> {
        let mut values = self
            .table_stats
            .iter()
            .map(|entry| entry.value().stat_value(entry.key().clone(), reset))
            .collect::<Vec<_>>();
        values.sort_by(|left, right| left.name.cmp(&right.name));
        values
    }

    /// 返回函数统计快照。
    #[must_use]
    pub fn function_stat_values(&self, reset: bool) -> Vec<WallFunctionStatValue> {
        let mut values = self
            .function_stats
            .iter()
            .map(|entry| entry.value().stat_value(entry.key().clone(), reset))
            .collect::<Vec<_>>();
        values.sort_by(|left, right| left.name.cmp(&right.name));
        values
    }

    /// 返回所有白/黑 SQL 快照。
    #[must_use]
    pub fn sql_stat_values(&self, reset: bool) -> Vec<WallSqlStatValue> {
        self.white_list
            .iter()
            .chain(self.black_list.iter())
            .map(|(_, stat)| stat.stat_value(reset))
            .collect()
    }

    /// 返回白名单 SQL 快照。
    #[must_use]
    pub fn white_list_values(&self, reset: bool) -> Vec<WallSqlStatValue> {
        self.white_list
            .iter()
            .map(|(_, stat)| stat.stat_value(reset))
            .collect()
    }

    /// 返回黑名单 SQL 快照。
    #[must_use]
    pub fn black_list_values(&self, reset: bool) -> Vec<WallSqlStatValue> {
        self.black_list
            .iter()
            .map(|(_, stat)| stat.stat_value(reset))
            .collect()
    }

    /// 返回 provider 的 canonical 管理快照。
    ///
    /// 对应 Java：`WallProvider#getStatValue(boolean)`。`reset=true` 时累计计数
    /// 和子统计使用原子 swap 取走，但白/黑名单 cache 本身保持存在。
    #[must_use]
    pub fn stat_value(&self, reset: bool) -> WallProviderStatValue {
        let tables = self
            .table_stat_values(reset)
            .into_iter()
            .filter(|value| value.total_execute_count() != 0)
            .collect();
        let functions = self
            .function_stat_values(reset)
            .into_iter()
            .filter(|value| value.invoke_count != 0)
            .collect();
        let white_list = self
            .white_list_values(reset)
            .into_iter()
            .filter(|value| value.execute_count != 0)
            .collect();
        let black_list = self
            .black_list_values(reset)
            .into_iter()
            .filter(|value| value.execute_count != 0)
            .collect();
        WallProviderStatValue {
            name: self.name(),
            check_count: load_or_reset(&self.check_count, reset),
            hard_check_count: load_or_reset(&self.hard_check_count, reset),
            violation_count: load_or_reset(&self.violation_count, reset),
            white_list_hit_count: load_or_reset(&self.white_list_hit_count, reset),
            black_list_hit_count: load_or_reset(&self.black_list_hit_count, reset),
            syntax_error_count: load_or_reset(&self.syntax_error_count, reset),
            violation_effect_row_count: load_or_reset(&self.violation_effect_row_count, reset),
            tables,
            functions,
            white_list,
            black_list,
        }
    }

    /// 返回 Java `getStatsMap()` 对应的管理字段。
    #[must_use]
    pub fn stats_map(&self) -> serde_json::Map<String, serde_json::Value> {
        self.stat_value(false).to_map()
    }

    /// 返回白名单当前大小。
    #[must_use]
    pub fn white_list_size(&self) -> u64 {
        self.white_list.entry_count()
    }

    /// 返回黑名单当前大小。
    #[must_use]
    pub fn black_list_size(&self) -> u64 {
        self.black_list.entry_count()
    }

    /// 清空计数、缓存和拒绝统计。
    pub fn reset(&self) {
        self.check_count.store(0, Ordering::Release);
        self.hard_check_count.store(0, Ordering::Release);
        self.white_list_hit_count.store(0, Ordering::Release);
        self.black_list_hit_count.store(0, Ordering::Release);
        self.syntax_error_count.store(0, Ordering::Release);
        self.violation_count.store(0, Ordering::Release);
        self.violation_effect_row_count.store(0, Ordering::Release);
        self.white_list.invalidate_all();
        self.black_list.invalidate_all();
        self.table_stats.clear();
        self.function_stats.clear();
        self.comment_denied_stat.reset();
    }

    /// 返回总检查次数。
    #[must_use]
    pub fn check_count(&self) -> u64 {
        self.check_count.load(Ordering::Acquire)
    }

    /// 返回实际解析检查次数。
    #[must_use]
    pub fn hard_check_count(&self) -> u64 {
        self.hard_check_count.load(Ordering::Acquire)
    }

    /// 返回白名单命中次数。
    #[must_use]
    pub fn white_list_hit_count(&self) -> u64 {
        self.white_list_hit_count.load(Ordering::Acquire)
    }

    /// 返回黑名单命中次数。
    #[must_use]
    pub fn black_list_hit_count(&self) -> u64 {
        self.black_list_hit_count.load(Ordering::Acquire)
    }

    /// 返回语法错误次数。
    #[must_use]
    pub fn syntax_error_count(&self) -> u64 {
        self.syntax_error_count.load(Ordering::Acquire)
    }

    /// 返回违规次数。
    #[must_use]
    pub fn violation_count(&self) -> u64 {
        self.violation_count.load(Ordering::Acquire)
    }

    /// 返回违规 SQL 影响行数。
    #[must_use]
    pub fn violation_effect_row_count(&self) -> u64 {
        self.violation_effect_row_count.load(Ordering::Acquire)
    }

    fn record_stats(&self, sql_stat: &WallSqlStat) {
        for (name, stat) in sql_stat.table_stats() {
            let aggregate = self
                .table_stats
                .entry(name.clone())
                .or_insert_with(|| Arc::new(WallTableStat::default()))
                .clone();
            aggregate.add_sql_table_stat(stat);
        }
        for (name, stat) in sql_stat.function_stats() {
            let aggregate = self
                .function_stats
                .entry(name.clone())
                .or_insert_with(|| Arc::new(WallFunctionStat::default()))
                .clone();
            aggregate.add_sql_function_stat(*stat);
        }
    }
}

impl Default for WallProvider {
    fn default() -> Self {
        Self::new(WallConfig::default())
    }
}

fn load_or_reset(value: &AtomicU64, reset: bool) -> u64 {
    if reset {
        value.swap(0, Ordering::AcqRel)
    } else {
        value.load(Ordering::Acquire)
    }
}

fn collect_sql_stats(
    statements: &[Statement],
) -> (
    HashMap<String, WallSqlTableStat>,
    HashMap<String, WallSqlFunctionStat>,
) {
    let mut tables = HashMap::<String, WallSqlTableStat>::new();
    let mut functions = HashMap::<String, WallSqlFunctionStat>::new();
    for statement in statements {
        match statement {
            Statement::Query(query) => {
                record_query_relations(query, &mut tables);
            }
            Statement::Insert(insert) => {
                tables
                    .entry(normalize_name(&insert.table_name))
                    .or_default()
                    .increment_insert_count();
                if let Some(source) = insert.source.as_deref() {
                    record_query_relations(source, &mut tables);
                }
            }
            Statement::Update { table, .. } => {
                if let TableFactor::Table { name, .. } = &table.relation {
                    tables
                        .entry(normalize_name(name))
                        .or_default()
                        .increment_update_count();
                }
            }
            Statement::Delete(delete) => {
                let from = match &delete.from {
                    sqlparser::ast::FromTable::WithFromKeyword(values)
                    | sqlparser::ast::FromTable::WithoutKeyword(values) => values,
                };
                for table in from {
                    if let TableFactor::Table { name, .. } = &table.relation {
                        tables
                            .entry(normalize_name(name))
                            .or_default()
                            .increment_delete_count();
                    }
                }
            }
            Statement::Truncate { table_names, .. } => {
                for table in table_names {
                    tables
                        .entry(normalize_name(&table.name))
                        .or_default()
                        .truncate_count += 1;
                }
            }
            Statement::CreateTable(create) => {
                tables
                    .entry(normalize_name(&create.name))
                    .or_default()
                    .create_count += 1;
            }
            Statement::AlterTable { name, .. } => {
                tables.entry(normalize_name(name)).or_default().alter_count += 1;
            }
            Statement::Drop {
                object_type, names, ..
            } if *object_type == ObjectType::Table => {
                for name in names {
                    tables.entry(normalize_name(name)).or_default().drop_count += 1;
                }
            }
            _ => {}
        }
        let _: ControlFlow<()> = visit_expressions(statement, |expression| {
            if let Expr::Function(function) = expression {
                functions
                    .entry(function.name.to_string().to_ascii_lowercase())
                    .or_default()
                    .increment_invoke_count();
            }
            ControlFlow::Continue(())
        });
    }
    (tables, functions)
}

fn record_query_relations(
    query: &sqlparser::ast::Query,
    tables: &mut HashMap<String, WallSqlTableStat>,
) {
    let _: ControlFlow<()> = visit_relations(query, |name| {
        tables
            .entry(normalize_name(name))
            .or_default()
            .increment_select_count();
        ControlFlow::Continue(())
    });
}

fn normalize_name(name: &ObjectName) -> String {
    name.to_string()
        .trim_matches(['`', '"', '[', ']'])
        .to_ascii_lowercase()
}
