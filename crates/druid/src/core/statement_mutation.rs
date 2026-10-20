use druid_rdbc::statement::FetchDirection;

/// 标准语句设置与取消的无损参数描述，用于保持过滤器参数和代理状态一致。
/// 对应 Java: Filter#statement_setMaxRows 等 setter 及 statement_cancel。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementMutation {
    /// 设置最大字段字节数，零表示不限。
    SetMaxFieldSize(i32),
    /// 设置最大行数，保留 Rust 标准合同的完整 i64 范围。
    SetMaxRows(i64),
    /// 设置 JDBC 转义处理。
    SetEscapeProcessing(bool),
    /// 设置查询超时秒数。
    SetQueryTimeout(i32),
    /// 设置游标名称。
    SetCursorName(String),
    /// 设置可空游标名称，None 与空字符串保持可区分。
    SetCursorNameNullable(Option<String>),
    /// 设置抓取方向。
    SetFetchDirection(FetchDirection),
    /// 设置抓取行数提示。
    SetFetchSize(i32),
    /// 设置语句池化提示。
    SetPoolable(bool),
    /// 启用依赖结果集完成后的自动关闭。
    CloseOnCompletion,
    /// 请求取消当前执行。
    Cancel,
}
