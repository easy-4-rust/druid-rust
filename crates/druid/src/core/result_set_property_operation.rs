use druid_rdbc::statement::FetchDirection;

/// 原 ResultSet 属性调用描述符，保留 setter 参数与原列名。
/// 对应 Java: java.sql.ResultSet 的属性及 findColumn 方法。
#[derive(Debug, Clone, Copy)]
pub enum ResultSetPropertyOperation<'a> {
    /// 是否为 SQL NULL。
    WasNull,
    /// 是否在首行之前。
    IsBeforeFirst,
    /// 是否在末行之后。
    IsAfterLast,
    /// 是否为首行。
    IsFirst,
    /// 是否为末行。
    IsLast,
    /// 当前行号。
    Row,
    /// 设置原提取方向。
    SetFetchDirection(FetchDirection),
    /// 读取提取方向。
    FetchDirection,
    /// 设置原提取大小。
    SetFetchSize(i32),
    /// 读取提取大小。
    FetchSize,
    /// 游标类型。
    ResultSetType,
    /// 并发模式。
    Concurrency,
    /// 提交后保留状态。
    Holdability,
    /// 游标名称。
    CursorName,
    /// 行更新状态。
    RowUpdated,
    /// 行插入状态。
    RowInserted,
    /// 行删除状态。
    RowDeleted,
    /// 关闭状态。
    IsClosed,
    /// 按原列名查找列，不预先访问驱动。
    FindColumn(&'a str),
}
