use super::{TenantStatementType, WallConfig, WallProvider, WallSqlStat};
use crate::core::{
    AfterFilter, BatchExecContext, BeforeFilter, ConnectionDatabaseMetaDataFilterChain, DruidError,
    ExecContext, ExecResult, PhysicalDatabaseMetaData, ResultSetFilter, ResultSetFilterChain,
    ResultSetOpenContext,
};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 把 WallProvider 接入 Druid FilterChain 的 canonical Filter。
///
/// 对应 Java：`com.alibaba.druid.wall.WallFilter`。违规可按配置记录日志，并可
/// 选择抛出 `WallViolation` 阻断执行；成功/失败及影响行数回写同一 SQL 统计。
pub struct WallFilter {
    provider: Arc<WallProvider>,
    log_violation: AtomicBool,
    throw_exception: AtomicBool,
    in_flight: DashMap<Instant, Vec<Arc<WallSqlStat>>>,
}

impl WallFilter {
    /// 使用指定 provider 创建 Filter。
    #[must_use]
    pub fn new(provider: Arc<WallProvider>) -> Self {
        Self {
            provider,
            log_violation: AtomicBool::new(false),
            throw_exception: AtomicBool::new(true),
            in_flight: DashMap::new(),
        }
    }

    /// 使用默认规则创建 Filter。
    #[must_use]
    pub fn with_config(config: WallConfig) -> Self {
        Self::new(Arc::new(WallProvider::new(config)))
    }

    /// 返回 provider。
    #[must_use]
    pub fn provider(&self) -> &Arc<WallProvider> {
        &self.provider
    }

    /// 设置是否记录违规。
    pub fn set_log_violation(&self, log_violation: bool) {
        self.log_violation.store(log_violation, Ordering::Release);
    }

    /// 设置是否以错误阻断违规 SQL。
    pub fn set_throw_exception(&self, throw_exception: bool) {
        self.throw_exception
            .store(throw_exception, Ordering::Release);
    }

    fn before_sql(
        &self,
        sql: &str,
        parameters: &[crate::core::Value],
        evaluate_update_items: bool,
        rewrite_tenant: bool,
    ) -> Result<(Option<Arc<WallSqlStat>>, String), DruidError> {
        let result = if rewrite_tenant {
            self.provider.try_check(sql)?
        } else {
            self.provider.try_check_without_tenant_rewrite(sql)?
        };
        if let Some(violation) = result.violations().first() {
            if self.log_violation.load(Ordering::Acquire) {
                tracing::error!(sql, violation = %violation, "wall violation");
            }
            if self.throw_exception.load(Ordering::Acquire) {
                return Err(DruidError::WallViolation(violation.to_string()));
            }
        }
        if evaluate_update_items {
            if let Some(items) = result.update_check_items() {
                let Some(handler) = self.provider.config().update_check_handler() else {
                    return Err(DruidError::WallViolation(
                        "wall update check handler missing.".to_owned(),
                    ));
                };
                for item in items {
                    let Some((set_value, filter_values)) = item.resolve_values(parameters) else {
                        return Err(DruidError::WallViolation(
                            "wall update check expression is neither literal nor placeholder."
                                .to_owned(),
                        ));
                    };
                    if !handler.check(
                        &item.table_name,
                        &item.column_name,
                        &set_value,
                        &filter_values,
                    ) {
                        return Err(DruidError::WallViolation(
                            "wall update check failed.".to_owned(),
                        ));
                    }
                }
            }
        }
        Ok((result.sql_stat().cloned(), result.sql().to_owned()))
    }

    fn after_sql(&self, stat: &WallSqlStat, result: &Result<ExecResult, DruidError>) {
        match result {
            Ok(result) => {
                stat.add_update_count(result.rows_affected);
                if let Some(row_count) = result.row_count {
                    stat.add_fetch_row_count(row_count);
                }
                self.provider.record_effect_rows_for_stat(
                    stat,
                    result.rows_affected,
                    result.row_count,
                );
                if !stat.violations().is_empty() {
                    self.provider
                        .add_violation_effect_row_count(result.rows_affected);
                }
            }
            Err(_) => {
                stat.increment_execute_error_count();
            }
        }
    }

