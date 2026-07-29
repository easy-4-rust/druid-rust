use super::{
    DbType, SqlUtils, Wall, WallCheckResult, WallConfig, WallDenyStat, WallFunctionStat,
    WallFunctionStatValue, WallSqlFunctionStat, WallSqlStat, WallSqlStatValue, WallSqlTableStat,
    WallTableStat, WallTableStatValue, WallViolation,
};
use dashmap::DashMap;
use moka::sync::Cache;
use parking_lot::RwLock;
use sqlparser::ast::{
    visit_expressions, visit_relations, Expr, ObjectName, ObjectType, Statement, TableFactor,
};
use sqlparser::parser::Parser;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const WHITE_SQL_MAX_SIZE: u64 = 1024;
const BLACK_SQL_MAX_SIZE: u64 = 256;

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
        self.check_count.fetch_add(1, Ordering::Relaxed);
        if let Some(stat) = self.white_list.get(sql) {
            self.white_list_hit_count.fetch_add(1, Ordering::Relaxed);
            self.record_stats(&stat);
            return WallCheckResult::new(sql.to_owned(), Vec::new(), Vec::new(), false, stat);
        }
        if let Some(stat) = self.black_list.get(sql) {
            self.black_list_hit_count.fetch_add(1, Ordering::Relaxed);
            self.record_stats(&stat);
            return WallCheckResult::new(
                sql.to_owned(),
                Vec::new(),
                stat.violations().to_vec(),
                stat.violations()
                    .iter()
                    .any(|violation| matches!(violation, WallViolation::SyntaxError(_))),
                stat,
            );
        }

        self.hard_check_count.fetch_add(1, Ordering::Relaxed);
        let dialect = SqlUtils::dialect(self.db_type());
        let parsed = Parser::parse_sql(dialect.as_ref(), sql);
        let statements = parsed.clone().unwrap_or_default();
        let violations = match self.wall.check(sql) {
            Ok(()) => Vec::new(),
            Err(violations) => violations,
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
        self.record_stats(&stat);
        if violations.is_empty() {
            self.white_list.insert(sql.to_owned(), Arc::clone(&stat));
        } else {
            self.violation_count.fetch_add(1, Ordering::Relaxed);
            if syntax_error {
                self.syntax_error_count.fetch_add(1, Ordering::Relaxed);
            }
            self.black_list.insert(sql.to_owned(), Arc::clone(&stat));
        }
        WallCheckResult::new(sql.to_owned(), statements, violations, syntax_error, stat)
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
