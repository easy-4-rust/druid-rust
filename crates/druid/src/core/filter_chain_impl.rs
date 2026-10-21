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

mod chain_a;
mod chain_b;

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

impl Default for FilterChainImpl {
    fn default() -> Self {
        Self::new()
    }
}
