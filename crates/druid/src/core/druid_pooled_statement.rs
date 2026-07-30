//! 对外池化普通语句。
//!
//! 对应 Java：
//! `com.alibaba.druid.pool.DruidPooledStatement`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/pool/DruidPooledStatement.java`。

use super::druid_pooled_result_set::DruidPooledResultSetTrace;
use super::{
    DruidError, DruidPooledConnection, DruidPooledResultSet, ExecResult, FilterChain,
    PhysicalResultSet, PhysicalStatement, Row, RowSetResultSet, SqlWarning, StatementExecuteResult,
    StatementExecuteType, StatementGeneratedKeys, Unwrapped, Value, Wrapper,
};
use super::{ProxyAttributeValue, ProxyAttributes};
use crate::stats::StatsCollector;
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) struct DruidPooledStatementInner {
    pub(crate) connection_id: u64,
    pub(crate) id: u64,
    pub(crate) statement: Arc<dyn PhysicalStatement>,
    pub(crate) result_set_id_seed: Arc<std::sync::atomic::AtomicU64>,
    pub(crate) metadata_id_seed: Arc<std::sync::atomic::AtomicU64>,
    attributes: ProxyAttributes,
    pub(crate) lease_active: Arc<AtomicBool>,
    pub(crate) filter_chain: Option<Arc<FilterChain>>,
    pub(crate) stats_collector: Option<Arc<StatsCollector>>,
    state: Mutex<DruidPooledStatementState>,
}

struct DruidPooledStatementState {
    closed: bool,
    last_sql: Option<String>,
    last_execute_type: Option<StatementExecuteType>,
    last_execute_started_at: Option<Instant>,
    last_execute_elapsed: Option<Duration>,
    first_result_set: bool,
    fetch_row_peak: i32,
    exception_count: u64,
    update_count: i64,
    execute_results: Vec<StatementExecuteResult>,
    current_physical_result_set: Option<Arc<dyn PhysicalResultSet>>,
    current_result_index: usize,
    generated_keys: Vec<Row>,
    result_set_trace: Vec<DruidPooledResultSetTrace>,
}

/// 借用池化连接执行动态 SQL 的普通语句。
///
/// Rust 不允许 Statement 长期持有连接的可变借用，因此执行与会产生驱动错误
/// 的属性方法显式接收创建它的 `DruidPooledConnection`。同租约身份由共享令牌
/// 校验，连接归还后旧 Statement 不能进入下一次租约。
#[derive(Clone)]
pub struct DruidPooledStatement {
    pub(crate) inner: Arc<DruidPooledStatementInner>,
}

