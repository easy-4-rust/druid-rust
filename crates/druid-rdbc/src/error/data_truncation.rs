/// 保存与 maxFieldSize 无关的意外数据截断诊断。
///
/// 对应 Java: java.sql.DataTruncation。写入截断是异常，读取截断是警告；
/// 已知参数或列位置使用 1-based 下标，未知字节长度以 None 保留。
///
/// Java 的 `DataTruncation` 继承 `SQLWarning`，因此同时携带 `SQLState`
/// 与 vendor code：读取截断为 `01004`，写入截断为 `22001`，vendor code 恒为 0。
/// Rust 侧把这些值保存在内嵌的 [`SqlWarning`] 中，并以 `next_warning` 保存
/// Java `SQLWarning#setNextWarning` 的警告链。
use super::SqlWarning;

/// Java `DataTruncation` 使用的固定原因文本。对应 Java: super("Data truncation", ...)。
const DATA_TRUNCATION_REASON: &str = "Data truncation";
/// 读取截断的 `SQLState`。对应 Java: read=true → "01004"。
const READ_TRUNCATION_SQL_STATE: &str = "01004";
/// 写入截断的 `SQLState`。对应 Java: read=false → "22001"。
const WRITE_TRUNCATION_SQL_STATE: &str = "22001";
/// Java `DataTruncation` 的 vendor code 恒为 0。
const DATA_TRUNCATION_VENDOR_CODE: i32 = 0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataTruncation {
    /// 被截断的参数或列位置。
    pub index: usize,
    /// 是否为参数；false 表示列。
    pub parameter: bool,
    /// 是否在读取时发生截断；false 表示写入。
    pub read: bool,
    /// 原始字节长度；None 表示未知。
    pub data_size: Option<usize>,
    /// 实际传输字节长度；None 表示未知。
    pub transfer_size: Option<usize>,
    /// 内嵌的 SQLWarning，承载 Java `SQLException` 侧的 SQLState 与 vendor code。
    sql_warning: SqlWarning,
    /// 警告链的后继节点。对应 Java: SQLWarning#setNextWarning。
    next_warning: Option<Box<SqlWarning>>,
}

impl DataTruncation {
    /// 以五个字段创建截断诊断。
    ///
    /// 对应 Java: java.sql.DataTruncation(int, boolean, boolean, int, int)。
    /// cause 重载在 Rust 侧由外层 `SqlException` 的 cause 链承载，本结构不重复保存。
    ///
    /// `SQLState` 按 Java 规则由 `read` 决定：读取截断为 `01004`，写入截断为 `22001`；
    /// vendor code 恒为 0，原因文本固定为 `Data truncation`。
    #[must_use]
    pub fn new(
        index: usize,
        parameter: bool,
        read: bool,
        data_size: Option<usize>,
        transfer_size: Option<usize>,
    ) -> Self {
        let sql_state = if read {
            READ_TRUNCATION_SQL_STATE
        } else {
            WRITE_TRUNCATION_SQL_STATE
        };
        Self {
            index,
            parameter,
            read,
            data_size,
            transfer_size,
            sql_warning: SqlWarning::new(
                DATA_TRUNCATION_REASON,
                Some(sql_state.to_owned()),
                DATA_TRUNCATION_VENDOR_CODE,
            ),
            next_warning: None,
        }
    }

    /// 返回被截断的参数或列位置。对应 Java: DataTruncation#getIndex。
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    /// 返回截断是否发生在参数上。对应 Java: DataTruncation#getParameter。
    #[must_use]
    pub fn parameter(&self) -> bool {
        self.parameter
    }

    /// 返回截断是否发生在读取时。对应 Java: DataTruncation#getRead。
    #[must_use]
    pub fn read(&self) -> bool {
        self.read
    }

    /// 返回原始字节长度；None 表示未知。对应 Java: DataTruncation#getDataSize。
    #[must_use]
    pub fn data_size(&self) -> Option<usize> {
        self.data_size
    }

    /// 返回实际传输字节长度；None 表示未知。对应 Java: DataTruncation#getTransferSize。
    #[must_use]
    pub fn transfer_size(&self) -> Option<usize> {
        self.transfer_size
    }

    /// 返回内嵌的 SQLWarning，承载 SQLState、vendor code 与原因文本。
    ///
    /// 对应 Java: DataTruncation 继承自 SQLWarning 的 SQLState 与 vendor code 语义。
    #[must_use]
    pub fn sql_warning(&self) -> &SqlWarning {
        &self.sql_warning
    }

    /// 返回 `SQLState`；读取截断为 `01004`，写入截断为 `22001`。
    ///
    /// 对应 Java: DataTruncation#getSQLState()（继承自 SQLException）。
    #[must_use]
    pub fn sql_state(&self) -> Option<&str> {
        self.sql_warning.sql_state()
    }

    /// 返回 vendor code；Java `DataTruncation` 恒为 0。
    ///
    /// 对应 Java: DataTruncation#getErrorCode()（继承自 SQLException）。
    #[must_use]
    pub fn error_code(&self) -> i32 {
        self.sql_warning.error_code()
    }

    /// 返回警告链中的下一条警告；None 表示没有后继警告。
    ///
    /// 对应 Java: DataTruncation#getNextWarning()（继承自 SQLWarning）。
    #[must_use]
    pub fn next_warning(&self) -> Option<&SqlWarning> {
        self.next_warning.as_deref()
    }

    /// 设置警告链中的下一条警告。
    ///
    /// 对应 Java: DataTruncation#setNextWarning(SQLWarning)（继承自 SQLWarning）。
    pub fn set_next_warning(&mut self, next_warning: SqlWarning) {
        self.next_warning = Some(Box::new(next_warning));
    }

    /// 返回与 Java `SQLWarning` 等价的警告对象，包含完整警告链。
    ///
    /// 对应 Java: 把 DataTruncation 当作 SQLWarning 观察时的视图。
    #[must_use]
    pub fn to_sql_warning(&self) -> SqlWarning {
        let mut warning = self.sql_warning.clone();
        if let Some(next_warning) = self.next_warning.as_deref() {
            warning.set_next_warning(next_warning.clone());
        }
        warning
    }

    /// 返回可遍历的警告链视图：首元素是本对象对应的内嵌警告，其后是警告链节点。
    ///
    /// 对应 Java: DataTruncation#iterator()（继承自 SQLException）。Java 迭代的是
    /// 异常链，`DataTruncation` 在 Java 中继承 `SQLWarning`，因此 Rust 侧迭代
    /// 其 SQLWarning 链。
    pub fn causes(&self) -> impl Iterator<Item = &SqlWarning> + '_ {
        std::iter::once(self.sql_warning()).chain(self.next_warning_chain())
    }

    /// 返回 `next_warning` 及其后继节点组成的迭代器。
    fn next_warning_chain(&self) -> impl Iterator<Item = &SqlWarning> + '_ {
        let mut current = self.next_warning.as_deref();
        std::iter::from_fn(move || {
            let warning = current?;
            current = warning.next_warning();
            Some(warning)
        })
    }
}
