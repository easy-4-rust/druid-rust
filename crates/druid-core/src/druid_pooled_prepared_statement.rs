//! 对外池化预编译语句。
//!
//! 对应 Java：
//! `com.alibaba.druid.pool.DruidPooledPreparedStatement`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/pool/DruidPooledPreparedStatement.java`。

use crate::{
    DruidError, DruidPooledConnection, ExecResult, PreparedStatementCacheStats,
    PreparedStatementHolder, PreparedStatementKey, PreparedStatementPool, Row, Value,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// 借用池化连接执行并在关闭时复用物理 PreparedStatement 的逻辑语句。
///
/// 语句句柄不独占连接借用，因此同一连接可同时持有多个 PreparedStatement，
/// 保留 Java `inUseCount`、`sharePreparedStatements` 和 LRU 替换语义。执行时
/// 显式传入原 `DruidPooledConnection`，关闭/Drop 则通过共享 statement pool
/// 归还物理语句。
pub struct DruidPooledPreparedStatement {
    holder: Arc<PreparedStatementHolder>,
    pooled: bool,
    statement_pool: Option<Arc<Mutex<PreparedStatementPool>>>,
    stats: Arc<PreparedStatementCacheStats>,
    lease_active: Arc<AtomicBool>,
    exception_count: u64,
    fetch_row_peak: i32,
    closed: bool,
}

impl std::fmt::Debug for DruidPooledPreparedStatement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidPooledPreparedStatement")
            .field("key", self.holder.key())
            .field("pooled", &self.pooled)
            .field("exception_count", &self.exception_count)
            .field("closed", &self.closed)
            .finish()
    }
}

impl DruidPooledPreparedStatement {
    pub(crate) fn new(
        holder: Arc<PreparedStatementHolder>,
        pooled: bool,
        statement_pool: Option<Arc<Mutex<PreparedStatementPool>>>,
        stats: Arc<PreparedStatementCacheStats>,
        lease_active: Arc<AtomicBool>,
    ) -> Self {
        Self {
            holder,
            pooled,
            statement_pool,
            stats,
            lease_active,
            exception_count: 0,
            fetch_row_peak: -1,
            closed: false,
        }
    }

    /// 返回完整 PreparedStatement 缓存键。
    pub fn key(&self) -> &PreparedStatementKey {
        self.holder.key()
    }

    /// 返回内部 statement holder。
    ///
    /// 对应 Java：`getPreparedStatementHolder()`。
    pub fn prepared_statement_holder(&self) -> &PreparedStatementHolder {
        &self.holder
    }

    /// 返回逻辑语句是否关闭。
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// 执行更新类 PreparedStatement。
    ///
    /// 参数 `params` 与 Java `setXxx` 后的参数快照等价地一次性交给驱动。
    pub async fn exec(
        &mut self,
        connection: &mut DruidPooledConnection,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        self.ensure_open_for(connection)?;
        let statement = self.holder.statement().clone();
        let result = connection
            .exec_prepared_with_filters(statement.as_ref(), params)
            .await;
        if result.is_err() {
            self.exception_count = self.exception_count.saturating_add(1);
        }
        result
    }

    /// 执行查询类 PreparedStatement。
    pub async fn fetch(
        &mut self,
        connection: &mut DruidPooledConnection,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        self.ensure_open_for(connection)?;
        let statement = self.holder.statement().clone();
        let result = connection
            .fetch_prepared_with_filters(statement.as_ref(), params)
            .await;
        if let Ok(rows) = &result {
            self.fetch_row_peak = self.fetch_row_peak.max(rows.len() as i32);
        } else {
            self.exception_count = self.exception_count.saturating_add(1);
        }
        result
    }

    /// 关闭逻辑语句并按 Java 分支放回缓存或删除物理语句。
    pub fn close(&mut self) -> Result<(), DruidError> {
        if self.closed {
            return Ok(());
        }

        if self.pooled {
            if let Err(error) = self.holder.statement().clear_parameters() {
                self.exception_count = self.exception_count.saturating_add(1);
                return Err(error);
            }
            if let Err(error) = self.holder.statement().clear_batch() {
                self.exception_count = self.exception_count.saturating_add(1);
                return Err(error);
            }
        }

        self.finish();
        Ok(())
    }

    pub(crate) fn ensure_open(&self) -> Result<(), DruidError> {
        if self.closed || !self.lease_active.load(Ordering::Acquire) {
            Err(DruidError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    pub(crate) fn record_exception(&mut self) {
        self.exception_count = self.exception_count.saturating_add(1);
    }

    fn ensure_open_for(&self, connection: &DruidPooledConnection) -> Result<(), DruidError> {
        self.ensure_open()?;
        if connection.is_same_open_lease(&self.lease_active) {
            Ok(())
        } else {
            Err(DruidError::ConnectionDiscarded)
        }
    }

    fn finish(&mut self) {
        if self.closed {
            return;
        }
        self.holder.decrement_in_use_count();
        if !self.lease_active.load(Ordering::Acquire) {
            // 连接已经归还：禁止旧 wrapper 重新进入下一次租约的 cache。
            if !self.holder.statement().is_closed() {
                let _ = self.holder.statement().close();
                if self.holder.hit_count() > 0 {
                    self.stats.record_cache_delete();
                } else {
                    self.stats.record_close();
                }
            }
            self.closed = true;
            return;
        }
        if self.pooled && self.exception_count == 0 {
            self.holder.set_fetch_row_peak(self.fetch_row_peak);
            if let Some(statement_pool) = &self.statement_pool {
                statement_pool
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .put(self.holder.clone());
            } else {
                let _ = self.holder.statement().close();
                self.stats.record_close();
            }
        } else if self.pooled {
            if let Some(statement_pool) = &self.statement_pool {
                statement_pool
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(&self.holder);
            } else {
                let _ = self.holder.statement().close();
                self.stats.record_close();
            }
        } else {
            let _ = self.holder.statement().close();
            self.stats.record_close();
        }
        self.closed = true;
    }
}

impl Drop for DruidPooledPreparedStatement {
    fn drop(&mut self) {
        self.finish();
    }
}
