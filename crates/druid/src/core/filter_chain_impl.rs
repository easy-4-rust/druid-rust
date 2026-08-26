//! Druid filter-chain implementation for the canonical standard surface.

use super::connection_info::ConnectionInfo;
use super::standard_result_set_filter_chain::StandardResultSetFilterChain;
use super::{
    AfterFilter, BatchExecContext, BeforeFilter, ClobProxy, Connection, ConnectionEvent,
    ConnectionEventContext, ConnectionFactory, DruidError, DruidPooledConnection, ExecContext,
    ExecResult, PoolState, RdbcClob, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcString,
    RdbcWriter, ResultSetFilter, ResultSetFilterContext, StatementEvent, StatementEventContext,
};
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

    pub(crate) async fn standard_connection_meta_data_before(
        &self,
        connection_id: u64,
    ) -> Result<(), DruidError> {
        for (index, filter) in self.before.iter().enumerate() {
            if let Err(error) = filter
                .standard_connection_get_meta_data_before(connection_id)
                .await
            {
                let failure = Err(error.clone());
                for completed in self.before[..index].iter().rev() {
                    completed
                        .standard_connection_get_meta_data_after(connection_id, &failure)
                        .await?;
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub(crate) async fn standard_connection_meta_data_after(
        &self,
        connection_id: u64,
        result: &Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        for filter in self.before.iter().rev() {
            filter
                .standard_connection_get_meta_data_after(connection_id, result)
                .await?;
        }
        Ok(())
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
