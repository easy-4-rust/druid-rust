use super::SharedRowSet;
use crate::error::RdbcError;

/// 保留真实可用 RowSet source 身份的变化事件。
/// 对应 Java: javax.sql.RowSetEvent 及继承的 java.util.EventObject#getSource。
#[derive(Clone, Debug)]
pub struct RowSetEvent {
    pub(crate) source: SharedRowSet,
}
impl RowSetEvent {
    /// 使用参数 source 创建事件；None 对应 Java null，返回参数错误而不创建无效事件。
    /// 对应 Java: RowSetEvent#RowSetEvent。
    pub fn new(source: Option<SharedRowSet>) -> Result<Self, RdbcError> {
        source
            .map(|source| Self { source })
            .ok_or_else(|| RdbcError::InvalidArgument("RowSet event source is null".into()))
    }
    /// 返回同一逻辑 source 的共享句柄；无参数，不复制行集数据。
    /// 对应 Java: EventObject#getSource。
    pub fn source(&self) -> SharedRowSet {
        self.source.clone()
    }
}
