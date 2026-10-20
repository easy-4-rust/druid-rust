//! Druid filter-chain implementation for the canonical standard surface.

use super::connection_info::ConnectionInfo;
use super::standard_logical_policy::StandardLogicalPolicy;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_registry::StandardResourceRegistry;
use super::standard_result_set_filter_chain::StandardResultSetFilterChain;
use super::{
    AfterFilter, BatchExecContext, BeforeFilter, ClobProxy, Connection, ConnectionEvent,
    ConnectionEventContext, ConnectionFactory, DruidError, DruidPooledConnection, ExecContext,
    ExecResult, PoolState, RdbcClob, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcString,
    RdbcWriter, RegisteredLob, ResultSetFilter, ResultSetFilterContext, SqlWarning,
    StandardConnectionLobFilterChain, StandardConnectionWarningFilterChain, StatementEvent,
    StatementEventContext,
};
use druid_rdbc::resource::{Blob, Clob, NClob};
use druid_rdbc::result_set::ColumnIndex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Filter storage and ordered dispatch.
#[derive(Clone)]
pub struct FilterChainImpl {
    before: Vec<Arc<dyn BeforeFilter>>,
    after: Vec<Arc<dyn AfterFilter>>,
    result_set: Vec<Arc<dyn ResultSetFilter>>,
    filter_class_names: Vec<String>,
}

#[async_trait::async_trait]
pub trait DataSourceConnectionProvider: Send + Sync {
    fn data_source_name(&self) -> &str;
    fn data_source_state(&self) -> PoolState;
    async fn get_connection_direct_for_filter(
        &self,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError>;
}

pub struct DataSourceGetConnectionFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    provider: &'a dyn DataSourceConnectionProvider,
}

impl<'a> DataSourceGetConnectionFilterChain<'a> {
    fn new(
        filters: &'a [Arc<dyn BeforeFilter>],
        provider: &'a dyn DataSourceConnectionProvider,
    ) -> Self {
        Self {
            filters,
            position: 0,
            provider,
        }
    }

    pub fn data_source_name(&self) -> &str {
        self.provider.data_source_name()
    }

    pub fn data_source_state(&self) -> PoolState {
        self.provider.data_source_state()
    }

    pub async fn data_source_get_connection(
        &mut self,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            filter.data_source_get_connection(self, max_wait).await
        } else {
            self.provider
                .get_connection_direct_for_filter(max_wait)
                .await
        }
    }
}

pub struct DataSourceReleaseConnectionFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
}

impl<'a> DataSourceReleaseConnectionFilterChain<'a> {
    fn new(filters: &'a [Arc<dyn BeforeFilter>]) -> Self {
        Self {
            filters,
            position: 0,
        }
    }

    pub async fn data_source_recycle(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            filter
                .data_source_release_connection(self, connection)
                .await
        } else {
            connection
                .recycle_from_data_source_filter()
                .await
                .map_err(DruidError::from)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConnectionCloseContext {
    pub connection_id: u64,
    pub connection_age: Duration,
}

pub struct ConnectionCloseFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    factory: &'a dyn ConnectionFactory,
    connection: &'a mut Box<dyn Connection>,
    context: ConnectionCloseContext,
}

impl<'a> ConnectionCloseFilterChain<'a> {
    fn new(
        filters: &'a [Arc<dyn BeforeFilter>],
        factory: &'a dyn ConnectionFactory,
        connection: &'a mut Box<dyn Connection>,
        context: ConnectionCloseContext,
    ) -> Self {
        Self {
            filters,
            position: 0,
            factory,
            connection,
            context,
        }
    }

    pub const fn context(&self) -> ConnectionCloseContext {
        self.context
    }

    pub async fn connection_close(&mut self) -> Result<(), DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            filter.connection_close(self).await
        } else {
            self.factory
                .close(self.connection)
                .await
                .map_err(DruidError::from)
        }
    }
}

pub struct ConnectionOpenResult {
    connection_info: ConnectionInfo,
    connection_id: u64,
}

impl ConnectionOpenResult {
    pub(crate) const fn new(connection_info: ConnectionInfo, connection_id: u64) -> Self {
        Self {
            connection_info,
            connection_id,
        }
    }

    #[must_use]
    pub const fn connection_id(&self) -> u64 {
        self.connection_id
    }

