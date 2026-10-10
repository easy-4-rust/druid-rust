use crate::error::RdbcError;
use crate::resource::ResourceOperations;
use crate::resource::{
    RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcWriter, RdbcXmlRepresentationType,
    RdbcXmlResult, RdbcXmlSource,
};
use crate::types::RdbcString;

/// 独占可变SQL资源标准接口；对应 Java: java.sql.SQLXML。
/// 对象独立拥有，仅要求Send，不要求Sync。各方法返回实际值或数据库错误。
#[async_trait::async_trait]
pub trait SqlXml: std::fmt::Debug + Send + 'static {
    /// 返回真实支持的操作集合（Rust扩展）。
    fn capabilities(&self) -> ResourceOperations;
    /// 验证本XML能否绑定到SQL输出，无参数；返回成功或原数据库/XML异常。
    /// 对应 Java: SQLOutput#writeSQLXML 的前置状态合同（Rust平台扩展）。
    /// 驱动必须确认所有Writer、OutputStream和框架Result已完成关闭，并传播XML处理错误。
    /// 本操作不得消费读写机会、读取XML内容或隐式关闭调用者资源；未知状态默认明确拒绝。
    async fn validate_for_binding(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported(
            "SQLXML#validate_for_binding",
        ))
    }
    /// 对应 Java: SQLXML#free()；释放后其他JDBC方法拒绝，重复释放无副作用。
    async fn free(&mut self) -> Result<(), RdbcError>;
    /// 对应 Java: SQLXML#getBinaryStream()；读取后资源变为不可读，可能不可写。
    async fn binary_stream(&mut self) -> Result<RdbcInputStream, RdbcError>;
    /// 对应 Java: SQLXML#setBinaryStream()；调用后不可写，流关闭后才可用于后续绑定。
    async fn set_binary_stream(&mut self) -> Result<RdbcOutputStream, RdbcError>;
    /// 对应 Java: SQLXML#getCharacterStream()；读取后资源变为不可读。
    async fn character_stream(&mut self) -> Result<RdbcReader, RdbcError>;
    /// 对应 Java: SQLXML#setCharacterStream()；调用后不可写，字符writer应完成关闭。
    async fn set_character_stream(&mut self) -> Result<RdbcWriter, RdbcError>;
    /// 对应 Java: SQLXML#getString()；读取后不可读，保留XML文本。
    async fn string(&mut self) -> Result<RdbcString, RdbcError>;
    /// 对应 Java: SQLXML#setString(String)；设置XML文本并进入不可写状态。
    async fn set_string(&mut self, value: &RdbcString) -> Result<(), RdbcError>;
    /// 对应 Java: SQLXML#getSource(Class)；None保留Java null的厂商默认表示请求，调用后不可读。
    async fn source(
        &mut self,
        representation: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlSource, RdbcError>;
    /// 对应 Java: SQLXML#setResult(Class)；None保留Java null的厂商默认表示请求，调用后不可写。
    async fn result(
        &mut self,
        representation: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlResult, RdbcError>;
}