impl DruidPooledStatement {
    /// 判断两个句柄是否指向同一个逻辑 Statement。
    ///
    /// 用于保留 Java `ResultSet#getStatement()` 的对象身份语义；比较的是共享
    /// 内核地址，不是字段值或物理语句快照。
    pub fn is_same_statement(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub(crate) fn new(
        statement: Arc<dyn PhysicalStatement>,
        connection_id: u64,
        id: u64,
        result_set_id_seed: Arc<std::sync::atomic::AtomicU64>,
        metadata_id_seed: Arc<std::sync::atomic::AtomicU64>,
        lease_active: Arc<AtomicBool>,
        filter_chain: Option<Arc<FilterChain>>,
        stats_collector: Option<Arc<StatsCollector>>,
    ) -> Self {
        Self {
            inner: Arc::new(DruidPooledStatementInner {
                connection_id,
                id,
                statement,
                result_set_id_seed,
                metadata_id_seed,
                attributes: ProxyAttributes::default(),
                lease_active,
                filter_chain,
                stats_collector,
                state: Mutex::new(DruidPooledStatementState {
                    closed: false,
                    last_sql: None,
                    last_execute_type: None,
                    last_execute_started_at: None,
                    last_execute_elapsed: None,
                    first_result_set: false,
                    fetch_row_peak: -1,
                    exception_count: 0,
                    update_count: -1,
                    execute_results: Vec::new(),
                    current_physical_result_set: None,
                    current_result_index: 0,
                    generated_keys: Vec::new(),
                    result_set_trace: Vec::new(),
                }),
            }),
        }
    }

    pub(crate) fn from_inner(inner: Arc<DruidPooledStatementInner>) -> Self {
        Self { inner }
    }

    /// 返回 holder statement trace 使用的共享内核。
    pub(crate) fn statement_trace_inner(&self) -> Arc<DruidPooledStatementInner> {
        Arc::clone(&self.inner)
    }

    /// 返回该逻辑 Statement 的对象身份。
    pub(crate) fn statement_trace_identity(&self) -> usize {
        Arc::as_ptr(&self.inner) as usize
    }

    /// 返回 Druid 数据源分配的 Statement proxy ID。
    ///
    /// 对应 Java：`WrapperProxy#getId()`；每个数据源从 20000 开始递增。
    #[must_use]
    pub fn id(&self) -> u64 {
        self.inner.id
    }

    /// 返回 Statement proxy attribute 数量。
    #[must_use]
    pub fn attributes_size(&self) -> usize {
        self.inner.attributes.len()
    }

    /// 清空 Statement proxy attributes。
    pub fn clear_attributes(&self) {
        self.inner.attributes.clear();
    }

    /// 返回 Statement proxy attributes 快照。
    #[must_use]
    pub fn attributes(&self) -> std::collections::HashMap<String, ProxyAttributeValue> {
        self.inner.attributes.snapshot()
    }

    /// 返回指定 Statement proxy attribute。
    #[must_use]
    pub fn attribute(&self, key: &str) -> Option<ProxyAttributeValue> {
        self.inner.attributes.get(key)
    }

    /// 保存或覆盖 Statement proxy attribute。
    pub fn put_attribute(
        &self,
        key: impl Into<String>,
        value: ProxyAttributeValue,
    ) -> Option<ProxyAttributeValue> {
        self.inner.attributes.put(key, value)
    }

    /// 返回底层普通语句 SPI。
    ///
    /// 对应 Java：`DruidPooledStatement#getStatement()`。
    pub fn statement(&self) -> &dyn PhysicalStatement {
        self.inner.statement.as_ref()
    }

    /// 返回历史单次查询的最大取回行数。
    ///
    /// 对应 Java：`DruidPooledStatement#getFetchRowPeak()`。
    pub fn fetch_row_peak(&self) -> i32 {
        self.state().fetch_row_peak
    }

    /// 返回已进入异常处理路径的次数。
    pub fn exception_count(&self) -> u64 {
        self.state().exception_count
    }

    /// 返回逻辑语句是否关闭。
    pub fn is_closed(&self) -> bool {
        self.state().closed
    }

    /// 执行查询并返回完整结果行。
    ///
    /// 对应 Java：`DruidPooledStatement#executeQuery(String)`。过滤链、
    /// holder execute 计数与 `ExceptionSorter` 均由池化连接统一处理。
    pub async fn execute_query(
        &mut self,
        connection: &mut DruidPooledConnection,
        sql: &str,
    ) -> Result<Vec<Row>, DruidError> {
        self.ensure_open_for(connection)?;
        self.begin_single_execution(sql, StatementExecuteType::ExecuteQuery);
        let execute_start = Instant::now();
        let result = connection
            .fetch_with_filters(sql, Vec::<Value>::new(), Some(self.id()))
            .await;
        self.record_external_elapsed(execute_start.elapsed(), connection);
        match &result {
            Ok(rows) => {
                let mut state = self.state_mut();
                state.first_result_set = true;
                state.fetch_row_peak = state.fetch_row_peak.max(rows.len() as i32);
                state.update_count = -1;
                state.execute_results = vec![StatementExecuteResult::ResultSet(rows.clone())];
                state.current_result_index = 0;
                state.generated_keys.clear();
            }
            Err(_) => self.record_exception(),
        }
        result
    }

    /// 执行查询并返回池化结果集。
    ///
    /// 对应 Java：`DruidPooledStatement#executeQuery(String)`。结果集持有同一
    /// Statement 身份，关闭时把实际抓取行数回写为 `fetchRowPeak`，Statement
    /// 关闭时也会关闭仍在 trace 中的结果集。
    pub async fn execute_query_result_set(
        &mut self,
        connection: &mut DruidPooledConnection,
        sql: &str,
    ) -> Result<DruidPooledResultSet, DruidError> {
        self.ensure_open_for(connection)?;
        self.begin_single_execution(sql, StatementExecuteType::ExecuteQuery);
        let execute_start = Instant::now();
        let result = connection
            .fetch_result_set_with_filters(sql, Vec::<Value>::new(), Some(self.id()))
            .await;
        self.record_external_elapsed(execute_start.elapsed(), connection);
        let physical = match result {
            Ok(physical) => physical,
            Err(error) => {
                self.record_exception();
                return Err(error);
            }
        };
        {
            let mut state = self.state_mut();
            state.first_result_set = true;
            state.update_count = -1;
            state.execute_results.clear();
            state.current_physical_result_set = Some(Arc::clone(&physical));
            state.current_result_index = 0;
            state.generated_keys.clear();
        }
        self.wrap_result_set(physical)
    }

    /// 把驱动 ResultSet SPI 包装成池化结果集并加入 Statement trace。
    ///
    /// 对应 Java：`new DruidPooledResultSet(this, resultSet)` 以及
    /// `addResultSetTrace`。扩展 Adapter 必须通过该入口保留同 Statement 身份
    /// 与级联关闭，不应直接暴露 raw ResultSet。
    pub fn wrap_result_set(
        &self,
        physical: Arc<dyn PhysicalResultSet>,
    ) -> Result<DruidPooledResultSet, DruidError> {
        let result_set = DruidPooledResultSet::new(Arc::clone(&self.inner), physical)?;
        self.add_result_set_trace(result_set.trace());
        Ok(result_set)
    }

    /// 执行更新并返回执行结果。
    ///
    /// 对应 Java：`DruidPooledStatement#executeUpdate(String)` 及其生成键重载。
    /// 生成键参数由具体 Adapter 的 `ExecResult#last_insert_id` 保留。
    pub async fn execute_update(
        &mut self,
        connection: &mut DruidPooledConnection,
        sql: &str,
    ) -> Result<ExecResult, DruidError> {
        self.ensure_open_for(connection)?;
        self.begin_single_execution(sql, StatementExecuteType::ExecuteUpdate);
        let execute_start = Instant::now();
        let result = connection
            .exec_with_filters(sql, Vec::<Value>::new(), Some(self.id()))
            .await;
        self.record_external_elapsed(execute_start.elapsed(), connection);
        match &result {
            Ok(execution) => {
                let mut state = self.state_mut();
                state.first_result_set = false;
                state.update_count = i64::try_from(execution.rows_affected).unwrap_or(i64::MAX);
                state.execute_results = vec![StatementExecuteResult::Update(execution.clone())];
                state.current_result_index = 0;
                state.generated_keys = Self::generated_key_rows(execution);
            }
            Err(_) => self.record_exception(),
        }
        result
    }

    /// 执行 `Statement#execute(String)`。
    pub async fn execute(
        &mut self,
        connection: &mut DruidPooledConnection,
        sql: &str,
    ) -> Result<bool, DruidError> {
        self.execute_internal(connection, sql, StatementGeneratedKeys::None)
            .await
    }

    /// 执行 `Statement#execute(String, int)`。
    ///
    /// `auto_generated_keys` 原样交给 Adapter；与 Java Druid 一样不在池化层
    /// 预先限制只能为 `RETURN_GENERATED_KEYS` 或 `NO_GENERATED_KEYS`。
    pub async fn execute_with_generated_keys(
        &mut self,
        connection: &mut DruidPooledConnection,
        sql: &str,
        auto_generated_keys: i32,
    ) -> Result<bool, DruidError> {
        self.execute_internal(
            connection,
            sql,
            StatementGeneratedKeys::AutoGeneratedKeys(auto_generated_keys),
        )
        .await
    }

    /// 执行 `Statement#execute(String, int[])`。
    pub async fn execute_with_column_indexes(
        &mut self,
        connection: &mut DruidPooledConnection,
        sql: &str,
        column_indexes: &[i32],
    ) -> Result<bool, DruidError> {
        self.execute_internal(
            connection,
            sql,
            StatementGeneratedKeys::ColumnIndexes(column_indexes.to_vec()),
        )
        .await
    }

    /// 执行 `Statement#execute(String, String[])`。
    pub async fn execute_with_column_names(
        &mut self,
        connection: &mut DruidPooledConnection,
        sql: &str,
        column_names: &[String],
    ) -> Result<bool, DruidError> {
        self.execute_internal(
            connection,
            sql,
            StatementGeneratedKeys::ColumnNames(column_names.to_vec()),
        )
        .await
    }

    /// 返回 generic execute 的当前 ResultSet。
    ///
    /// 对应 Java：`DruidPooledStatement#getResultSet()`；当前不是 ResultSet 时
    /// 返回 `None`，每次非空调用都创建池化 wrapper 并进入 ResultSet open hook。
    pub fn result_set(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<Option<DruidPooledResultSet>, DruidError> {
        self.ensure_open_for(connection)?;
        let getter_result = self.inner.statement.get_result_set();
        self.classify(connection, getter_result)?;
        let (physical, rows) = {
            let state = self.state();
            let physical = state.current_physical_result_set.clone();
            let rows = match state.execute_results.get(state.current_result_index) {
                Some(StatementExecuteResult::ResultSet(rows)) => Some(rows.clone()),
                _ => None,
            };
            (physical, rows)
        };
        if let Some(physical) = physical {
            let result = self.wrap_result_set(physical);
            return self.classify(connection, result).map(Some);
        }
        let Some(rows) = rows else {
            return Ok(None);
        };
        let physical: Arc<dyn PhysicalResultSet> = Arc::new(RowSetResultSet::new(rows));
        let result = self.wrap_result_set(physical);
        self.classify(connection, result).map(Some)
    }

    /// 返回最近一次执行产生的生成键 ResultSet。
    ///
    /// 对应 Java：`DruidPooledStatement#getGeneratedKeys()`。SQLite/xerial 即使
    /// 使用无参数 `execute(String)` 也会暴露最后插入 rowid；无键时返回空结果集。
    pub fn generated_keys(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<DruidPooledResultSet, DruidError> {
        self.ensure_open_for(connection)?;
        let getter_result = self.inner.statement.get_generated_keys();
        self.classify(connection, getter_result)?;
        let rows = self.state().generated_keys.clone();
        let physical: Arc<dyn PhysicalResultSet> = Arc::new(RowSetResultSet::new(rows));
        let result = self.wrap_result_set(physical);
        self.classify(connection, result)
    }

    /// 推进到下一个 JDBC 结果。
    ///
    /// 对应 Java：`DruidPooledStatement#getMoreResults()`。
    pub fn more_results(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<bool, DruidError> {
        self.more_results_internal(connection, None)
    }

    /// 使用 JDBC current-result 常量推进到下一个结果。
    ///
    /// 对应 Java：`getMoreResults(int)`；只接受 1/2/3，非法值在推进和关闭旧
    /// ResultSet 之前返回错误。
    pub fn more_results_with_current(
        &mut self,
        connection: &mut DruidPooledConnection,
        current: i32,
    ) -> Result<bool, DruidError> {
        self.more_results_internal(connection, Some(current))
    }

    /// 添加一条批处理 SQL。
    ///
    /// 对应 Java：`DruidPooledStatement#addBatch(String)`。
    pub fn add_batch(
        &mut self,
        connection: &mut DruidPooledConnection,
        sql: &str,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let rewritten_sql = self.inner.filter_chain.as_ref().map_or_else(
            || Ok(sql.to_owned()),
            |filter_chain| filter_chain.statement_add_batch_sql(sql),
        )?;
        let result = self.inner.statement.add_batch(&rewritten_sql);
        self.classify(connection, result)
    }

    /// 清空批处理；已经关闭时与 Java 一样直接成功。
    ///
    /// 对应 Java：`DruidPooledStatement#clearBatch()`。
    pub fn clear_batch(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        if self.is_closed() {
            return Ok(());
        }
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.clear_batch();
        self.classify(connection, result)
    }

    /// 执行当前批处理快照并返回 JDBC 更新计数数组。
    ///
    /// 对应 Java：`DruidPooledStatement#executeBatch()`。整个批次只进入一次
    /// Filter before/after；任一 SQL 失败时返回携带部分更新计数的
    /// `BatchUpdateException`，并保留 Statement 中的批次供调用方显式清理。
    pub async fn execute_batch(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<Vec<i32>, DruidError> {
        self.ensure_open_for(connection)?;
        self.begin_batch_execution();
        let batch_result = self.inner.statement.batch();
        let batch = self.classify(connection, batch_result)?;
        let merged_sql = batch.join("\n;\n");
        let execute_start = Instant::now();
        let result = connection
            .exec_batch_with_filters(&merged_sql, &batch, Some(self.id()))
            .await;
        self.record_external_elapsed(execute_start.elapsed(), connection);
        match result {
            Ok(update_counts) => {
                // Java StatementProxyImpl 只有单元素数组才更新 getUpdateCount。
                let mut state = self.state_mut();
                state.first_result_set = false;
                state.update_count = if update_counts.len() == 1 {
                    i64::from(update_counts[0])
                } else {
                    -1
                };
                Ok(update_counts)
            }
            Err(error) => {
                self.record_exception();
                Err(error)
            }
        }
    }

    /// 返回最近一次更新计数；查询或尚未执行时返回 `-1`。
    ///
    /// 对应 Java：`DruidPooledStatement#getUpdateCount()`。
    pub fn update_count(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i64, DruidError> {
        self.ensure_open_for(connection)?;
        let getter_result = self.inner.statement.get_update_count();
        self.classify(connection, getter_result)?;
        Ok(self.state().update_count)
    }

    /// 返回结果集类型。
    pub fn result_set_type(&self, connection: &DruidPooledConnection) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        Ok(self.inner.statement.options().result_set_type)
    }

    /// 返回结果集并发模式。
    pub fn result_set_concurrency(
        &self,
        connection: &DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        Ok(self.inner.statement.options().result_set_concurrency)
    }

    /// 返回结果集保持性。
    pub fn result_set_holdability(
        &self,
        connection: &DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        Ok(self.inner.statement.options().result_set_holdability)
    }

    /// 返回最大字段大小。
    pub fn max_field_size(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.max_field_size();
        self.classify(connection, result)
    }

    /// 设置最大字段大小。
    pub fn set_max_field_size(
        &mut self,
        connection: &mut DruidPooledConnection,
        max: i32,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.set_max_field_size(max);
        self.classify(connection, result)
    }

    /// 返回最大结果行数。
    pub fn max_rows(&mut self, connection: &mut DruidPooledConnection) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.max_rows();
        self.classify(connection, result)
    }

    /// 设置最大结果行数。
    pub fn set_max_rows(
        &mut self,
        connection: &mut DruidPooledConnection,
        max: i32,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.set_max_rows(max);
        self.classify(connection, result)
    }

    /// 设置 escape processing。
    pub fn set_escape_processing(
        &mut self,
        connection: &mut DruidPooledConnection,
        enabled: bool,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.set_escape_processing(enabled);
        self.classify(connection, result)
    }

    /// 返回查询超时秒数。
    pub fn query_timeout(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.query_timeout();
        self.classify(connection, result)
    }

    /// 设置查询超时秒数。
    pub fn set_query_timeout(
        &mut self,
        connection: &mut DruidPooledConnection,
        seconds: i32,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.set_query_timeout(seconds);
        self.classify(connection, result)
    }

    /// 取消当前执行。
    pub fn cancel(&mut self, connection: &mut DruidPooledConnection) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.cancel();
        self.classify(connection, result)
    }

    /// 返回语句警告链。
    ///
    /// 对应 Java：`DruidPooledStatement#getWarnings()`。调用从位置 0 穿过
    /// `statement_getWarnings` Filter around-chain。
    pub async fn warnings(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<Option<SqlWarning>, DruidError> {
        self.ensure_open_for(connection)?;
        let filter_chain = self.inner.filter_chain.clone();
        let result = match filter_chain {
            Some(filter_chain) => {
                filter_chain
                    .statement_warnings(self.inner.statement.as_ref())
                    .await
            }
            None => self.inner.statement.warnings(),
        };
        self.classify(connection, result)
    }

    /// 清除语句警告。
    ///
    /// 对应 Java：`DruidPooledStatement#clearWarnings()`。
    pub async fn clear_warnings(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let filter_chain = self.inner.filter_chain.clone();
        let result = match filter_chain {
            Some(filter_chain) => {
                filter_chain
                    .statement_clear_warnings(self.inner.statement.as_ref())
                    .await
            }
            None => self.inner.statement.clear_warnings(),
        };
        self.classify(connection, result)
    }

    /// 设置游标名称。
    pub fn set_cursor_name(
        &mut self,
        connection: &mut DruidPooledConnection,
        name: &str,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.set_cursor_name(name);
        self.classify(connection, result)
    }

    /// 设置抓取方向。
    pub fn set_fetch_direction(
        &mut self,
        connection: &mut DruidPooledConnection,
        direction: i32,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.set_fetch_direction(direction);
        self.classify(connection, result)
    }

    /// 返回抓取方向。
    pub fn fetch_direction(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.fetch_direction();
        self.classify(connection, result)
    }

    /// 设置抓取行数。
    pub fn set_fetch_size(
        &mut self,
        connection: &mut DruidPooledConnection,
        rows: i32,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.set_fetch_size(rows);
        self.classify(connection, result)
    }

    /// 返回抓取行数。
    pub fn fetch_size(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i32, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.fetch_size();
        self.classify(connection, result)
    }

    /// 设置 poolable；与 Java Druid 一致，传 `false` 返回不支持。
    pub fn set_poolable(
        &mut self,
        connection: &mut DruidPooledConnection,
        poolable: bool,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.set_poolable(poolable);
        self.classify(connection, result)
    }

    /// 普通 Statement 不进入 PreparedStatement 缓存。
    pub fn is_poolable(&self) -> bool {
        self.inner.statement.is_poolable()
    }

    /// 设置执行完成后关闭。
    pub fn close_on_completion(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.close_on_completion();
        self.classify(connection, result)
    }

    /// 返回执行完成后关闭状态。
    pub fn is_close_on_completion(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let result = self.inner.statement.is_close_on_completion();
        self.classify(connection, result)
    }

    /// 关闭逻辑与物理语句。
    ///
    /// 对应 Java：`DruidPooledStatement#close()`；重复关闭无副作用。
    pub fn close_with_connection(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        if self.is_closed() {
            return Ok(());
        }
        self.ensure_open_for(connection)?;
        self.clear_result_sets();
        let result = self.inner.statement.close();
        let result = self.classify(connection, result);
        if result.is_ok() {
            self.state_mut().closed = true;
            if let Some(stats) = self.inner.stats_collector.as_ref() {
                stats.statement_stat().increment_statement_close_counter();
            }
            connection.remove_statement_trace(self.statement_trace_identity());
            if let Some(filter_chain) = &self.inner.filter_chain {
                filter_chain
                    .after_statement_close_with_identity(self.inner.connection_id, self.inner.id)?;
            }
        }
        result
    }

    fn ensure_open_for(&self, connection: &DruidPooledConnection) -> Result<(), DruidError> {
        if self.is_closed()
            || self.inner.statement.is_closed()
            || !self.inner.lease_active.load(Ordering::Acquire)
            || !connection.is_same_open_lease(&self.inner.lease_active)
        {
            Err(DruidError::Other("statement is closed".to_string()))
        } else {
            Ok(())
        }
    }

    async fn execute_internal(
        &mut self,
        connection: &mut DruidPooledConnection,
        sql: &str,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        self.begin_single_execution(sql, StatementExecuteType::Execute);
        let execute_start = Instant::now();
        let result = connection
            .execute_with_filters(sql, generated_keys, Some(self.id()))
            .await;
        self.record_external_elapsed(execute_start.elapsed(), connection);
        match result {
            Ok(results) => Ok(self.complete_external_execute(results)),
            Err(error) => {
                self.record_exception();
                Err(error)
            }
        }
    }

    fn more_results_internal(
        &mut self,
        connection: &mut DruidPooledConnection,
        current: Option<i32>,
    ) -> Result<bool, DruidError> {
        self.ensure_open_for(connection)?;
        let physical_result = self.inner.statement.get_more_results(current);
        self.classify(connection, physical_result)?;

        // Java wrapper 在底层推进成功后，无论 current 常量为何，都直接把最后一个
        // DruidPooledResultSet wrapper 标为 closed，不触发显式 close Filter。
        if let Some(last) = self.state().result_set_trace.last() {
            last.mark_closed_by_more_results();
        }

        let mut state = self.state_mut();
        if state.current_result_index < state.execute_results.len() {
            state.current_result_index = state.current_result_index.saturating_add(1);
        }
        state.current_physical_result_set = None;
        state.update_count = state
            .execute_results
            .get(state.current_result_index)
            .map_or(-1, StatementExecuteResult::update_count);
        Ok(state
            .execute_results
            .get(state.current_result_index)
            .is_some_and(StatementExecuteResult::is_result_set))
    }

    fn generated_key_rows(execution: &ExecResult) -> Vec<Row> {
        execution
            .last_insert_id
            .map(|key| Row::new(vec![Value::Int(key)]))
            .into_iter()
            .collect()
    }

    fn invalidate_result_sets_for_execute(&self) {
        let state = self.state();
        for result_set in &state.result_set_trace {
            if !result_set.is_closed() {
                result_set.mark_closed_by_more_results();
            }
        }
    }

    pub(crate) fn begin_external_execution(&self, sql: &str, execute_type: StatementExecuteType) {
        self.invalidate_result_sets_for_execute();
        let mut state = self.state_mut();
        state.last_sql = Some(sql.to_owned());
        state.last_execute_type = Some(execute_type);
        state.last_execute_started_at = Some(Instant::now());
        state.last_execute_elapsed = None;
        state.first_result_set = false;
        state.execute_results.clear();
        state.current_physical_result_set = None;
        state.current_result_index = 0;
        state.update_count = -1;
        state.generated_keys.clear();
    }

    fn begin_single_execution(&self, sql: &str, execute_type: StatementExecuteType) {
        self.begin_external_execution(sql, execute_type);
    }

    pub(crate) fn begin_batch_execution(&self) {
        self.invalidate_result_sets_for_execute();
        let mut state = self.state_mut();
        state.last_execute_type = Some(StatementExecuteType::ExecuteBatch);
        state.last_execute_started_at = Some(Instant::now());
        state.last_execute_elapsed = None;
        state.first_result_set = false;
        state.execute_results.clear();
        state.current_physical_result_set = None;
        state.current_result_index = 0;
        state.update_count = -1;
        state.generated_keys.clear();
    }

    /// 返回最近一次执行使用的 SQL，供 ResultSet 关闭时回写 SQL 统计。
    pub fn last_sql(&self) -> Option<String> {
        self.state().last_sql.clone()
    }

    /// 返回最近一次执行耗时，供 ResultSet hold 统计组合。
    pub fn last_execute_elapsed(&self) -> Option<Duration> {
        self.state().last_execute_elapsed
    }

    /// 返回最近一次执行入口类型。
    #[must_use]
    pub fn last_execute_type(&self) -> Option<StatementExecuteType> {
        self.state().last_execute_type
    }

    /// 返回最近一次执行开始至当前的单调耗时。
    #[must_use]
    pub fn last_execute_start_elapsed(&self) -> Option<Duration> {
        self.state()
            .last_execute_started_at
            .map(|started_at| started_at.elapsed())
    }

    /// 返回最近一次 generic/query 执行的首结果是否为 ResultSet。
    #[must_use]
    pub fn is_first_result_set(&self) -> bool {
        self.state().first_result_set
    }

    /// 保存外部 Statement/PreparedStatement 执行耗时。
    pub(crate) fn record_external_elapsed(
        &self,
        elapsed: Duration,
        connection: &DruidPooledConnection,
    ) {
        self.state_mut().last_execute_elapsed = Some(elapsed);
        if let Some(sql) = connection.last_execute_sql() {
            self.state_mut().last_sql = Some(sql.to_owned());
        }
    }

    /// 保存外部 `PreparedStatement` generic execute 的有序结果。
    ///
    /// `PreparedStatement` 在 Java 中继承 `Statement` 的结果状态机；Rust 组合该
    /// 基类状态，确保两类语句的 update count、generated keys 与多结果推进一致。
    pub(crate) fn complete_external_execute(&self, results: Vec<StatementExecuteResult>) -> bool {
        let first_is_result_set = results
            .first()
            .is_some_and(StatementExecuteResult::is_result_set);
        let update_count = results
            .first()
            .map_or(-1, StatementExecuteResult::update_count);
        let generated_key_rows = results
            .iter()
            .find_map(|result| match result {
                StatementExecuteResult::Update(execution) if execution.last_insert_id.is_some() => {
                    Some(Self::generated_key_rows(execution))
                }
                StatementExecuteResult::Update(_) | StatementExecuteResult::ResultSet(_) => None,
            })
            .unwrap_or_default();
        let mut state = self.state_mut();
        state.first_result_set = first_is_result_set;
        state.execute_results = results;
        state.current_physical_result_set = None;
        state.current_result_index = 0;
        state.update_count = update_count;
        state.generated_keys = generated_key_rows;
        first_is_result_set
    }

    /// 保存 `PreparedStatement` 查询结果到共享 `Statement` 状态机。
    pub(crate) fn complete_external_query(&self, rows: Vec<Row>) {
        let mut state = self.state_mut();
        state.first_result_set = true;
        state.fetch_row_peak = state
            .fetch_row_peak
            .max(i32::try_from(rows.len()).unwrap_or(i32::MAX));
        state.update_count = -1;
        state.execute_results = vec![StatementExecuteResult::ResultSet(rows)];
        state.current_physical_result_set = None;
        state.current_result_index = 0;
        state.generated_keys.clear();
    }

    /// 保存 `PreparedStatement` 返回的物理结果集到共享 `Statement` 状态机。
    pub(crate) fn complete_external_query_result_set(
        &self,
        result_set: Arc<dyn PhysicalResultSet>,
    ) {
        let mut state = self.state_mut();
        state.first_result_set = true;
        state.update_count = -1;
        state.execute_results.clear();
        state.current_physical_result_set = Some(result_set);
        state.current_result_index = 0;
        state.generated_keys.clear();
    }

    /// 保存 `PreparedStatement` 更新结果到共享 `Statement` 状态机。
    pub(crate) fn complete_external_update(&self, execution: &ExecResult) {
        let mut state = self.state_mut();
        state.first_result_set = false;
        state.update_count = i64::try_from(execution.rows_affected).unwrap_or(i64::MAX);
        state.execute_results = vec![StatementExecuteResult::Update(execution.clone())];
        state.current_physical_result_set = None;
        state.current_result_index = 0;
        state.generated_keys = Self::generated_key_rows(execution);
    }

    /// 保存 PreparedStatement batch 的首结果状态。
    pub(crate) fn complete_external_batch(&self, update_counts: &[i32]) {
        let mut state = self.state_mut();
        state.first_result_set = false;
        state.update_count = if update_counts.len() == 1 {
            i64::from(update_counts[0])
        } else {
            -1
        };
        state.execute_results.clear();
        state.current_physical_result_set = None;
        state.current_result_index = 0;
        state.generated_keys.clear();
    }

    /// 关闭嵌入在 `PreparedStatement` 中的 `Statement` 基类状态与所有 `ResultSet`。
    pub(crate) fn close_embedded_base(&self) {
        if self.is_closed() {
            return;
        }
        self.clear_result_sets();
        let _ = self.inner.statement.close();
        self.state_mut().closed = true;
        if let Some(stats) = self.inner.stats_collector.as_ref() {
            stats.statement_stat().increment_statement_close_counter();
        }
        if let Some(filter_chain) = &self.inner.filter_chain {
            let _ = filter_chain
                .after_statement_close_with_identity(self.inner.connection_id, self.inner.id);
        }
    }

    fn classify<T>(
        &mut self,
        connection: &mut DruidPooledConnection,
        result: Result<T, DruidError>,
    ) -> Result<T, DruidError> {
        let result = connection.classify_result(result);
        if result.is_err() {
            self.record_exception();
        }
        result
    }

    pub(crate) fn record_exception(&mut self) {
        let mut state = self.state_mut();
        state.exception_count = state.exception_count.saturating_add(1);
    }

    pub(crate) fn record_fetch_row_count(&self, fetch_row_count: i32) {
        let mut state = self.state_mut();
        state.fetch_row_peak = state.fetch_row_peak.max(fetch_row_count);
    }

    pub(crate) fn record_result_set_exception(&self) {
        let mut state = self.state_mut();
        state.exception_count = state.exception_count.saturating_add(1);
    }

    /// 同时更新数据源与当前 SQL 的 Blob 打开计数。
    ///
    /// 对应 Java：`StatFilter#blobOpenAfter(...)`。
    pub(crate) fn record_blob_open(&self) {
        let Some(stats) = self.inner.stats_collector.as_ref() else {
            return;
        };
        stats.record_blob_open();
        if let Some(sql_stat) = self
            .last_sql()
            .as_deref()
            .and_then(|sql| stats.sql_merger.active_stat_for_sql(sql))
        {
            sql_stat.increment_blob_open_count();
        }
    }

    /// 同时更新数据源与当前 SQL 的 Clob/NClob 打开计数。
    ///
    /// 对应 Java：`StatFilter#clobOpenAfter(...)`。
    pub(crate) fn record_clob_open(&self) {
        let Some(stats) = self.inner.stats_collector.as_ref() else {
            return;
        };
        stats.record_clob_open();
        if let Some(sql_stat) = self
            .last_sql()
            .as_deref()
            .and_then(|sql| stats.sql_merger.active_stat_for_sql(sql))
        {
            sql_stat.increment_clob_open_count();
        }
    }

    fn add_result_set_trace(&self, result_set: DruidPooledResultSetTrace) {
        let mut state = self.state_mut();
        if let Some(last) = state.result_set_trace.last_mut() {
            if last.is_closed() {
                *last = result_set;
                return;
            }
        }
        state.result_set_trace.push(result_set);
    }

    fn clear_result_sets(&self) {
        let result_sets = {
            let mut state = self.state_mut();
            std::mem::take(&mut state.result_set_trace)
        };
        for result_set in result_sets {
            if !result_set.is_closed() {
                result_set.close();
            }
            self.record_fetch_row_count(result_set.fetch_row_count());
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, DruidPooledStatementState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn state_mut(&self) -> std::sync::MutexGuard<'_, DruidPooledStatementState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Wrapper for DruidPooledStatement {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_wrapper_for(&self, iface: Option<TypeId>) -> bool {
        iface.is_some_and(|iface| {
            iface == TypeId::of::<Self>()
                || iface == TypeId::of::<dyn PhysicalStatement>()
                || self.inner.statement.as_any().type_id() == iface
        })
    }

    fn unwrap(&self, iface: Option<TypeId>) -> Option<Unwrapped<'_>> {
        let iface = iface?;
        if iface == TypeId::of::<Self>() {
            return Some(Unwrapped::Object(self));
        }
        if iface == TypeId::of::<dyn PhysicalStatement>() {
            return Some(Unwrapped::Statement(self.inner.statement.as_ref()));
        }
        (self.inner.statement.as_any().type_id() == iface)
            .then(|| Unwrapped::Object(self.inner.statement.as_any()))
    }
}

impl std::fmt::Debug for DruidPooledStatement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidPooledStatement")
            .field("statement", &self.inner.statement.as_any().type_id())
            .field("closed", &self.is_closed())
            .field("fetch_row_peak", &self.fetch_row_peak())
            .field("exception_count", &self.exception_count())
            .field("update_count", &self.state().update_count)
            .finish()
    }
}