    pub(crate) fn into_parts(self) -> (ConnectionInfo, u64) {
        (self.connection_info, self.connection_id)
    }
}

pub struct ConnectionOpenFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    factory: &'a dyn ConnectionFactory,
    login_timeout_seconds: i32,
    next_connection_id: &'a mut (dyn FnMut() -> u64 + Send),
}

impl<'a> ConnectionOpenFilterChain<'a> {
    fn new(
        filters: &'a [Arc<dyn BeforeFilter>],
        factory: &'a dyn ConnectionFactory,
        login_timeout_seconds: i32,
        next_connection_id: &'a mut (dyn FnMut() -> u64 + Send),
    ) -> Self {
        Self {
            filters,
            position: 0,
            factory,
            login_timeout_seconds,
            next_connection_id,
        }
    }

    pub async fn connection_connect(
        &mut self,
        properties: &mut HashMap<String, String>,
    ) -> Result<ConnectionOpenResult, DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            return filter.connection_connect(self, properties).await;
        }
        let started = Instant::now();
        let future = self.factory.create_with_properties(properties);
        let connection = if self.login_timeout_seconds > 0 {
            tokio::time::timeout(
                Duration::from_secs(self.login_timeout_seconds as u64),
                future,
            )
            .await
            .map_err(|_| DruidError::LoginTimeout)??
        } else {
            future.await?
        };
        Ok(ConnectionOpenResult::new(
            ConnectionInfo::connected(connection, started),
            (self.next_connection_id)(),
        ))
    }
}

pub struct ClobFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    wrapper: &'a dyn ClobProxy,
}

impl<'a> ClobFilterChain<'a> {
    fn new(filters: &'a [Arc<dyn BeforeFilter>], wrapper: &'a dyn ClobProxy) -> Self {
        Self {
            filters,
            position: 0,
            wrapper,
        }
    }

    pub fn wrapper(&self) -> &dyn ClobProxy {
        self.wrapper
    }

    fn next_filter(&mut self) -> Option<Arc<dyn BeforeFilter>> {
        let filter = self.filters.get(self.position).cloned();
        if filter.is_some() {
            self.position += 1;
        }
        filter
    }

    pub async fn clob_length(&mut self) -> Result<i64, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_length(self).await
        } else {
            self.wrapper
                .raw_clob()
                .length()
                .await
                .map_err(DruidError::from)
        }
    }

    pub async fn clob_get_sub_string(
        &mut self,
        position: i64,
        length: i32,
    ) -> Result<RdbcString, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_get_sub_string(self, position, length).await
        } else {
            self.wrapper
                .raw_clob()
                .get_sub_string(position, length)
                .await
                .map_err(DruidError::from)
        }
    }

    pub async fn clob_get_character_stream(&mut self) -> Result<RdbcReader, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_get_character_stream(self).await
        } else {
            self.wrapper
                .raw_clob()
                .get_character_stream()
                .await
                .map_err(DruidError::from)
        }
    }

    pub async fn clob_get_ascii_stream(&mut self) -> Result<RdbcInputStream, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_get_ascii_stream(self).await
        } else {
            self.wrapper
                .raw_clob()
                .get_ascii_stream()
                .await
                .map_err(DruidError::from)
        }
    }

    pub async fn clob_position_string(
        &mut self,
        pattern: &RdbcString,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_position_string(self, pattern, start).await
        } else {
            self.wrapper
                .raw_clob()
                .position_string(pattern, start)
                .await
                .map_err(DruidError::from)
        }
    }

    pub async fn clob_position_clob(
        &mut self,
        pattern: &RdbcClob,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_position_clob(self, pattern, start).await
        } else {
            self.wrapper
                .raw_clob()
                .position_clob(pattern, start)
                .await
                .map_err(DruidError::from)
        }
    }

    pub async fn clob_set_string(
        &mut self,
        position: i64,
        value: &RdbcString,
    ) -> Result<i32, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_set_string(self, position, value).await
        } else {
            self.wrapper
                .raw_clob()
                .set_string(position, value)
                .await
                .map_err(DruidError::from)
        }
    }

    pub async fn clob_set_string_range(
        &mut self,
        position: i64,
        value: &RdbcString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter
                .clob_set_string_range(self, position, value, offset, length)
                .await
        } else {
            self.wrapper
                .raw_clob()
                .set_string_range(position, value, offset, length)
                .await
                .map_err(DruidError::from)
        }
    }

    pub async fn clob_set_ascii_stream(
        &mut self,
        position: i64,
    ) -> Result<RdbcOutputStream, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_set_ascii_stream(self, position).await
        } else {
            self.wrapper
                .raw_clob()
                .set_ascii_stream(position)
                .await
                .map_err(DruidError::from)
        }
    }

    pub async fn clob_set_character_stream(
        &mut self,
        position: i64,
    ) -> Result<RdbcWriter, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_set_character_stream(self, position).await
        } else {
            self.wrapper
                .raw_clob()
                .set_character_stream(position)
                .await
                .map_err(DruidError::from)
        }
    }

    pub async fn clob_truncate(&mut self, length: i64) -> Result<(), DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_truncate(self, length).await
        } else {
            self.wrapper
                .raw_clob()
                .truncate(length)
                .await
                .map_err(DruidError::from)
        }
    }

    pub async fn clob_free(&mut self) -> Result<(), DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_free(self).await
        } else {
            self.wrapper
                .raw_clob()
                .free()
                .await
                .map_err(DruidError::from)
        }
    }

    pub async fn clob_get_character_stream_range(
        &mut self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter
                .clob_get_character_stream_range(self, position, length)
                .await
        } else {
            self.wrapper
                .raw_clob()
                .get_character_stream_range(position, length)
                .await
                .map_err(DruidError::from)
        }
    }
}

