use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetRead};
use druid_rdbc::types::{RdbcObject, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// 保留原顺序合同的原生调用日志、关闭错误及整数32，不替代过滤链。
/// 对应 Java: java.sql.ResultSet#next/close/getInt。
pub(super) struct FilterOrderResultSetProbe {
    calls: Arc<Mutex<Vec<String>>>,
    closed: AtomicBool,
    pub(super) next_count: AtomicU64,
    close_error: bool,
}

impl FilterOrderResultSetProbe {
    /// 参数为原调用日志与关闭失败开关，返回独立的原生游标探针。
    pub(super) fn new(calls: Arc<Mutex<Vec<String>>>, close_error: bool) -> Self {
        Self {
            calls,
            closed: AtomicBool::new(false),
            next_count: AtomicU64::new(0),
            close_error,
        }
    }
    fn record(&self, operation: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(operation);
    }
}
druid_rdbc::impl_standard_wrapper!(FilterOrderResultSetProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for FilterOrderResultSetProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.record("physical:next".to_owned());
        self.next_count.fetch_add(1, Ordering::AcqRel);
        Ok(true)
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.record("physical:close".to_owned());
        self.closed.store(true, Ordering::Release);
        if self.close_error {
            Err(RdbcError::DriverError("close failed".to_owned()))
        } else {
            Ok(())
        }
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed.load(Ordering::Acquire))
    }
    fn read_object(
        &self,
        column: ColumnIndex,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        match request {
            ResultSetRead::Int => {
                self.record(format!("physical:int:{}", column.get()));
                Ok(RdbcObject::Scalar(Value::Int(32)))
            }
            _ => Err(RdbcError::DriverError(
                "unexpected filter order read descriptor".to_owned(),
            )),
        }
    }
}
