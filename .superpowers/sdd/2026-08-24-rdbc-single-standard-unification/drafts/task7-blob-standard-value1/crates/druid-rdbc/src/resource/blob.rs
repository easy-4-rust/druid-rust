use crate::error::RdbcError;
use crate::resource::{BlobPattern, RdbcInputStream, RdbcOutputStream, ResourceOperations};

/// 表示 SQL 二进制大对象的独占可变标准接口。
/// 对应 Java: java.sql.Blob。资源按值拥有，不借用创建连接；只要求 Send，不要求 Sync。
/// JDBC 声明共十一项；调用 free 后，除再次 free 外的 JDBC 操作必须拒绝。
#[async_trait::async_trait]
pub trait Blob: std::fmt::Debug + Send + 'static {
    /// 返回实例支持的操作集合；这是 Rust 能力扩展，声明必须与实际行为一致。
    fn capabilities(&self) -> ResourceOperations;
    /// 返回字节长度；失败时返回数据库错误。对应 Java: Blob#length。
    async fn length(&mut self) -> Result<i64, RdbcError>;
    /// 从1-based position读取至多length字节；position<1或length<0必须报错。
    /// 对应 Java: Blob#getBytes(long,int)。返回实际可用的字节副本。
    async fn get_bytes(&mut self, position: i64, length: i32) -> Result<Vec<u8>, RdbcError>;
    /// 返回完整二进制读取流；不支持时显式返回错误。对应 Java: Blob#getBinaryStream()。
    async fn get_binary_stream(&mut self) -> Result<RdbcInputStream, RdbcError>;
    /// 从1-based start搜索字节模式；未找到返回None，start<1必须报错。
    /// 对应 Java: Blob#position(byte[],long)，Java的-1映射为None。
    async fn position_bytes(&mut self, pattern: &[u8], start: i64) -> Result<Option<i64>, RdbcError>;
    /// 搜索另一个标准Blob的内容并返回1-based位置；未找到返回None。
    /// 对应 Java: Blob#position(Blob,long)。Other独占借用另一资源；SameResource保留自身模式，不构造重叠可变引用或具体类型downcast。
    async fn position_blob(&mut self, pattern: BlobPattern<'_>, start: i64) -> Result<Option<i64>, RdbcError>;
    /// 从1-based position覆盖写入bytes，越过原尾部时扩展，返回写入字节数。
    /// 对应 Java: Blob#setBytes(long,byte[])。position>长度+1属于驱动定义行为。
    async fn set_bytes(&mut self, position: i64, bytes: &[u8]) -> Result<i32, RdbcError>;
    /// 从输入bytes的0-based offset取length字节，写入1-based position并返回写入数。
    /// 对应 Java: Blob#setBytes(long,byte[],int,int)，不得混淆两种索引基准。
    async fn set_bytes_range(&mut self, position: i64, bytes: &[u8], offset: i32, length: i32) -> Result<i32, RdbcError>;
    /// 从1-based position打开写入流，写入覆盖旧内容并可扩展资源。
    /// 对应 Java: Blob#setBinaryStream(long)。position>长度+1属于驱动定义行为。
    async fn set_binary_stream(&mut self, position: i64) -> Result<RdbcOutputStream, RdbcError>;
    /// 将资源截断为length字节，负长度必须报错。对应 Java: Blob#truncate(long)。
    async fn truncate(&mut self, length: i64) -> Result<(), RdbcError>;
    /// 释放资源；重复调用无副作用，其他JDBC操作随后必须拒绝。
    /// 对应 Java: Blob#free。取消安全及owner通知由Druid内部包装监管。
    async fn free(&mut self) -> Result<(), RdbcError>;
    /// 打开1-based position起始、length字节长的部分读取流。
    /// 对应 Java: Blob#getBinaryStream(long,long)。非法起点或越界范围必须报错。
    async fn get_binary_stream_range(&mut self, position: i64, length: i64) -> Result<RdbcInputStream, RdbcError>;
}
