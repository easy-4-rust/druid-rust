use crate::error::RdbcError;
use crate::resource::ResourceOperations;
use crate::resource::{RdbcInputStream,RdbcOutputStream,RdbcReader,RdbcWriter};
use crate::types::RdbcString;

/// 独占可变SQL资源标准接口；对应 Java: java.sql.Clob。
/// 对象独立拥有，仅要求Send，不要求Sync。各方法返回实际值或数据库错误。
#[async_trait::async_trait]
pub trait Clob: std::fmt::Debug + Send + 'static {
    /// 返回真实支持的操作集合（Rust扩展）。
    fn capabilities(&self) -> ResourceOperations;
    /// 对应 Java: Clob#length()；字符长度采用Java UTF-16单元。
    async fn length(&mut self) -> Result<i64, RdbcError>;
    /// 对应 Java: Clob#getSubString(long,int)；1-based位置，保留UTF-16包括孤立代理项。
    async fn get_sub_string(&mut self, position: i64, length: i32) -> Result<RdbcString, RdbcError>;
    /// 对应 Java: Clob#getCharacterStream()；返回字符读取流。
    async fn get_character_stream(&mut self) -> Result<RdbcReader, RdbcError>;
    /// 对应 Java: Clob#getAsciiStream()；返回ASCII读取流。
    async fn get_ascii_stream(&mut self) -> Result<RdbcInputStream, RdbcError>;
    /// 对应 Java: Clob#position(String,long)；返回1-based位置，未找到为None。
    async fn position_string(&mut self, pattern: &RdbcString, start: i64) -> Result<Option<i64>, RdbcError>;
    /// 对应 Java: Clob#position(Clob,long)；模式也使用同一标准接口。
    async fn position_clob(&mut self, pattern: &mut dyn Clob, start: i64) -> Result<Option<i64>, RdbcError>;
    /// 对应 Java: Clob#setString(long,String)；返回写入字符单元数。
    async fn set_string(&mut self, position: i64, value: &RdbcString) -> Result<i32, RdbcError>;
    /// 对应 Java: Clob#setString(long,String,int,int)；资源位置1-based，输入offset为0-based UTF-16。
    async fn set_string_range(&mut self, position: i64, value: &RdbcString, offset: i32, length: i32) -> Result<i32, RdbcError>;
    /// 对应 Java: Clob#setAsciiStream(long)；返回指定位置ASCII写入流。
    async fn set_ascii_stream(&mut self, position: i64) -> Result<RdbcOutputStream, RdbcError>;
    /// 对应 Java: Clob#setCharacterStream(long)；返回指定位置字符写入器。
    async fn set_character_stream(&mut self, position: i64) -> Result<RdbcWriter, RdbcError>;
    /// 对应 Java: Clob#truncate(long)；截断为指定字符长度，负值拒绝。
    async fn truncate(&mut self, length: i64) -> Result<(), RdbcError>;
    /// 对应 Java: Clob#free()；幂等释放后其他JDBC操作拒绝。
    async fn free(&mut self) -> Result<(), RdbcError>;
    /// 对应 Java: Clob#getCharacterStream(long,long)；返回指定1-based字符范围。
    async fn get_character_stream_range(&mut self, position: i64, length: i64) -> Result<RdbcReader, RdbcError>;
}