    fn preprocess_result_set(
        &self,
        context: &mut ResultSetOpenContext<'_>,
    ) -> Result<(), DruidError> {
        let config = self.provider.config();
        let tenant_call_back = config.tenant_call_back();
        let tenant_table_pattern = config.tenant_table_pattern.as_str();
        if tenant_call_back.is_none() && tenant_table_pattern.is_empty() {
            return Ok(());
        }

        let meta_data = context.raw_result_set().meta_data()?;
        let column_count = meta_data.column_count()?;
        let mut logic_column_map = HashMap::new();
        let mut physical_column_map = HashMap::new();
        let mut hidden_columns = Vec::new();
        let mut tenant_columns = Vec::new();
        let mut logic_column = 1_i32;

        for physical_column in 1..=column_count {
            let table_name = meta_data.table_name(physical_column)?;
            let mut hidden_column = tenant_call_back
                .as_ref()
                .and_then(|call_back| call_back.hidden_column(&table_name));
            let mut tenant_column = tenant_call_back.as_ref().and_then(|call_back| {
                call_back.tenant_column(TenantStatementType::Select, &table_name)
            });

            // Java metadata 可以返回 null；Rust SPI 用空字符串表达未知表名。
            // 其余匹配规则逐分支复刻 ServletPathMatcher。
            if option_is_empty(&hidden_column) || option_is_empty(&tenant_column) {
                if table_name.is_empty() || servlet_path_matches(tenant_table_pattern, &table_name)
                {
                    if option_is_empty(&hidden_column) {
                        hidden_column = non_empty(config.tenant_column.as_str());
                    }
                    if option_is_empty(&tenant_column) {
                        tenant_column = non_empty(config.tenant_column.as_str());
                    }
                }
            }

            let column_name = meta_data.column_name(physical_column)?;
            let is_hidden = hidden_column
                .as_deref()
                .is_some_and(|hidden| hidden.eq_ignore_ascii_case(&column_name));
            let physical_column_i32 = i32::try_from(physical_column).map_err(|_| {
                DruidError::InvalidArgument("result set column index exceeds i32".to_owned())
            })?;
            if is_hidden {
                hidden_columns.push(physical_column_i32);
            } else {
                logic_column_map.insert(logic_column, physical_column_i32);
                physical_column_map.insert(physical_column_i32, logic_column);
                logic_column = logic_column.saturating_add(1);
            }

            if tenant_column
                .as_deref()
                .is_some_and(|tenant| tenant.eq_ignore_ascii_case(&column_name))
            {
                tenant_columns.push(physical_column);
            }
        }

        if !hidden_columns.is_empty() {
            context.set_logic_column_map(logic_column_map);
            context.set_physical_column_map(physical_column_map);
            context.set_hidden_columns(hidden_columns);
        }
        context.set_tenant_columns(tenant_columns);
        Ok(())
    }
}

impl Default for WallFilter {
    fn default() -> Self {
        Self::with_config(WallConfig::default())
    }
}

#[async_trait::async_trait]
impl BeforeFilter for WallFilter {
    fn name(&self) -> &str {
        "wall"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        let rewrite_tenant = context.prepared_parameters.is_none();
        let (stat, rewritten_sql) =
            self.before_sql(&context.sql, context.params, true, rewrite_tenant)?;
        // Java 普通 Statement 在执行 hook 内替换 SQL。PreparedStatement 的 SQL
        // 必须在 prepare 边界替换并重建物理对象，执行期不能只改字符串冒充。
        if rewrite_tenant {
            context.sql = rewritten_sql;
        }
        self.in_flight
            .insert(context.start, stat.into_iter().collect());
        Ok(())
    }

    fn prepare_statement_sql(&self, sql: &str) -> Result<String, DruidError> {
        self.before_sql(sql, &[], false, true)
            .map(|(_, rewritten_sql)| rewritten_sql)
    }

    fn statement_add_batch_sql(&self, sql: &str) -> Result<String, DruidError> {
        self.before_sql(sql, &[], false, true)
            .map(|(_, rewritten_sql)| rewritten_sql)
    }

