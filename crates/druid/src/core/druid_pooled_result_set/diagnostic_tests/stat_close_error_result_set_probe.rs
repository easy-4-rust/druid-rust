use crate::stats::StatsCollector;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::ResultSet;
use std::sync::{Arc, Mutex};

/// 在原生关闭入口观察已提交的统计，随后返回原关闭错误。
/// 对应 Java: com.alibaba.druid.filter.stat.StatFilter#resultSet_close。
pub(super) struct StatCloseErrorResultSetProbe {
    calls: Arc<Mutex<Vec<String>>>,
    collector: Arc<StatsCollector>,
    observations: Arc<Mutex<Vec<String>>>,
    closed: bool,
}

impl StatCloseErrorResultSetProbe {
    /// 创建探针；参数为原生日志、共享统计和入口观察，返回开放结果集。
    pub(super) fn new(
        calls: Arc<Mutex<Vec<String>>>,
        collector: Arc<StatsCollector>,
        observations: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self {
            calls,
            collector,
            observations,
            closed: false,
        }
    }
}

druid_rdbc::impl_standard_wrapper!(StatCloseErrorResultSetProbe => ResultSet);

#[async_trait::async_trait]
impl ResultSet for StatCloseErrorResultSetProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "stat_close_error_next",
        })
    }

    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(Vec::new())))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.calls.lock().unwrap().push("physical:close".to_owned());
        self.closed = true;
        // 在下游真正收到 close、且尚未返回错误时取样，证明提交顺序。
        let stat = self.collector.result_set_stat();
        self.observations.lock().unwrap().push(format!(
            "{}:{}:{}:{}",
            stat.open_count(),
            stat.opening_count(),
            stat.fetch_row_count(),
            stat.close_count()
        ));
        Err(RdbcError::DriverError("close failed".to_string()))
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}