impl FilterChainImpl {
    pub fn new() -> Self {
        Self {
            before: Vec::new(),
            after: Vec::new(),
            result_set: Vec::new(),
            filter_class_names: Vec::new(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.before.is_empty() && self.after.is_empty() && self.result_set.is_empty()
    }

    pub fn add_before(&mut self, filter: Arc<dyn BeforeFilter>) {
        self.before.push(filter);
    }

    pub fn add_after(&mut self, filter: Arc<dyn AfterFilter>) {
        self.after.push(filter);
    }

    pub fn add_result_set(&mut self, filter: Arc<dyn ResultSetFilter>) {
        self.result_set.push(filter);
    }

    pub fn add_filter<T>(&mut self, filter: Arc<T>)
    where
        T: BeforeFilter + AfterFilter + ResultSetFilter + 'static,
    {
        self.filter_class_names
            .push(std::any::type_name::<T>().to_owned());
        self.before
            .push(Arc::clone(&filter) as Arc<dyn BeforeFilter>);
        self.after.push(Arc::clone(&filter) as Arc<dyn AfterFilter>);
        self.result_set.push(filter);
    }

    pub(crate) fn add_registered_filter(
        &mut self,
        class_name: String,
        before: Arc<dyn BeforeFilter>,
        after: Arc<dyn AfterFilter>,
        result_set: Arc<dyn ResultSetFilter>,
    ) {
        self.filter_class_names.push(class_name);
        self.before.push(before);
        self.after.push(after);
        self.result_set.push(result_set);
    }

    #[must_use]
    pub fn contains_filter_class_name(&self, class_name: &str) -> bool {
        self.filter_class_names
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(class_name))
    }

    #[must_use]
    pub fn filter_class_names(&self) -> &[String] {
        &self.filter_class_names
    }

    #[must_use]
    pub fn before_count(&self) -> usize {
        self.before.len()
    }

    #[must_use]
    pub fn after_count(&self) -> usize {
        self.after.len()
    }

    #[must_use]
    pub fn result_set_count(&self) -> usize {
        self.result_set.len()
    }

    pub(crate) async fn init_filters(&self) -> Result<(), DruidError> {
        for filter in &self.before {
            filter.init().await?;
        }
        Ok(())
    }

    pub(crate) fn configure_filters(
        &self,
        properties: &HashMap<String, String>,
        system_properties: &HashMap<String, String>,
    ) -> Result<(), DruidError> {
        for filter in &self.before {
            filter.config_from_properties(properties)?;
            filter.config_from_system_properties(system_properties)?;
        }
        Ok(())
    }

    pub(crate) async fn destroy_filters(&self) {
        for filter in &self.before {
            if let Err(error) = filter.destroy().await {
                tracing::warn!(%error, filter = filter.name(), "destroy filter error");
            }
        }
    }

    pub async fn data_source_get_connection(
        &self,
        provider: &dyn DataSourceConnectionProvider,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        DataSourceGetConnectionFilterChain::new(&self.before, provider)
            .data_source_get_connection(max_wait)
            .await
    }

    pub async fn data_source_release_connection(
        &self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        DataSourceReleaseConnectionFilterChain::new(&self.before)
            .data_source_recycle(connection)
            .await
    }

    pub async fn connection_close(
        &self,
        factory: &dyn ConnectionFactory,
        connection: &mut Box<dyn Connection>,
        connection_id: u64,
        connection_age: Duration,
    ) -> Result<(), DruidError> {
        ConnectionCloseFilterChain::new(
            &self.before,
            factory,
            connection,
            ConnectionCloseContext {
                connection_id,
                connection_age,
            },
        )
        .connection_close()
        .await
    }

    /// 通过标准连接按注册顺序读取警告，允许过滤器包围或短路。
    pub async fn connection_get_warnings(
        &self,
        connection: &mut dyn Connection,
    ) -> Result<Option<SqlWarning>, DruidError> {
        StandardConnectionWarningFilterChain::new(&self.before, connection)
            .connection_get_warnings()
            .await
    }

    /// 通过标准连接按注册顺序清除警告，允许过滤器包围或短路。
    pub async fn connection_clear_warnings(
        &self,
        connection: &mut dyn Connection,
    ) -> Result<(), DruidError> {
        StandardConnectionWarningFilterChain::new(&self.before, connection)
            .connection_clear_warnings()
            .await
    }

    /// 按注册顺序检查标准语句属性读取；参数为 getter 事件，返回首个过滤器错误。
    pub(crate) fn standard_statement_property_read(
        &self,
        event: &super::StatementPropertyEvent,
    ) -> Result<(), DruidError> {
        for filter in &self.before {
            filter.standard_statement_property_read(event)?;
        }
        Ok(())
    }

    /// 包围标准语句批次清除；参数为底层标准语句，返回过滤或驱动结果。
    pub(crate) fn standard_statement_clear_batch(
        &self,
        statement: &mut dyn druid_rdbc::statement::Statement,
    ) -> Result<(), DruidError> {
        super::StandardStatementClearBatchFilterChain::new(&self.before, statement)
            .statement_clear_batch()
    }

    /// 按注册顺序读取标准语句警告；参数为驱动语句，返回完整警告链。
    pub(crate) fn standard_statement_get_warnings(
        &self,
        statement: &mut dyn druid_rdbc::statement::Statement,
    ) -> Result<Option<SqlWarning>, DruidError> {
        super::StandardStatementWarningFilterChain::new(&self.before, statement)
            .statement_get_warnings()
    }

    /// 按注册顺序清除标准语句警告；参数为驱动语句，返回操作结果。
    pub(crate) fn standard_statement_clear_warnings(
        &self,
        statement: &mut dyn druid_rdbc::statement::Statement,
    ) -> Result<(), DruidError> {
        super::StandardStatementWarningFilterChain::new(&self.before, statement)
            .statement_clear_warnings()
    }

    /// 在标准连接上以注册顺序创建 Blob，返回已登记结果。
    pub(crate) async fn connection_create_blob(
        &self,
        connection: &mut dyn Connection,
        registry: Arc<StandardResourceRegistry>,
        admission: Arc<StandardResourceAdmission>,
        policy: Arc<StandardLogicalPolicy>,
    ) -> Result<RegisteredLob<dyn Blob>, DruidError> {
        StandardConnectionLobFilterChain::new(&self.before, connection, registry, policy, admission)
            .connection_create_blob()
            .await
    }

    /// 在标准连接上以注册顺序创建 Clob，返回已登记结果。
    pub(crate) async fn connection_create_clob(
        &self,
        connection: &mut dyn Connection,
        registry: Arc<StandardResourceRegistry>,
        admission: Arc<StandardResourceAdmission>,
        policy: Arc<StandardLogicalPolicy>,
    ) -> Result<RegisteredLob<dyn Clob>, DruidError> {
        StandardConnectionLobFilterChain::new(&self.before, connection, registry, policy, admission)
            .connection_create_clob()
            .await
    }

    /// 在标准连接上以注册顺序创建 NClob，返回已登记结果。
    pub(crate) async fn connection_create_n_clob(
        &self,
        connection: &mut dyn Connection,
        registry: Arc<StandardResourceRegistry>,
        admission: Arc<StandardResourceAdmission>,
        policy: Arc<StandardLogicalPolicy>,
    ) -> Result<RegisteredLob<dyn NClob>, DruidError> {
        StandardConnectionLobFilterChain::new(&self.before, connection, registry, policy, admission)
            .connection_create_n_clob()
            .await
    }

    pub async fn connection_open(
        &self,
        factory: &dyn ConnectionFactory,
        properties: &mut HashMap<String, String>,
        login_timeout_seconds: i32,
        next_connection_id: &mut (dyn FnMut() -> u64 + Send),
    ) -> Result<ConnectionOpenResult, DruidError> {
        ConnectionOpenFilterChain::new(
            &self.before,
            factory,
            login_timeout_seconds,
            next_connection_id,
        )
        .connection_connect(properties)
        .await
    }

    pub async fn clob_length(&self, wrapper: &dyn ClobProxy) -> Result<i64, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_length()
            .await
    }

