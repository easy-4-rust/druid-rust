/// 设置连接客户端信息属性失败的标准原因，供每个失败属性保留独立状态。
///
/// 对应 Java: java.sql.ClientInfoStatus；由 SQLClientInfoException 报告，
/// 区分未知原因、未知属性、非法属性值和存储或传输时的值截断。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClientInfoStatus {
    /// 无法确定属性设置失败的原因。
    ReasonUnknown,
    /// 数据源无法识别该客户端信息属性名。
    ReasonUnknownProperty,
    /// 客户端信息属性值无效。
    ReasonValueInvalid,
    /// 属性值在存储或传输时被截断。
    ReasonValueTruncated,
}
