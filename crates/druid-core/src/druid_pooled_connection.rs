//! 对外池化连接。

use crate::error::DruidError;
use crate::exec_result::ExecResult;
use crate::filter::{ConnectionEvent, ExecContext};
use crate::filter_chain::FilterChain;
use crate::physical_connection::PhysicalConnection;
use crate::physical_connection_capabilities::PhysicalConnectionCapabilities;
use crate::row::Row;
use crate::savepoint::Savepoint;
use crate::value::Value;
use std::sync::Arc;
use std::time::Instant;

/// 对外池化连接。
///
/// 对应 Java: `com.alibaba.druid.pool.DruidPooledConnection`。
/// 该对象拥有一次连接租约，对外暴露连接语义；底层只依赖
/// `PhysicalConnection`。显式关闭和 `Drop` 都通过同一条回收路径，
/// 并由 `FnOnce` 回调保证物理连接最多归还一次。
pub struct DruidPooledConnection {
    physical_connection: Option<Box<dyn PhysicalConnection>>,
    id: u64,
    data_source: String,
    filter_chain: Option<Arc<FilterChain>>,
    return_connection: Option<
        Box<dyn FnOnce(Box<dyn PhysicalConnection>, u64) + Send>,
    >,
    recycled: bool,
}

impl std::fmt::Debug for DruidPooledConnection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidPooledConnection")
            .field("id", &self.id)
            .field("data_source", &self.data_source)
            .field("has_physical_connection", &self.physical_connection.is_some())
            .field("recycled", &self.recycled)
            .finish()
    }
}

impl DruidPooledConnection {
    /// 创建不带过滤上下文的池化连接。
    ///
    /// 参数 `physical_connection` 为底层物理连接，`id` 为连接 ID，
    /// `return_connection` 在关闭或析构时接收并归还连接。
    pub fn new(
        physical_connection: Box<dyn PhysicalConnection>,
        id: u64,
        return_connection: Box<
            dyn FnOnce(Box<dyn PhysicalConnection>, u64) + Send,
        >,
    ) -> Self {
        Self::with_context(
            physical_connection,
            id,
            String::new(),
            None,
            return_connection,
        )
    }

    /// 创建带数据源和过滤链上下文的池化连接。
    ///
    /// 参数 `data_source` 会原样进入前置与后置过滤器，
    /// `filter_chain` 为该数据源装配的过滤链。
    pub fn with_context(
        physical_connection: Box<dyn PhysicalConnection>,
        id: u64,
        data_source: String,
        filter_chain: Option<Arc<FilterChain>>,
        return_connection: Box<
            dyn FnOnce(Box<dyn PhysicalConnection>, u64) + Send,
        >,
    ) -> Self {
        Self {
            physical_connection: Some(physical_connection),
            id,
            data_source,
            filter_chain,
            return_connection: Some(return_connection),
            recycled: false,
        }
    }

    /// 返回连接 ID。
    pub fn id(&self) -> u64 {
        self.id
    }

    /// 返回数据源名称。
    pub fn data_source(&self) -> &str {
        &self.data_source
    }

    /// 返回连接是否已经归还。
    pub fn is_recycled(&self) -> bool {
        self.recycled
    }

    /// 返回底层物理连接的可变引用；连接已归还时返回 `None`。
    pub fn physical_connection_mut(
        &mut self,
    ) -> Option<&mut (dyn PhysicalConnection + 'static)> {
        self.physical_connection.as_deref_mut()
    }

    /// 显式归还连接。
    ///
    /// 多次调用与随后发生的 `Drop` 都不会重复归还。
    pub fn recycle(mut self) {
        self.recycle_once();
    }

    fn physical_mut(&mut self) -> Result<&mut Box<dyn PhysicalConnection>, DruidError> {
        self.physical_connection
            .as_mut()
            .ok_or(DruidError::ConnectionDiscarded)
    }

    fn recycle_once(&mut self) {
        if self.recycled {
            return;
        }

        self.recycled = true;
        if let (Some(physical_connection), Some(return_connection)) = (
            self.physical_connection.take(),
            self.return_connection.take(),
        ) {
            return_connection(physical_connection, self.id);
        }
    }

    async fn before_connection_event(
        &mut self,
        event: &ConnectionEvent,
    ) -> Result<(), DruidError> {
        match &self.filter_chain {
            Some(filter_chain) => filter_chain.before_connection_event(event).await,
            None => Ok(()),
        }
    }

    async fn exec_with_filters(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            sql,
            params: &params,
            data_source: &data_source,
            start,
            fingerprint: None,
        };

