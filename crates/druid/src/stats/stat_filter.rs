//! 对应 Java 类：com.alibaba.druid.filter.stat.StatFilter
//!
//! 统计 `Filter`，实现 `AfterFilter` 接口。

use super::{StatFilterContext, StatsCollector};
use crate::core::{
    AfterFilter, BatchExecContext, BatchExecKind, BeforeFilter, ConnectionEvent, DruidError,
    ExecContext, ExecOperation, ExecResult, JdbcObject, PreparedInputParameter, ResultSetFilter,
    ResultSetFilterChain, ResultSetFilterContext, Value,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// 统计 Filter。
///
/// 对应 Druid Java 的 `StatFilter`，在 SQL 执行后记录统计。
pub struct StatFilter {
    collector: Arc<StatsCollector>,
    merge_sql: AtomicBool,
    slow_sql_millis: AtomicI64,
    log_slow_sql: AtomicBool,
    slow_sql_log_level: RwLock<String>,
    connection_stack_trace_enable: AtomicBool,
    db_type: RwLock<Option<String>>,
}

impl StatFilter {
    /// 创建普通统计 Filter；与 Java 一致，默认不合并 SQL。
    pub fn new(collector: Arc<StatsCollector>) -> Self {
        Self {
            collector,
            merge_sql: AtomicBool::new(false),
            slow_sql_millis: AtomicI64::new(3_000),
            log_slow_sql: AtomicBool::new(false),
            slow_sql_log_level: RwLock::new("ERROR".to_owned()),
            connection_stack_trace_enable: AtomicBool::new(false),
            db_type: RwLock::new(None),
        }
    }

    /// 返回是否参数化并合并 SQL。对应 Java：`StatFilter#isMergeSql()`。
    #[must_use]
    pub fn is_merge_sql(&self) -> bool {
        self.merge_sql.load(Ordering::Acquire)
    }

    /// 设置是否参数化并合并 SQL。对应 Java：`StatFilter#setMergeSql(boolean)`。
    pub fn set_merge_sql(&self, merge_sql: bool) {
        self.merge_sql.store(merge_sql, Ordering::Release);
    }

    /// 返回慢 SQL 阈值（毫秒）。
    ///
    /// 对应 Java：`StatFilterMBean#getSlowSqlMillis()`。
    #[must_use]
    pub fn get_slow_sql_millis(&self) -> i64 {
        self.slow_sql_millis.load(Ordering::Acquire)
    }

    /// 设置慢 SQL 阈值（毫秒）。
    ///
    /// Java `long` 允许负数，Rust 不擅自校正；负阈值表示所有执行均为慢 SQL。
    pub fn set_slow_sql_millis(&self, slow_sql_millis: i64) {
        self.slow_sql_millis
            .store(slow_sql_millis, Ordering::Release);
    }

    /// 返回是否输出慢 SQL 日志。
    ///
    /// 对应 Java：`StatFilterMBean#isLogSlowSql()`。
    #[must_use]
    pub fn is_log_slow_sql(&self) -> bool {
        self.log_slow_sql.load(Ordering::Acquire)
    }

    /// 设置是否输出慢 SQL 日志。
    ///
    /// 对应 Java：`StatFilterMBean#setLogSlowSql(boolean)`。
    pub fn set_log_slow_sql(&self, log_slow_sql: bool) {
        self.log_slow_sql.store(log_slow_sql, Ordering::Release);
    }

    /// 返回慢 SQL tracing level。
    #[must_use]
    pub fn get_slow_sql_log_level(&self) -> String {
        self.slow_sql_log_level.read().clone()
    }

    /// 设置慢 SQL tracing level。
    ///
    /// 与 Java 一样只接受 ERROR/WARN/INFO/DEBUG（忽略大小写），其他值不改变。
    pub fn set_slow_sql_log_level(&self, slow_sql_log_level: &str) {
        if matches!(
            slow_sql_log_level.to_ascii_uppercase().as_str(),
            "ERROR" | "WARN" | "INFO" | "DEBUG"
        ) {
            *self.slow_sql_log_level.write() = slow_sql_log_level.to_ascii_uppercase();
        }
    }

    /// 返回是否记录连接创建栈。
    #[must_use]
    pub fn is_connection_stack_trace_enable(&self) -> bool {
        self.connection_stack_trace_enable.load(Ordering::Acquire)
    }

    /// 设置是否记录连接创建栈。
    pub fn set_connection_stack_trace_enable(&self, enabled: bool) {
        self.connection_stack_trace_enable
            .store(enabled, Ordering::Release);
    }

    /// 返回 Filter 的数据库类型。
    #[must_use]
    pub fn get_db_type(&self) -> Option<String> {
        self.db_type.read().clone()
    }

    /// 设置 Filter 的数据库类型。
    pub fn set_db_type(&self, db_type: Option<&str>) {
        *self.db_type.write() = db_type.map(str::to_owned);
    }

    /// 按当前 merge 开关参数化 SQL。
    ///
    /// 对应 Java：`StatFilterMBean#mergeSql(String, String)`。当前 Rust
    /// 参数化器对公共字面量规则统一处理；方言专项差异继续由 SQL 子域跟踪。
    #[must_use]
    pub fn merge_sql(&self, sql: &str, _db_type: Option<&str>) -> String {
        if self.is_merge_sql() {
            super::parameterize(sql).template
        } else {
            sql.to_owned()
        }
    }

    /// 返回本 `Filter` 所属数据源的 `ResultSet` 统计对象。
    pub fn result_set_stat(&self) -> &super::JdbcResultSetStat {
        self.collector.result_set_stat()
    }

    fn apply_config(&self, properties: &HashMap<String, String>) {
        if let Some(value) = properties.get("druid.stat.mergeSql") {
            match value.as_str() {
                "true" => self.set_merge_sql(true),
                "false" => self.set_merge_sql(false),
                _ => {}
            }
        }
        if let Some(value) = properties.get("druid.stat.slowSqlMillis") {
            let value = trim_java_string(value);
            if !value.is_empty() {
                match value.parse::<i64>() {
                    Ok(value) => self.set_slow_sql_millis(value),
                    Err(error) => {
                        tracing::error!(%error, "property 'druid.stat.slowSqlMillis' format error");
                    }
                }
            }
        }
        if let Some(value) = properties.get("druid.stat.logSlowSql") {
            match value.as_str() {
                "true" => self.set_log_slow_sql(true),
                "false" => self.set_log_slow_sql(false),
                _ => {}
            }
        }
        if let Some(value) = properties.get("druid.stat.slowSqlLogLevel") {
            self.set_slow_sql_log_level(value);
        }
        if let Some(value) = properties.get("druid.stat.sql.MaxSize") {
            let value = trim_java_string(value);
            if !value.is_empty() {
                match value.parse::<i32>() {
                    Ok(value) => self.collector.set_max_sql_size(value),
                    Err(error) => {
                        tracing::error!(%error, "property 'druid.stat.sql.MaxSize' format error");
                    }
                }
            }
        }
    }

    fn log_slow_sql(&self, sql: &str, elapsed: Duration) {
        let elapsed_millis = i128::try_from(elapsed.as_millis()).unwrap_or(i128::MAX);
        if !self.is_slow(elapsed) || !self.is_log_slow_sql() {
            return;
        }
        match self.slow_sql_log_level.read().as_str() {
            "WARN" => tracing::warn!(sql = %sql, elapsed_ms = elapsed_millis, "slow sql"),
            "INFO" => tracing::info!(sql = %sql, elapsed_ms = elapsed_millis, "slow sql"),
            "DEBUG" => tracing::debug!(sql = %sql, elapsed_ms = elapsed_millis, "slow sql"),
            _ => tracing::error!(sql = %sql, elapsed_ms = elapsed_millis, "slow sql"),
        }
    }

    fn is_slow(&self, elapsed: Duration) -> bool {
        i128::try_from(elapsed.as_millis()).unwrap_or(i128::MAX)
            >= i128::from(self.get_slow_sql_millis())
    }
}

#[async_trait::async_trait]
impl BeforeFilter for StatFilter {
    fn name(&self) -> &str {
        "stat"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        let sql_stat = self
            .collector
            .sql_merger
            .prepare(&context.sql, self.is_merge_sql());
        let db_type = self.get_db_type();
        sql_stat.set_management_identity(Some(context.data_source), None, db_type.as_deref());
        context.fingerprint = Some(sql_stat.fingerprint);
        sql_stat.increment_running_count();
        if context.in_transaction {
            sql_stat.increment_in_transaction_count();
        }
        let result =
            StatFilterContext::global().execute_before(&context.sql, context.in_transaction);
        if result.is_err() {
            sql_stat.decrement_running_count();
        }
        result
    }

    async fn before_execute_error(
        &self,
        context: &ExecContext<'_>,
        _error: &DruidError,
    ) -> Result<(), DruidError> {
        if let Some(sql_stat) = context
            .fingerprint
            .and_then(|fingerprint| self.collector.sql_merger.get_stat(fingerprint))
        {
            sql_stat.decrement_running_count();
        }
        Ok(())
    }

    fn config_from_properties(
        &self,
        properties: &HashMap<String, String>,
    ) -> Result<(), DruidError> {
        self.apply_config(properties);
        Ok(())
    }

    fn config_from_system_properties(
        &self,
        properties: &HashMap<String, String>,
    ) -> Result<(), DruidError> {
        self.apply_config(properties);
        Ok(())
    }

    async fn before_batch(&self, context: &mut BatchExecContext<'_>) -> Result<(), DruidError> {
        self.collector
            // Java PreparedStatementProxyImpl 不填充继承的 batchSqlList，故为 0。
            .record_execute_batch(context.statements.len());
        let sql_stat = self
            .collector
            .sql_merger
            .prepare(context.sql, self.is_merge_sql());
        let db_type = self.get_db_type();
        sql_stat.set_management_identity(Some(context.data_source), None, db_type.as_deref());
        context.fingerprint = Some(sql_stat.fingerprint);
        sql_stat.increment_running_count();
        if context.in_transaction {
            sql_stat.increment_in_transaction_count();
        }
        let result =
            StatFilterContext::global().execute_before(context.sql, context.in_transaction);
        if result.is_err() {
            sql_stat.decrement_running_count();
        }
        result
    }

    async fn before_batch_error(
        &self,
        context: &BatchExecContext<'_>,
        _error: &DruidError,
    ) -> Result<(), DruidError> {
        if let Some(sql_stat) = context
            .fingerprint
            .and_then(|fingerprint| self.collector.sql_merger.get_stat(fingerprint))
        {
            sql_stat.decrement_running_count();
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for StatFilter {
    fn name(&self) -> &str {
        "stat"
    }

    async fn after(
        &self,
        ctx: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let running_stat = ctx
            .fingerprint
            .and_then(|fingerprint| self.collector.sql_merger.get_stat(fingerprint));
        let sql_stat = self.collector.record_sql_with_merge_and_slow_millis_stat(
            &ctx.sql,
            elapsed,
            result.is_ok(),
            self.is_merge_sql(),
            self.get_slow_sql_millis(),
        );
        if let Some(running_stat) = running_stat {
            running_stat.decrement_running_count();
        }
        if let Err(error) = result {
            sql_stat.record_error_detail(error);
        }
        if result.is_ok() && self.is_slow(elapsed) {
            sql_stat.set_last_slow_parameters(Some(build_slow_parameters(
                ctx.params,
                ctx.prepared_parameters,
            )));
        }
        if matches!(ctx.operation, ExecOperation::Update)
            || matches!(ctx.operation, ExecOperation::Execute)
                && (result.is_err()
                    || result
                        .as_ref()
                        .is_ok_and(|execution| execution.row_count.is_none()))
        {
            sql_stat.record_execute_and_result_hold_time(elapsed);
        }
        self.log_slow_sql(&ctx.sql, elapsed);
        let context = StatFilterContext::global();
        if let Ok(execution) = result {
            match ctx.operation {
                ExecOperation::Update => {
                    let update_count = i32::try_from(execution.rows_affected).unwrap_or(i32::MAX);
                    sql_stat.add_update_count(update_count);
                    // Java 对 executeUpdate 同时记录一次零行 ResultSet 桶。
                    sql_stat.add_fetch_row_count(0);
                    context.add_update_count(update_count)?;
                }
                ExecOperation::Query => {
                    // eager Rust 查询没有 ResultSet close 回调，只能在此完成等价统计；
                    // 流式 ResultSet 的 row_count 为 None，统一延迟到 close。
                    if let Some(row_count) = execution.row_count {
                        sql_stat.add_fetch_row_count(row_count);
                    }
                }
                ExecOperation::Execute if execution.row_count.is_none() => {
                    // Java generic execute 的更新首结果只累加 SQL updateCount。
                    let update_count = i32::try_from(execution.rows_affected).unwrap_or(i32::MAX);
                    sql_stat.add_update_count(update_count);
                }
                ExecOperation::Execute | ExecOperation::Batch => {}
            }
        }
        let elapsed_nanos = i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX);
        context.execute_after(Some(&ctx.sql), elapsed_nanos, result.as_ref().err())
    }

    async fn after_batch(
        &self,
        context: &BatchExecContext<'_>,
        result: &Result<Vec<i32>, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let running_stat = context
            .fingerprint
            .and_then(|fingerprint| self.collector.sql_merger.get_stat(fingerprint));
        let sql_stat = self.collector.record_sql_with_merge_and_slow_millis_stat(
            context.sql,
            elapsed,
            result.is_ok(),
            self.is_merge_sql(),
            self.get_slow_sql_millis(),
        );
        if let Some(running_stat) = running_stat {
            running_stat.decrement_running_count();
        }
        if let Err(error) = result {
            sql_stat.record_error_detail(error);
        }
        if result.is_ok() && self.is_slow(elapsed) {
            let params = context
                .parameter_sets
                .last()
                .map(Vec::as_slice)
                .unwrap_or_default();
            let prepared_parameters = context
                .prepared_parameter_sets
                .and_then(|sets| sets.last())
                .map(Vec::as_slice);
            sql_stat
                .set_last_slow_parameters(Some(build_slow_parameters(params, prepared_parameters)));
        }
        sql_stat.record_execute_and_result_hold_time(elapsed);
        self.log_slow_sql(context.sql, elapsed);
        let global = StatFilterContext::global();
        // 普通 Statement 使用 batchSqlList 长度；PreparedStatement 在 Java 代理中
        // 该列表固定为空，因此 BatchExecContext 已按入口保留相同 statements 语义。
        sql_stat.add_execute_batch_count(context.statements.len());
        if let Ok(update_counts) = result {
            for update_count in update_counts {
                sql_stat.add_update_count(*update_count);
                sql_stat.add_fetch_row_count(0);
                global.add_update_count(*update_count)?;
            }
        }
        let elapsed_nanos = i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX);
        let sql = match (context.kind, result.is_ok()) {
            // Java PreparedStatementProxyImpl#getLastExecuteSql() 固定返回预编译 SQL。
            (BatchExecKind::PreparedStatement, _) => Some(context.sql),
            // 普通 Statement 成功 batch 不设置 lastExecuteSql；错误路径使用 batch SQL。
            (BatchExecKind::Statement, true) => None,
            (BatchExecKind::Statement, false) => Some(context.sql),
        };
        global.execute_after(sql, elapsed_nanos, result.as_ref().err())
    }

    async fn after_connection_event(
        &self,
        event: &ConnectionEvent,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        match event {
            ConnectionEvent::Commit => StatFilterContext::global().commit(),
            ConnectionEvent::Rollback => StatFilterContext::global().rollback(),
            _ => Ok(()),
        }
    }
}

impl ResultSetFilter for StatFilter {
    fn result_set_open_after(&self, context: &ResultSetFilterContext) -> Result<(), DruidError> {
        self.collector.result_set_stat().before_open();
        context.set_construct_time();
        StatFilterContext::global().result_set_open()
    }

    fn result_set_close(&self, chain: &mut ResultSetFilterChain<'_>) -> Result<(), DruidError> {
        let elapsed = chain.context().elapsed().unwrap_or_default();
        let fetch_row_count = chain.context().fetch_row_count();
        let stat_elapsed_nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        let listener_elapsed_nanos = i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX);
        let stat = self.collector.result_set_stat();
        stat.after_close(stat_elapsed_nanos);
        stat.add_fetch_row_count(u64::try_from(fetch_row_count).unwrap_or_default());
        stat.increment_close_counter();
        StatFilterContext::global().add_fetch_row_count(fetch_row_count)?;
        if chain.context().close_count() == 0 {
            if let Some(sql) = chain.context().sql() {
                let key = if self.is_merge_sql() {
                    super::parameterize(sql).fingerprint
                } else {
                    super::fingerprint(sql)
                };
                if let Some(sql_stat) = self.collector.sql_merger.get_stat(key) {
                    sql_stat
                        .add_fetch_row_count(u64::try_from(fetch_row_count).unwrap_or_default());
                    sql_stat.add_result_set_hold_time(
                        chain
                            .context()
                            .statement_execute_elapsed()
                            .unwrap_or_default(),
                        elapsed,
                    );
                    sql_stat.add_read_string_length(chain.context().read_string_length());
                    sql_stat.add_read_bytes_length(chain.context().read_bytes_length());
                    sql_stat.add_input_stream_open_count(chain.context().open_input_stream_count());
                    sql_stat.add_reader_open_count(chain.context().open_reader_count());
                }
            }
        }
        chain.result_set_close()?;
        StatFilterContext::global().result_set_close(listener_elapsed_nanos)
    }
}

fn trim_java_string(value: &str) -> &str {
    value.trim_matches(|character| character <= '\u{20}')
}

fn build_slow_parameters(
    params: &[Value],
    prepared_parameters: Option<&[PreparedInputParameter]>,
) -> String {
    let parameters = prepared_parameters.map_or_else(
        || params.iter().map(slow_value).collect(),
        |parameters| parameters.iter().map(slow_prepared_parameter).collect(),
    );
    serde_json::to_string(&serde_json::Value::Array(parameters)).unwrap_or_else(|_| "[]".to_owned())
}

fn slow_prepared_parameter(parameter: &PreparedInputParameter) -> serde_json::Value {
    match parameter {
        PreparedInputParameter::RustValue(value) => slow_value(value),
        PreparedInputParameter::Null { .. } => serde_json::Value::Null,
        PreparedInputParameter::Boolean(value) => serde_json::Value::Bool(*value),
        PreparedInputParameter::Byte(value) => json_integer(i64::from(*value)),
        PreparedInputParameter::Short(value) => json_integer(i64::from(*value)),
        PreparedInputParameter::Int(value) => json_integer(i64::from(*value)),
        PreparedInputParameter::Long(value) => json_integer(*value),
        PreparedInputParameter::Float(value) => json_float(f64::from(*value)),
        PreparedInputParameter::Double(value) => json_float(*value),
        PreparedInputParameter::BigDecimal(value) => {
            value.as_ref().map_or(serde_json::Value::Null, json_decimal)
        }
        PreparedInputParameter::String(value) | PreparedInputParameter::NString(value) => value
            .as_deref()
            .map_or(serde_json::Value::Null, json_java_string),
        PreparedInputParameter::Bytes(value) => value
            .as_ref()
            .map_or(serde_json::Value::Null, |_| json_marker("<[B>")),
        PreparedInputParameter::Date { value, .. } => value
            .map_or(serde_json::Value::Null, |value| {
                json_marker(&value.format("%Y-%m-%d").to_string())
            }),
        PreparedInputParameter::Time { value, .. } => value
            .map_or(serde_json::Value::Null, |value| {
                json_marker(&value.format("%H:%M:%S").to_string())
            }),
        PreparedInputParameter::Timestamp { value, .. } => value
            .map_or(serde_json::Value::Null, |value| {
                json_marker(&value.format("%Y-%m-%d %H:%M:%S").to_string())
            }),
        PreparedInputParameter::AsciiStream { stream, .. }
        | PreparedInputParameter::UnicodeStream { stream, .. }
        | PreparedInputParameter::BinaryStream { stream, .. }
        | PreparedInputParameter::BlobStream { stream, .. } => stream
            .as_ref()
            .map_or(serde_json::Value::Null, |_| json_marker("<InputStream>")),
        PreparedInputParameter::CharacterStream { reader, .. }
        | PreparedInputParameter::NCharacterStream { reader, .. }
        | PreparedInputParameter::ClobReader { reader, .. }
        | PreparedInputParameter::NClobReader { reader, .. } => reader
            .as_ref()
            .map_or(serde_json::Value::Null, |_| json_marker("<java.io.Reader>")),
        PreparedInputParameter::Object { value, .. } => value
            .as_ref()
            .map_or(serde_json::Value::Null, slow_jdbc_object),
        PreparedInputParameter::Ref(value) => resource_marker(value, "<java.sql.Ref>"),
        PreparedInputParameter::Blob(value) => resource_marker(value, "<Blob>"),
        PreparedInputParameter::Clob(value) => resource_marker(value, "<Clob>"),
        PreparedInputParameter::NClob(value) => resource_marker(value, "<NClob>"),
        PreparedInputParameter::Array(value) => resource_marker(value, "<java.sql.Array>"),
        PreparedInputParameter::Url(value) => resource_marker(value, "<java.net.URL>"),
        PreparedInputParameter::RowId(value) => resource_marker(value, "<java.sql.RowId>"),
        PreparedInputParameter::SqlXml(value) => resource_marker(value, "<java.sql.SQLXML>"),
    }
}

fn slow_value(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(value) => serde_json::Value::Bool(*value),
        Value::Int(value) => json_integer(*value),
        Value::Float(value) => json_float(*value),
        Value::Decimal(value) => json_decimal(value),
        Value::Date(value) => json_marker(&value.format("%Y-%m-%d").to_string()),
        Value::Time(value) => json_marker(&value.format("%H:%M:%S").to_string()),
        Value::Timestamp(value) => json_marker(&value.format("%Y-%m-%d %H:%M:%S").to_string()),
        Value::String(value) => json_java_string(value),
        Value::Bytes(_) => json_marker("<[B>"),
    }
}

