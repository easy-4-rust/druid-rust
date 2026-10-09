use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{
    FetchDirection, ResultSetConcurrency, ResultSetHoldability, ResultSetType,
};

/// 保留标准属性的原类型；Filter 不得以不兼容类型替代返回值。
/// 对应 Java: java.sql.ResultSet 属性方法的返回类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultSetPropertyValue {
    /// 布尔状态。
    Boolean(bool),
    /// 有符号整数属性。
    Integer(i32),
    /// 标准提取方向。
    FetchDirection(FetchDirection),
    /// 标准结果集类型。
    ResultSetType(ResultSetType),
    /// 标准并发模式。
    Concurrency(ResultSetConcurrency),
    /// 标准提交保留模式。
    Holdability(ResultSetHoldability),
    /// 可空字符串属性。
    String(Option<String>),
    /// 1-based 标准列下标。
    Column(ColumnIndex),
    /// 无返回值的 setter 完成。
    Unit,
}

impl ResultSetPropertyValue {
    /// 提取Boolean属性；无参数，类型不匹配时返回标准参数错误。
    pub(crate) fn into_boolean(self) -> Result<bool, RdbcError> {
        match self {
            Self::Boolean(value) => Ok(value),
            _ => Err(RdbcError::InvalidArgument(
                "incompatible result set property filter value".to_owned(),
            )),
        }
    }
    /// 提取Integer属性；无参数，类型不匹配时返回标准参数错误。
    pub(crate) fn into_integer(self) -> Result<i32, RdbcError> {
        match self {
            Self::Integer(value) => Ok(value),
            _ => Err(RdbcError::InvalidArgument(
                "incompatible result set property filter value".to_owned(),
            )),
        }
    }
    /// 提取FetchDirection属性；无参数，类型不匹配时返回标准参数错误。
    pub(crate) fn into_direction(self) -> Result<FetchDirection, RdbcError> {
        match self {
            Self::FetchDirection(value) => Ok(value),
            _ => Err(RdbcError::InvalidArgument(
                "incompatible result set property filter value".to_owned(),
            )),
        }
    }
    /// 提取ResultSetType属性；无参数，类型不匹配时返回标准参数错误。
    pub(crate) fn into_result_set_type(self) -> Result<ResultSetType, RdbcError> {
        match self {
            Self::ResultSetType(value) => Ok(value),
            _ => Err(RdbcError::InvalidArgument(
                "incompatible result set property filter value".to_owned(),
            )),
        }
    }
    /// 提取Concurrency属性；无参数，类型不匹配时返回标准参数错误。
    pub(crate) fn into_concurrency(self) -> Result<ResultSetConcurrency, RdbcError> {
        match self {
            Self::Concurrency(value) => Ok(value),
            _ => Err(RdbcError::InvalidArgument(
                "incompatible result set property filter value".to_owned(),
            )),
        }
    }
    /// 提取Holdability属性；无参数，类型不匹配时返回标准参数错误。
    pub(crate) fn into_holdability(self) -> Result<ResultSetHoldability, RdbcError> {
        match self {
            Self::Holdability(value) => Ok(value),
            _ => Err(RdbcError::InvalidArgument(
                "incompatible result set property filter value".to_owned(),
            )),
        }
    }
    /// 提取String属性；无参数，类型不匹配时返回标准参数错误。
    pub(crate) fn into_string(self) -> Result<Option<String>, RdbcError> {
        match self {
            Self::String(value) => Ok(value),
            _ => Err(RdbcError::InvalidArgument(
                "incompatible result set property filter value".to_owned(),
            )),
        }
    }
    /// 提取Column属性；无参数，类型不匹配时返回标准参数错误。
    pub(crate) fn into_column(self) -> Result<ColumnIndex, RdbcError> {
        match self {
            Self::Column(value) => Ok(value),
            _ => Err(RdbcError::InvalidArgument(
                "incompatible result set property filter value".to_owned(),
            )),
        }
    }
    /// 提取 setter 完成状态；无参数，拒绝不兼容的 Filter 返回类型。
    pub(crate) fn into_unit(self) -> Result<(), RdbcError> {
        match self {
            Self::Unit => Ok(()),
            _ => Err(RdbcError::InvalidArgument(
                "incompatible result set property filter value".to_owned(),
            )),
        }
    }
}
