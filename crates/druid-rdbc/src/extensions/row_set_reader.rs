use super::RowSetInternal;
use crate::error::RdbcError;

/// 行集数据读取 provider；填充同一逻辑 RowSet 的执行视图。
/// 对应 Java: javax.sql.RowSetReader。读取期间的行/游标变化不单独通知监听器。
#[async_trait::async_trait]
pub trait RowSetReader: Send {
    /// 使用参数 caller 获取参数及借用连接、安装 metadata 并填充行；返回成功或原数据库错误。
    /// 对应 Java: RowSetReader#readData。递归 execute 必须拒绝，失败不发布部分数据。
    async fn read_data(&mut self, caller: &mut dyn RowSetInternal) -> Result<(), RdbcError>;
}
