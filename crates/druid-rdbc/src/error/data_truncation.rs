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
