use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ResultSet;

/// 仅实现标准接口必需的 next 和原关闭合同，其余能力使用真实默认实现。
/// 对应 Java: java.sql.ResultSet；迁移 BASE SparsePhysicalResultSet。
#[derive(Debug)]
pub struct SparseDefaultResultSet;

druid_rdbc::impl_standard_wrapper!(SparseDefaultResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for SparseDefaultResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_next",
        })
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}
