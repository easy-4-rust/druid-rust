use super::RowSetEvent;

/// 行集数据或游标变化的监听器；通知前实现必须释放内部锁。
/// 对应 Java: javax.sql.RowSetListener。事件携带原 RowSet source，允许回调重入。
pub trait RowSetListener: Send + Sync {
    /// 参数 event 表示完整行集变化；无返回值。对应 Java: RowSetListener#rowSetChanged。
    fn row_set_changed(&self, event: &RowSetEvent);
    /// 参数 event 表示一行插入、更新或删除；无返回值。对应 Java: RowSetListener#rowChanged。
    fn row_changed(&self, event: &RowSetEvent);
    /// 参数 event 表示游标移动；无返回值。对应 Java: RowSetListener#cursorMoved。
    fn cursor_moved(&self, event: &RowSetEvent);
}
