//! P04 前的 RDBC 错误占位类型。

/// 仅用于预留 `druid_rdbc::error::RdbcError` 名称的不可外部构造占位类型。
///
/// 该类型不代表最终错误分类，不提供构造器、转换或生产签名；P04 将以完整且经过差分验证的
/// RDBC 错误合同替换它。
#[derive(Debug)]
pub struct RdbcError {
    _placeholder: (),
}
