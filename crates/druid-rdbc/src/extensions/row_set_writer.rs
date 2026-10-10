use super::RowSetInternal;
use crate::error::RdbcError;

/// 行集写回 provider，负责数据源连接及冲突策略。
/// 对应 Java: javax.sql.RowSetWriter。false 表示未写入，包括检测到冲突的情况。
#[async_trait::async_trait]
pub trait RowSetWriter: Send {
    /// 将参数 caller 的修改写回数据源；返回 true 才可接受本地新基线，错误原样传播。
    /// 对应 Java: RowSetWriter#writeData。取消和 false 不代表外部事务已回滚。
    async fn write_data(&mut self, caller: &mut dyn RowSetInternal) -> Result<bool, RdbcError>;
}