fn slow_jdbc_object(value: &JdbcObject) -> serde_json::Value {
    match value {
        JdbcObject::Scalar(value) => slow_value(value),
        JdbcObject::String(value) | JdbcObject::NString(value) => json_java_string(value),
        JdbcObject::Boolean(value) => serde_json::Value::Bool(*value),
        JdbcObject::Byte(value) => json_integer(i64::from(*value)),
        JdbcObject::Short(value) => json_integer(i64::from(*value)),
        JdbcObject::Integer(value) => json_integer(i64::from(*value)),
        JdbcObject::Long(value) => json_integer(*value),
        JdbcObject::Float(value) => json_float(f64::from(*value)),
        JdbcObject::Double(value) => json_float(*value),
        JdbcObject::BigDecimal(value) => json_decimal(value),
        JdbcObject::Date(value) => json_marker(&value.format("%Y-%m-%d").to_string()),
        JdbcObject::Time(value) => json_marker(&value.format("%H:%M:%S").to_string()),
        JdbcObject::Timestamp(value) => json_marker(&value.format("%Y-%m-%d %H:%M:%S").to_string()),
        JdbcObject::Bytes(_) => json_marker("<[B>"),
        JdbcObject::Url(_) => json_marker("<java.net.URL>"),
        JdbcObject::Ref(_) => json_marker("<java.sql.Ref>"),
        JdbcObject::Array(_) => json_marker("<java.sql.Array>"),
        JdbcObject::RowId(_) => json_marker("<java.sql.RowId>"),
        JdbcObject::SqlXml(_) => json_marker("<java.sql.SQLXML>"),
        JdbcObject::Blob(_) => json_marker("<Blob>"),
        JdbcObject::Clob(_) => json_marker("<Clob>"),
        JdbcObject::NClob(_) => json_marker("<NClob>"),
        JdbcObject::CharacterStream(_) | JdbcObject::NCharacterStream(_) => {
            json_marker("<java.io.Reader>")
        }
        JdbcObject::Custom(value) => json_marker(&format!("<{}>", value.class_name())),
    }
}

fn json_java_string(value: &str) -> serde_json::Value {
    let utf16 = value.encode_utf16().collect::<Vec<_>>();
    if utf16.len() <= 100 {
        return json_marker(value);
    }
    let mut truncated = String::from_utf16_lossy(&utf16[..97]);
    truncated.push_str("...");
    json_marker(&truncated)
}

fn json_integer(value: i64) -> serde_json::Value {
    serde_json::Value::Number(value.into())
}

fn json_float(value: f64) -> serde_json::Value {
    serde_json::Number::from_f64(value).map_or(serde_json::Value::Null, serde_json::Value::Number)
}

fn json_decimal(value: &bigdecimal::BigDecimal) -> serde_json::Value {
    serde_json::Number::from_str(&value.to_string()).map_or_else(
        |_| json_marker(&value.to_string()),
        serde_json::Value::Number,
    )
}

fn json_marker(value: &str) -> serde_json::Value {
    serde_json::Value::String(value.to_owned())
}

fn resource_marker<T>(value: &Option<T>, marker: &str) -> serde_json::Value {
    value
        .as_ref()
        .map_or(serde_json::Value::Null, |_| json_marker(marker))
}
