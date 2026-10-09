/// 保留结果集七种行操作的独立调用身份。
/// 对应 Java: java.sql.ResultSet 的 insertRow/updateRow/deleteRow/refreshRow/cancelRowUpdates/moveToInsertRow/moveToCurrentRow。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultSetRowOperation {
    /// 插入当前插入行。
    InsertRow,
    /// 提交当前行更新。
    UpdateRow,
    /// 删除当前行。
    DeleteRow,
    /// 从驱动刷新当前行。
    RefreshRow,
    /// 撤销当前行待提交更新。
    CancelRowUpdates,
    /// 移至插入行。
    MoveToInsertRow,
    /// 从插入行回到当前行。
    MoveToCurrentRow,
}
