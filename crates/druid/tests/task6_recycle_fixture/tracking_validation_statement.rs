use super::TrackingConnection;
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{GeneratedKeys, Statement};

// 测试专用标准语句，复用原 TrackingConnection 的真实 fetch/exec 实现。
// 接入位置：完整回收测试夹具，不属于生产合同。
pub(super) struct TrackingValidationStatement {
    connection: TrackingConnection,
    closed: bool,
}

impl TrackingValidationStatement {
    pub(super) fn new(connection: TrackingConnection) -> Self {
        Self {
            connection,
            closed: false,
        }
    }
}

druid_rdbc::impl_standard_wrapper!(TrackingValidationStatement => Statement);

#[async_trait::async_trait]
impl Statement for TrackingValidationStatement {
    async fn execute_query(&mut self, sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        if self.closed {
            return Err(RdbcError::Other("statement is closed".to_owned()));
        }
        let rows = self.connection.fetch(sql, Vec::new()).await?;
        Ok(Box::new(BufferedRowSet::new(rows)))
    }

    async fn execute_update(&mut self, sql: &str, _keys: GeneratedKeys) -> Result<i64, RdbcError> {
        if self.closed {
            return Err(RdbcError::Other("statement is closed".to_owned()));
        }
        let result = self.connection.exec(sql, Vec::new()).await?;
        i64::try_from(result.rows_affected)
            .map_err(|_| RdbcError::InvalidArgument("update count exceeds i64".to_owned()))
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
}
