use super::{RowSet, SharedRowSetMetaData};
use crate::connection::Connection;
use crate::error::RdbcError;
use crate::result_set::ResultSet;
use crate::statement::PreparedInputParameter;

/// Reader/Writer 访问同一行集内部状态的接口，不独立拥有调用者连接。
/// 对应 Java: javax.sql.RowSetInternal。不是 Statement，也不引入另一套连接标准。
pub trait RowSetInternal: Send {
    /// 返回命令参数的非消费快照；无参数，名称解析失败时返回 SQL 错误。
    /// 对应 Java: RowSetInternal#getParams。
    fn params(&self) -> Result<Vec<PreparedInputParameter>, RdbcError>;
    /// 返回本次执行借用的连接，未传入时为 None；不得关闭该连接。
    /// 对应 Java: RowSetInternal#getConnection。
    fn connection(&mut self) -> Result<Option<&mut dyn Connection>, RdbcError>;
    /// 安装参数 metadata 的同一共享对象；返回成功或状态错误。
    /// 对应 Java: RowSetInternal#setMetaData。
    fn set_meta_data(&mut self, metadata: SharedRowSetMetaData) -> Result<(), RdbcError>;
    /// 返回所有原始行的独立 before-first 标准结果集；无参数。
    /// 对应 Java: RowSetInternal#getOriginal。
    fn original(&self) -> Result<Box<dyn ResultSet>, RdbcError>;
    /// 返回当前行的独立原始结果集；无原始值为空，无当前行返回 SQL 错误。
    /// 对应 Java: RowSetInternal#getOriginalRow。
    fn original_row(&self) -> Result<Box<dyn ResultSet>, RdbcError>;
    /// 返回同一逻辑 source 的 RowSet 操作视图；无参数，不创建第二个独立行集。
    /// Rust 适配 Java provider 对 caller 的 RowSet 接口转换。
    fn row_set(&mut self) -> &mut dyn RowSet;
}