    pub async fn clob_get_sub_string(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
        length: i32,
    ) -> Result<RdbcString, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_get_sub_string(position, length)
            .await
    }

    pub async fn clob_get_character_stream(
        &self,
        wrapper: &dyn ClobProxy,
    ) -> Result<RdbcReader, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_get_character_stream()
            .await
    }

    pub async fn clob_get_ascii_stream(
        &self,
        wrapper: &dyn ClobProxy,
    ) -> Result<RdbcInputStream, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_get_ascii_stream()
            .await
    }

    pub async fn clob_get_character_stream_range(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_get_character_stream_range(position, length)
            .await
    }

    pub async fn clob_position_string(
        &self,
        wrapper: &dyn ClobProxy,
        pattern: &RdbcString,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_position_string(pattern, start)
            .await
    }

    pub async fn clob_position_clob(
        &self,
        wrapper: &dyn ClobProxy,
        pattern: &RdbcClob,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_position_clob(pattern, start)
            .await
    }

    pub async fn clob_set_string(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
        value: &RdbcString,
    ) -> Result<i32, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_set_string(position, value)
            .await
    }

    pub async fn clob_set_string_range(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
        value: &RdbcString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_set_string_range(position, value, offset, length)
            .await
    }

    pub async fn clob_set_ascii_stream(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
    ) -> Result<RdbcOutputStream, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_set_ascii_stream(position)
            .await
    }

    pub async fn clob_set_character_stream(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
    ) -> Result<RdbcWriter, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_set_character_stream(position)
            .await
    }

    pub async fn clob_truncate(
        &self,
        wrapper: &dyn ClobProxy,
        length: i64,
    ) -> Result<(), DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_truncate(length)
            .await
    }

    pub async fn clob_free(&self, wrapper: &dyn ClobProxy) -> Result<(), DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_free()
            .await
    }

    pub async fn before_execute(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        for (index, filter) in self.before.iter().enumerate() {
            if let Err(error) = filter.before(context).await {
                for completed in self.before[..index].iter().rev() {
                    let _ = completed.before_execute_error(context, &error).await;
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub async fn after_execute(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let mut first_error = None;
        for filter in self.after.iter().rev() {
            if let Err(error) = filter.after(context, result, elapsed).await {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub async fn before_batch(&self, context: &mut BatchExecContext<'_>) -> Result<(), DruidError> {
        for (index, filter) in self.before.iter().enumerate() {
            if let Err(error) = filter.before_batch(context).await {
                for completed in self.before[..index].iter().rev() {
                    let _ = completed.before_batch_error(context, &error).await;
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub async fn after_batch(
        &self,
        context: &BatchExecContext<'_>,
        result: &Result<Vec<i32>, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let mut first_error = None;
        for filter in self.after.iter().rev() {
            if let Err(error) = filter.after_batch(context, result, elapsed).await {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub fn prepare_statement_sql(&self, sql: &str) -> Result<String, DruidError> {
        self.before
            .iter()
            .try_fold(sql.to_owned(), |current, filter| {
                filter.prepare_statement_sql(&current)
            })
    }

    pub fn statement_add_batch_sql(&self, sql: &str) -> Result<String, DruidError> {
        self.before
            .iter()
            .try_fold(sql.to_owned(), |current, filter| {
                filter.statement_add_batch_sql(&current)
            })
    }

    pub async fn before_connection_event_with_identity(
        &self,
        connection_id: u64,
        event: &ConnectionEvent,
    ) -> Result<(), DruidError> {
        let context = ConnectionEventContext {
            connection_id,
            event,
        };
        for filter in &self.before {
            filter.on_connection_event_context(&context).await?;
        }
        Ok(())
    }

    pub async fn after_connection_event_with_identity(
        &self,
        connection_id: u64,
        event: &ConnectionEvent,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let context = ConnectionEventContext {
            connection_id,
            event,
        };
        for filter in self.after.iter().rev() {
            filter
                .after_connection_event_context(&context, elapsed)
                .await?;
        }
        Ok(())
    }

    pub async fn after_statement_event_with_identity(
        &self,
        connection_id: u64,
        statement_id: u64,
        event: &StatementEvent,
    ) -> Result<(), DruidError> {
        let context = StatementEventContext {
            connection_id,
            statement_id,
            event,
        };
        for filter in self.before.iter().rev() {
            filter.on_statement_event_context(&context).await?;
        }
        Ok(())
    }

    pub fn after_statement_close_with_identity(
        &self,
        connection_id: u64,
        statement_id: u64,
    ) -> Result<(), DruidError> {
        let event = StatementEvent::Close;
        let context = StatementEventContext {
            connection_id,
            statement_id,
            event: &event,
        };
        for filter in self.before.iter().rev() {
            filter.on_statement_close_context(&context)?;
        }
        Ok(())
    }

    /// 从首个过滤器包围完整标准 metadata 获取；参数为驱动连接及逻辑 ID。
    pub(crate) async fn standard_connection_meta_data(
        &self,
        connection: &mut dyn Connection,
        connection_id: u64,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, DruidError> {
        super::StandardConnectionMetaDataFilterChain::new(&self.before, connection, connection_id)
            .connection_get_meta_data()
            .await
    }

    pub(crate) fn standard_result_set_open_after_with_proxy(
        &self,
        context: &mut super::StandardResultSetOpenContext<'_>,
    ) -> Result<(), DruidError> {
        for filter in self.result_set.iter().rev() {
            filter.standard_result_set_open_after_with_proxy(context)?;
        }
        Ok(())
    }

    /// 包围原导航参数；普通默认链不读取 row，池化路径显式启用实际行号跟踪。
    /// 参数为同一驱动、上下文、导航和跟踪开关；返回结果及可空真实行号。
    /// 对应 Java: FilterChainImpl 的默认导航与 DruidPooledResultSet 的游标状态治理。
    pub(crate) async fn standard_result_set_navigation(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        navigation: druid_rdbc::result_set::ResultSetNavigation,
        capture_row: bool,
    ) -> Result<(bool, Option<i32>), DruidError> {
        let mut chain = if capture_row {
            StandardResultSetFilterChain::new_with_navigation_row(
                &self.result_set,
                result_set,
                context,
            )
        } else {
            StandardResultSetFilterChain::new(&self.result_set, result_set, context)
        };
        let present = chain.result_set_navigation(navigation).await?;
        Ok((present, chain.navigation_row()))
    }

    /// 按注册顺序包围原行操作；参数为上下文、操作及惰性驱动末端，返回原结果。
    /// 对应 Java: FilterChainImpl 的七个 ResultSet 行操作。
    pub(crate) fn standard_result_set_row_operation(
        &self,
        context: &ResultSetFilterContext,
        operation: super::ResultSetRowOperation,
        terminal: &mut dyn FnMut() -> Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        fn invoke(
            filters: &[Arc<dyn super::ResultSetFilter>],
            context: &ResultSetFilterContext,
            operation: super::ResultSetRowOperation,
            terminal: &mut dyn FnMut() -> Result<(), DruidError>,
        ) -> Result<(), DruidError> {
            match filters.split_first() {
                Some((filter, remaining)) => {
                    filter.standard_result_set_row_operation(context, operation, &mut || {
                        invoke(remaining, context, operation, terminal)
                    })
                }
                None => terminal(),
            }
        }
        invoke(&self.result_set, context, operation, terminal)
    }

    /// 按注册顺序读取结果集警告；参数为上下文及驱动末端，返回完整警告链。
    pub(crate) fn standard_result_set_get_warnings(
        &self,
        context: &ResultSetFilterContext,
        terminal: &mut dyn FnMut() -> Result<Option<SqlWarning>, DruidError>,
    ) -> Result<Option<SqlWarning>, DruidError> {
        fn invoke(
            filters: &[Arc<dyn super::ResultSetFilter>],
            context: &ResultSetFilterContext,
            terminal: &mut dyn FnMut() -> Result<Option<SqlWarning>, DruidError>,
        ) -> Result<Option<SqlWarning>, DruidError> {
            match filters.split_first() {
                Some((filter, remaining)) => filter
                    .standard_result_set_get_warnings(context, &mut || {
                        invoke(remaining, context, terminal)
                    }),
                None => terminal(),
            }
        }
        invoke(&self.result_set, context, terminal)
    }

    /// 按注册顺序清除结果集警告；参数为上下文及驱动末端，返回原结果。
    pub(crate) fn standard_result_set_clear_warnings(
        &self,
        context: &ResultSetFilterContext,
        terminal: &mut dyn FnMut() -> Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        fn invoke(
            filters: &[Arc<dyn super::ResultSetFilter>],
            context: &ResultSetFilterContext,
            terminal: &mut dyn FnMut() -> Result<(), DruidError>,
        ) -> Result<(), DruidError> {
            match filters.split_first() {
                Some((filter, remaining)) => filter
                    .standard_result_set_clear_warnings(context, &mut || {
                        invoke(remaining, context, terminal)
                    }),
                None => terminal(),
            }
        }
        invoke(&self.result_set, context, terminal)
    }

    /// 按注册顺序包围属性调用；参数为原请求和末端，保留短路及错误反向退出。
    /// 对应 Java: FilterChainImpl 的 ResultSet 属性方法。
    pub(crate) fn standard_result_set_property(
        &self,
        context: &ResultSetFilterContext,
        request: super::ResultSetPropertyOperation<'_>,
        terminal: &mut dyn FnMut() -> Result<super::ResultSetPropertyValue, DruidError>,
    ) -> Result<super::ResultSetPropertyValue, DruidError> {
        fn invoke(
            filters: &[Arc<dyn super::ResultSetFilter>],
            context: &ResultSetFilterContext,
            request: super::ResultSetPropertyOperation<'_>,
            terminal: &mut dyn FnMut() -> Result<super::ResultSetPropertyValue, DruidError>,
        ) -> Result<super::ResultSetPropertyValue, DruidError> {
            match filters.split_first() {
                Some((filter, remaining)) => {
                    filter.standard_result_set_property(context, request, &mut || {
                        invoke(remaining, context, request, terminal)
                    })
                }
                None => terminal(),
            }
        }
        invoke(&self.result_set, context, request, terminal)
    }

    pub(crate) async fn standard_result_set_next(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_next()
            .await
    }

    pub(crate) async fn standard_result_set_close(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_close()
            .await
    }

    pub(crate) fn standard_result_set_value(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: ColumnIndex,
    ) -> Result<super::Value, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_value(column)
    }

    /// 保留 value around-hook 顺序，末端使用标准驱动的 getString 语义。
    pub(crate) fn standard_result_set_string_value(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: ColumnIndex,
    ) -> Result<super::Value, DruidError> {
        StandardResultSetFilterChain::new_string(&self.result_set, result_set, context)
            .result_set_value(column)
    }

    /// 包围逻辑创建语句查询；参数为上下文和原所有者末端，返回原或替换标准句柄。
    /// 对应 Java: FilterChainImpl#resultSet_getStatement；不借用或公开原生游标。
    pub(crate) fn standard_result_set_statement(
        &self,
        context: &ResultSetFilterContext,
        terminal: &mut dyn FnMut() -> Result<
            Option<Box<dyn druid_rdbc::statement::Statement>>,
            DruidError,
        >,
    ) -> Result<Option<Box<dyn druid_rdbc::statement::Statement>>, DruidError> {
        fn invoke(
            filters: &[Arc<dyn super::ResultSetFilter>],
            context: &ResultSetFilterContext,
            terminal: &mut dyn FnMut() -> Result<
                Option<Box<dyn druid_rdbc::statement::Statement>>,
                DruidError,
            >,
        ) -> Result<Option<Box<dyn druid_rdbc::statement::Statement>>, DruidError> {
            if let Some((filter, remaining)) = filters.split_first() {
                filter.standard_result_set_statement(context, &mut || {
                    invoke(remaining, context, terminal)
                })
            } else {
                terminal()
            }
        }
        invoke(&self.result_set, context, terminal)
    }

    /// 从首个Filter包围完整对象重载，返回未扁平化的驱动或短路对象。
    /// 参数为驱动、上下文、治理列、原请求和原列名；对应 Java: FilterChainImpl#getObject。
    pub(crate) fn standard_result_set_object(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        request: &super::ResultSetObjectRead<'_>,
        label: Option<&str>,
    ) -> Result<druid_rdbc::types::RdbcObject, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_object(column, label, request)
    }

    /// 从首个Filter包围资源对象请求，返回完整原生句柄或精确错误。
    /// 参数为驱动、上下文、治理列、原请求和标签。对应 Java: FilterChainImpl#resultSet_getXxx。
    pub(crate) fn standard_result_set_resource(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        request: &druid_rdbc::result_set::ResultSetRead,
        label: Option<&str>,
    ) -> Result<druid_rdbc::types::RdbcObject, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_resource(column, label, request)
    }

    /// 包围字节流请求，默认保留原生重载；参数为驱动和完整读取身份，返回原流。
    /// 对应 Java: FilterChainImpl#resultSet_getAsciiStream/getUnicodeStream/getBinaryStream。
    pub(crate) fn standard_result_set_input_stream(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        kind: druid_rdbc::result_set::ResultSetStreamKind,
    ) -> Result<Option<druid_rdbc::resource::RdbcInputStream>, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_input_stream(column, label, kind)
    }

    /// 包围字符流请求，不预读或复制Reader；参数为驱动和完整重载，返回原Reader。
    /// 对应 Java: FilterChainImpl#resultSet_getCharacterStream/getNCharacterStream。
    pub(crate) fn standard_result_set_reader(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        national: bool,
    ) -> Result<Option<druid_rdbc::resource::RdbcReader>, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_reader(column, label, national)
    }

    /// 强类型标量保留完整读取 around-hook；默认委托继续既有 value hooks。
    /// 参数含驱动、上下文、列、请求和可空原列名，返回过滤后的值。
    /// 对应 Java: FilterChainImpl#resultSet_getBigDecimal/getDate/getTime/getTimestamp。
    pub(crate) fn standard_result_set_typed_value(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        request: &druid_rdbc::result_set::ResultSetRead,
        label: Option<&str>,
    ) -> Result<super::Value, DruidError> {
        StandardResultSetFilterChain::new_typed(
            &self.result_set,
            result_set,
            context,
            request,
            label,
        )
        .result_set_read(column)
    }

    /// 从position0执行完整下标更新链；参数为驱动、上下文、列和原描述符，返回执行结果。
    /// 对应 Java: FilterChainImpl#resultSet_updateXxx(int, ...)。
    pub(crate) fn standard_result_set_update(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: ColumnIndex,
        update: druid_rdbc::result_set::ResultSetUpdate,
    ) -> Result<(), DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_update(column, update)
    }

    /// 从position0执行完整原列名更新链；参数为驱动、上下文、列名和描述符，返回结果。
    /// 对应 Java: FilterChainImpl#resultSet_updateXxx(String, ...)。
    pub(crate) fn standard_result_set_update_by_label(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        update: druid_rdbc::result_set::ResultSetUpdate,
    ) -> Result<(), DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_update_by_label(label, update)
    }

    pub(crate) fn standard_result_set_meta_data(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<Arc<dyn druid_rdbc::metadata::ResultSetMetaData>, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_meta_data()
    }

    pub(crate) fn standard_result_set_was_null(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_was_null()
    }
}

impl Default for FilterChainImpl {
    fn default() -> Self {
        Self::new()
    }
}
