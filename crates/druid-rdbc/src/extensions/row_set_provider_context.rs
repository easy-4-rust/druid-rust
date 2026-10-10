use super::{RowSet, RowSetInternal, SharedRowSet, SharedRowSetMetaData};
use crate::connection::Connection;
use crate::error::RdbcError;
use crate::result_set::ResultSet;
use crate::statement::PreparedInputParameter;

/// 本次 provider 调用的上下文，连接仅借用，行集 facet 保持原 source 身份。
/// 对应 Java: javax.sql.RowSetInternal 与 RowSetReader/Writer 的 caller 参数。
pub(crate) struct RowSetProviderContext<'a> {
    pub(crate) source: SharedRowSet,
    pub(crate) connection: Option<&'a mut dyn Connection>,
}
impl RowSetInternal for RowSetProviderContext<'_> {
    fn params(&self) -> Result<Vec<PreparedInputParameter>, RdbcError> {
        self.source.params()
    }
    fn connection(&mut self) -> Result<Option<&mut dyn Connection>, RdbcError> {
        match self.connection {
            Some(ref mut connection) => Ok(Some(&mut **connection)),
            None => Ok(None),
        }
    }
    fn set_meta_data(&mut self, metadata: SharedRowSetMetaData) -> Result<(), RdbcError> {
        self.source.set_meta_data(metadata)
    }
    fn original(&self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.source.original()
    }
    fn original_row(&self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.source.original_row()
    }
    fn row_set(&mut self) -> &mut dyn RowSet {
        &mut self.source
    }
}
