use crate::error::RdbcError;
use crate::wrapper::Wrapper;

/// JDBC parameter mode descriptor。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParameterMode {
    /// 输入参数。
    In,
    /// 输出参数。
    Out,
    /// 输入输出参数。
    InOut,
    /// 驱动无法确定。
    Unknown,
}

/// 一个 immutable prepared parameter snapshot。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterDescriptor {
    type_name: String,
    mode: ParameterMode,
    nullable: bool,
}

impl ParameterDescriptor {
    /// 创建 parameter snapshot。
    pub fn new(type_name: impl Into<String>, mode: ParameterMode, nullable: bool) -> Self {
        Self {
            type_name: type_name.into(),
            mode,
            nullable,
        }
    }

    /// 返回驱动 SQL type name。
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// 返回 parameter mode。
    pub const fn mode(&self) -> ParameterMode {
        self.mode
    }

    /// 返回参数是否允许 SQL NULL。
    pub const fn nullable(&self) -> bool {
        self.nullable
    }
}

/// JDBC `ParameterMetaData` 的 immutable owned snapshot 标准。
pub trait ParameterMetaData: Wrapper + Send + Sync + 'static {
    /// 返回 parameter 数量。
    fn parameter_count(&self) -> usize;

    /// 返回一个 1-based parameter descriptor。
    fn parameter(&self, _parameter_index: usize) -> Result<ParameterDescriptor, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "parameter_meta_data_parameter",
        })
    }
}