        if let Some(filter_chain) = &filter_chain {
            filter_chain.before_execute(&mut context).await?;
        }

        // 过滤器需要在 after 阶段观察同一组参数，因此只克隆驱动调用所需所有权。
        let result = self.physical_mut()?.exec(sql, params.clone()).await;
        if let Some(filter_chain) = &filter_chain {
            filter_chain
                .after_execute(&context, &result, start.elapsed())
                .await;
        }
        result
    }

    async fn fetch_with_filters(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            sql,
            params: &params,
            data_source: &data_source,
            start,
            fingerprint: None,
        };

        if let Some(filter_chain) = &filter_chain {
            filter_chain.before_execute(&mut context).await?;
        }

        let result = self.physical_mut()?.fetch(sql, params.clone()).await;
        if let Some(filter_chain) = &filter_chain {
            let filter_result = match &result {
                Ok(rows) => Ok(ExecResult {
                    rows_affected: 0,
                    last_insert_id: None,
                    row_count: Some(rows.len() as u64),
                }),
                Err(error) => Err(error.clone()),
            };
            filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await;
        }
        result
    }
}

#[async_trait::async_trait]
impl PhysicalConnection for DruidPooledConnection {
    async fn exec(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        self.exec_with_filters(sql, params).await
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        self.fetch_with_filters(sql, params).await
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        self.physical_mut()?.begin().await
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::Commit).await?;
        self.physical_mut()?.commit().await
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::Rollback).await?;
        self.physical_mut()?.rollback().await
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        self.physical_mut()?.rollback_to(savepoint).await
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, DruidError> {
        self.physical_mut()?.set_savepoint().await
    }

    async fn set_savepoint_named(&mut self, name: &str) -> Result<Savepoint, DruidError> {
        self.physical_mut()?.set_savepoint_named(name).await
    }

    async fn release_savepoint(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        self.physical_mut()?.release_savepoint(savepoint).await
    }

    async fn abort(&mut self) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::Abort).await?;
        let result = self.physical_mut()?.abort().await;
        self.recycle_once();
        result
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::IsValid).await?;
        self.physical_mut()?.ping().await
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        if self.recycled {
            return Ok(());
        }

        self.before_connection_event(&ConnectionEvent::Close).await?;
        let filter_chain = self.filter_chain.clone();
        self.recycle_once();
        if let Some(filter_chain) = filter_chain {
            filter_chain.after_connection_close().await;
        }
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.recycled
    }

    fn capabilities(&self) -> PhysicalConnectionCapabilities {
        self.physical_connection
            .as_ref()
            .map_or_else(PhysicalConnectionCapabilities::default, |connection| {
                connection.capabilities()
            })
    }

    fn auto_commit(&self) -> bool {
        self.physical_connection
            .as_ref()
            .map_or(true, |connection| connection.auto_commit())
    }

    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::SetAutoCommit(auto_commit))
            .await?;
        self.physical_mut()?.set_auto_commit(auto_commit).await
    }

    fn read_only(&self) -> bool {
        self.physical_connection
            .as_ref()
            .is_some_and(|connection| connection.read_only())
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::SetReadOnly(read_only))
            .await?;
        self.physical_mut()?.set_read_only(read_only).await
    }

    fn transaction_isolation(&self) -> u8 {
        self.physical_connection
            .as_ref()
            .map_or(2, |connection| connection.transaction_isolation())
    }

    async fn set_transaction_isolation(&mut self, level: u8) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::SetTransactionIsolation(level))
            .await?;
        self.physical_mut()?.set_transaction_isolation(level).await
    }

    fn catalog(&self) -> Option<&str> {
        self.physical_connection
            .as_ref()
            .and_then(|connection| connection.catalog())
    }

    async fn set_catalog(&mut self, catalog: &str) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::SetCatalog(catalog.to_string()))
            .await?;
        self.physical_mut()?.set_catalog(catalog).await
    }

    fn schema(&self) -> Option<&str> {
        self.physical_connection
            .as_ref()
            .and_then(|connection| connection.schema())
    }

    async fn set_schema(&mut self, schema: &str) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::SetSchema(schema.to_string()))
            .await?;
        self.physical_mut()?.set_schema(schema).await
    }

    fn driver_name(&self) -> &str {
        self.physical_connection
            .as_ref()
            .map_or("", |connection| connection.driver_name())
    }
}

impl Drop for DruidPooledConnection {
    fn drop(&mut self) {
        self.recycle_once();
    }
}