    fn connection_get_meta_data<'filters, 'connection>(
        &self,
        chain: ConnectionDatabaseMetaDataFilterChain<'filters, 'connection>,
    ) -> Result<Box<dyn PhysicalDatabaseMetaData + 'connection>, DruidError> {
        if self.provider.config().do_privileged_allow && WallProvider::is_privileged() {
            return chain.connection_get_meta_data();
        }
        if !self.provider.config().metadata_allow {
            if self.log_violation.load(Ordering::Acquire) {
                tracing::error!(
                    connection_id = chain.connection_id(),
                    "not support method : Connection.getMetaData"
                );
            }
            if self.throw_exception.load(Ordering::Acquire) {
                return Err(DruidError::WallViolation(
                    "not support method : Connection.getMetaData".to_owned(),
                ));
            }
        }
        chain.connection_get_meta_data()
    }

    async fn before_batch(&self, context: &mut BatchExecContext<'_>) -> Result<(), DruidError> {
        // Java WallFilter 在 Statement#addBatch(String) 时逐条检查/改写，且没有
        // 覆盖 preparedStatement_executeBatch；executeBatch 不重复 hard-check，
        // 也不对每组 Prepared 参数调用 WallUpdateCheckHandler。
        self.in_flight.insert(context.start, Vec::new());
        Ok(())
    }

    async fn before_execute_error(
        &self,
        context: &ExecContext<'_>,
        _error: &DruidError,
    ) -> Result<(), DruidError> {
        self.in_flight.remove(&context.start);
        Ok(())
    }

    async fn before_batch_error(
        &self,
        context: &BatchExecContext<'_>,
        _error: &DruidError,
    ) -> Result<(), DruidError> {
        self.in_flight.remove(&context.start);
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for WallFilter {
    fn name(&self) -> &str {
        "wall"
    }

    async fn after(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        if let Some((_, stats)) = self.in_flight.remove(&context.start) {
            if let Some(stat) = stats.first() {
                self.after_sql(stat, result);
            }
        }
        Ok(())
    }

    async fn after_batch(
        &self,
        context: &BatchExecContext<'_>,
        result: &Result<Vec<i32>, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        let stats = self
            .in_flight
            .remove(&context.start)
            .map(|(_, stats)| stats)
            .unwrap_or_default();
        match result {
            Ok(update_counts) => {
                for (index, stat) in stats.iter().enumerate() {
                    let rows_affected = update_counts
                        .get(index)
                        .and_then(|count| u64::try_from(*count).ok())
                        .unwrap_or_default();
                    self.after_sql(
                        stat,
                        &Ok(ExecResult {
                            rows_affected,
                            last_insert_id: None,
                            row_count: None,
                        }),
                    );
                }
            }
            Err(error) => {
                for stat in stats {
                    self.after_sql(&stat, &Err(error.clone()));
                }
            }
        }
        Ok(())
    }
}

impl ResultSetFilter for WallFilter {
    fn result_set_open_after_with_proxy(
        &self,
        context: &mut ResultSetOpenContext<'_>,
    ) -> Result<(), DruidError> {
        self.preprocess_result_set(context)
    }

    fn result_set_next(&self, chain: &mut ResultSetFilterChain<'_>) -> Result<bool, DruidError> {
        let has_next = chain.result_set_next()?;
        if !has_next {
            return Ok(false);
        }
        let Some(tenant_call_back) = self.provider.config().tenant_call_back() else {
            return Ok(true);
        };
        for column_index in chain.context().tenant_columns() {
            let value = chain.raw_value(column_index)?;
            tenant_call_back.filter_resultset_tenant_column(&value);
        }
        Ok(true)
    }
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn option_is_empty(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(str::is_empty)
}

/// 逐分支迁移 Java `ServletPathMatcher#matches`。
fn servlet_path_matches(pattern: &str, source: &str) -> bool {
    let pattern = pattern.trim();
    let source = source.trim();
    if let Some(prefix) = pattern.strip_suffix('*') {
        source.starts_with(prefix)
    } else if let Some(suffix) = pattern.strip_prefix('*') {
        source.ends_with(suffix)
    } else if let (Some(start), Some(end)) = (pattern.find('*'), pattern.rfind('*')) {
        source.starts_with(&pattern[..start]) && source.ends_with(&pattern[end + 1..])
    } else {
        pattern == source
    }
}
