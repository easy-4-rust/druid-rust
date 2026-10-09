//! 尚未提供总行数的受控 raw driver fixture；不作为真实数据库或 Task 7 streaming 证据。

use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{ExecutionResult, GeneratedKeys, Statement, StatementOptions};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// 仅在 driver 边界注入关闭失败/取消，并观察实际释放次数。
#[derive(Default)]
pub struct CursorCloseProbe {
    pub cursor_close_attempts: AtomicUsize,
    pub statement_closes: AtomicUsize,
    pub fail_once: AtomicBool,
    pub block_once: AtomicBool,
    pub entered: tokio::sync::Notify,
    pub release: tokio::sync::Notify,
}

/// 模拟总行数未知的 driver 连接；只提供本测试需要的 owned statement。
pub struct UnknownCountConnection {
    next_calls: Arc<AtomicUsize>,
    close_probe: Arc<CursorCloseProbe>,
    closed: bool,
}

impl UnknownCountConnection {
    /// 创建连接并关联调用者持有的 raw next 观察器。
    pub fn new(next_calls: Arc<AtomicUsize>) -> Self {
        Self {
            next_calls,
            close_probe: Arc::new(CursorCloseProbe::default()),
            closed: false,
        }
    }

    /// 注入指定 raw 关闭探针，供失败和调用者取消回归测试使用。
    pub fn with_close_probe(
        next_calls: Arc<AtomicUsize>,
        close_probe: Arc<CursorCloseProbe>,
    ) -> Self {
        Self {
            next_calls,
            close_probe,
            closed: false,
        }
    }
}

druid_rdbc::impl_standard_wrapper!(UnknownCountConnection => Connection);

#[async_trait::async_trait]
impl Connection for UnknownCountConnection {
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        if self.closed {
            return Err(RdbcError::ConnectionDiscarded);
        }
        Ok(Box::new(UnknownCountStatement {
            next_calls: self.next_calls.clone(),
            close_probe: self.close_probe.clone(),
            closed: false,
        }))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }

    async fn prepare_statement(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        if self.closed {
            return Err(RdbcError::ConnectionDiscarded);
        }
        Ok(Box::new(UnknownCountStatement {
            next_calls: self.next_calls.clone(),
            close_probe: self.close_probe.clone(),
            closed: false,
        }))
    }
}

/// 不读取 cursor 内容的 raw generic execution。
struct UnknownCountStatement {
    next_calls: Arc<AtomicUsize>,
    close_probe: Arc<CursorCloseProbe>,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(UnknownCountStatement => PreparedStatement);

#[async_trait::async_trait]
impl Statement for UnknownCountStatement {
    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn execute(
        &mut self,
        _sql: &str,
        _keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        if self.closed {
            return Err(RdbcError::ConnectionDiscarded);
        }
        Ok(vec![ExecutionResult::ResultSet(Box::new(
            UnknownCountCursor {
                next_calls: self.next_calls.clone(),
                close_probe: self.close_probe.clone(),
                closed: false,
            },
        ))])
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        if !self.closed {
            self.close_probe
                .statement_closes
                .fetch_add(1, Ordering::SeqCst);
        }
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

#[async_trait::async_trait]
impl druid_rdbc::statement::PreparedStatement for UnknownCountStatement {
    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.execute("SELECT delayed_value", GeneratedKeys::None)
            .await
    }
}

/// 延迟读取的 raw cursor，沿用 canonical 未知行数默认方法。
struct UnknownCountCursor {
    next_calls: Arc<AtomicUsize>,
    close_probe: Arc<CursorCloseProbe>,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(UnknownCountCursor => ResultSet);

#[async_trait::async_trait]
impl ResultSet for UnknownCountCursor {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        if self.closed {
            return Err(RdbcError::ConnectionDiscarded);
        }
        Ok(self.next_calls.fetch_add(1, Ordering::SeqCst) == 0)
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(vec![])))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        if self.closed {
            return Ok(());
        }
        self.close_probe
            .cursor_close_attempts
            .fetch_add(1, Ordering::SeqCst);
        if self.close_probe.fail_once.swap(false, Ordering::SeqCst) {
            return Err(RdbcError::DriverError(
                "cursor close failed once".to_owned(),
            ));
        }
        if self.close_probe.block_once.swap(false, Ordering::SeqCst) {
            self.close_probe.entered.notify_one();
            self.close_probe.release.notified().await;
        }
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}
