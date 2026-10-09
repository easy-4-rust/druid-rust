use druid_rdbc::result_set::ResultSetUpdate;
/// 原驱动实际收到的下标或列名更新及完整描述符。对应 Java: java.sql.ResultSet#updateXxx。
#[derive(Debug, Clone, PartialEq)]
pub(super) enum UpdateCall {
    Index(usize, ResultSetUpdate),
    Label(String, ResultSetUpdate),
}
