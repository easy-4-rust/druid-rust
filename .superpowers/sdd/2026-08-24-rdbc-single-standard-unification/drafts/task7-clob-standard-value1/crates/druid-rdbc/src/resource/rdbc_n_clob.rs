use crate::error::RdbcError;
use crate::resource::{
    Clob, NClob, RdbcClob, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcWriter,
    ResourceOperations,
};
use crate::types::RdbcString;
/// 国家字符大对象共享值句柄；对应 Java: java.sql.NClob extends Clob。
/// 显式委托继承操作，不使用Deref模拟继承；Clob视图共享对象、身份与释放状态。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RdbcNClob {
    clob: RdbcClob,
}
impl RdbcNClob {
    /// 接管参数value国家字符对象，返回保留NClob身份的句柄；上转不复制内容。
    pub(crate) fn from_standard(value: Box<dyn NClob>) -> Self {
        let value: Box<dyn Clob> = value;
        Self {
            clob: RdbcClob::from_standard_named(value, "NClob"),
        }
    }
    /// 返回共享同一对象、身份与释放状态的Clob视图，无参数。
    #[must_use]
    pub const fn as_clob(&self) -> &RdbcClob {
        &self.clob
    }
    /// 返回UTF-16字符单元长度，无参数；失败返回原数据库错误。 对应 Java: Clob#length。
    pub async fn length(&self) -> Result<i64, RdbcError> {
        self.clob.length().await
    }
    /// 读取1-based参数position起至多length个UTF-16单元，返回保留孤立代理项的原值。 对应 Java: Clob#getSubString。
    pub async fn get_sub_string(
        &self,
        position: i64,
        length: i32,
    ) -> Result<RdbcString, RdbcError> {
        self.clob.get_sub_string(position, length).await
    }
    /// 返回原驱动字符读取流，无参数；不提前读取或转换UTF-16。 对应 Java: Clob#getCharacterStream。
    pub async fn get_character_stream(&self) -> Result<RdbcReader, RdbcError> {
        self.clob.get_character_stream().await
    }
    /// 返回原驱动ASCII读取流，无参数；编码策略由驱动决定。 对应 Java: Clob#getAsciiStream。
    pub async fn get_ascii_stream(&self) -> Result<RdbcInputStream, RdbcError> {
        self.clob.get_ascii_stream().await
    }
    /// 从1-based参数start搜索pattern，返回位置、None或原错误。 对应 Java: Clob#position(String,long)。
    pub async fn position_string(
        &self,
        pattern: &RdbcString,
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        self.clob.position_string(pattern, start).await
    }
    /// 将value写至1-based参数position，返回UTF-16单元数或原错误。 对应 Java: Clob#setString。
    pub async fn set_string(&self, position: i64, value: &RdbcString) -> Result<i32, RdbcError> {
        self.clob.set_string(position, value).await
    }
    /// 将value的0-based offset起length单元写至1-based position，返回实际写入数。 对应 Java: Clob#setString(long,String,int,int)。
    pub async fn set_string_range(
        &self,
        position: i64,
        value: &RdbcString,
        offset: i32,
        length: i32,
    ) -> Result<i32, RdbcError> {
        self.clob
            .set_string_range(position, value, offset, length)
            .await
    }
    /// 在1-based参数position返回原驱动ASCII写入流，不提前写入。 对应 Java: Clob#setAsciiStream。
    pub async fn set_ascii_stream(&self, position: i64) -> Result<RdbcOutputStream, RdbcError> {
        self.clob.set_ascii_stream(position).await
    }
    /// 在1-based参数position返回原驱动UTF-16字符写入器。 对应 Java: Clob#setCharacterStream。
    pub async fn set_character_stream(&self, position: i64) -> Result<RdbcWriter, RdbcError> {
        self.clob.set_character_stream(position).await
    }
    /// 截断至参数length个UTF-16单元，返回成功或原错误。 对应 Java: Clob#truncate。
    pub async fn truncate(&self, length: i64) -> Result<(), RdbcError> {
        self.clob.truncate(length).await
    }
    /// 返回1-based参数position起length单元的原字符流或原错误。 对应 Java: Clob#getCharacterStream(long,long)。
    pub async fn get_character_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, RdbcError> {
        self.clob.get_character_stream_range(position, length).await
    }
    /// 从1-based参数start搜索pattern资源，返回位置、None或原错误。对应 Java: Clob#position(Clob,long)。
    pub async fn position_clob(
        &self,
        pattern: &RdbcClob,
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        self.clob.position_clob(pattern, start).await
    }
    /// 幂等释放共享资源，无参数；失败返回原错误。对应 Java: Clob#free。
    pub async fn free(&self) -> Result<(), RdbcError> {
        self.clob.free().await
    }
    /// 返回共享资源是否已确认释放，无参数。
    #[must_use]
    pub fn is_freed(&self) -> bool {
        self.clob.is_freed()
    }
    /// 返回与Clob视图相同的稳定身份，无参数。
    #[must_use]
    pub fn resource_id(&self) -> &str {
        self.clob.resource_id()
    }
    /// 返回实际支持的Clob操作集合，无参数。
    #[must_use]
    pub fn capabilities(&self) -> ResourceOperations {
        self.clob.capabilities()
    }
}
