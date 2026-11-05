/// 保存与 maxFieldSize 无关的意外数据截断诊断。
///
/// 对应 Java: java.sql.DataTruncation。写入截断是异常，读取截断是警告；
/// 已知参数或列位置使用 1-based 下标，未知字节长度以 None 保留。
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
}

impl DataTruncation {
    /// 以五个字段创建截断诊断。
    ///
    /// 对应 Java: java.sql.DataTruncation(int, boolean, boolean, int, int)。
    /// cause 重载在 Rust 侧由外层 `SqlException` 的 cause 链承载，本结构不重复保存。
    #[must_use]
    pub fn new(
        index: usize,
        parameter: bool,
        read: bool,
        data_size: Option<usize>,
        transfer_size: Option<usize>,
    ) -> Self {
        Self {
            index,
            parameter,
            read,
            data_size,
            transfer_size,
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
}
