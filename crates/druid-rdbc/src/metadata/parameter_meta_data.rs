use crate::error::RdbcError;
use crate::statement::ParameterIndex;
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

/// JDBC parameter nullability descriptor。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParameterNullability {
    /// Parameter 不允许 SQL NULL。
    NoNulls,
    /// Parameter 允许 SQL NULL。
    Nullable,
    /// 驱动无法确定。
    Unknown,
}

/// 一个 immutable prepared parameter snapshot。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterDescriptor {
    type_name: String,
    mode: ParameterMode,
    nullability: ParameterNullability,
}

impl ParameterDescriptor {
    /// 创建 parameter snapshot。
    pub fn new(
        type_name: impl Into<String>,
        mode: ParameterMode,
        nullability: ParameterNullability,
    ) -> Self {
        Self {
            type_name: type_name.into(),
            mode,
            nullability,
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
    pub const fn nullability(&self) -> ParameterNullability {
        self.nullability
    }
}

/// JDBC `ParameterMetaData` 的 immutable owned snapshot 标准。
pub trait ParameterMetaData: Wrapper + Send + Sync + 'static {
    /// 返回 parameter 数量。
    fn parameter_count(&self) -> usize;

    /// 返回一个 1-based parameter descriptor。
    fn parameter(
        &self,
        _parameter_index: ParameterIndex,
    ) -> Result<ParameterDescriptor, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "parameter_meta_data_parameter",
        })
    }
}
