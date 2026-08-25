//! Immutable standard parameter metadata snapshot shared by wrapper adapters.

use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{
    ParameterDescriptor, ParameterMetaData, ParameterMode, ParameterNullability,
};
use druid_rdbc::statement::ParameterIndex;

/// 一个不借用 connection 或 statement 的 immutable 参数 metadata snapshot。
#[derive(Debug)]
pub(crate) struct PreparedParameterMetaDataSnapshot {
    parameters: Vec<ParameterDescriptor>,
}

impl PreparedParameterMetaDataSnapshot {
    /// 按 SQL placeholder 数量创建未知类型的输入参数 snapshot。
    pub(crate) fn unknown_inputs(parameter_count: usize) -> Self {
        Self {
            parameters: (0..parameter_count)
                .map(|_| {
                    ParameterDescriptor::new(
                        "UNKNOWN",
                        ParameterMode::In,
                        ParameterNullability::Unknown,
                    )
                })
                .collect(),
        }
    }
}

impl ParameterMetaData for PreparedParameterMetaDataSnapshot {
    fn parameter_count(&self) -> usize {
        self.parameters.len()
    }

    fn parameter(&self, parameter_index: ParameterIndex) -> Result<ParameterDescriptor, RdbcError> {
        let offset = parameter_index
            .get()
            .checked_sub(1)
            .ok_or_else(|| RdbcError::InvalidArgument("parameter index is 1-based".to_owned()))?;
        self.parameters.get(offset).cloned().ok_or_else(|| {
            RdbcError::InvalidArgument(format!(
                "parameter index {} exceeds metadata width",
                parameter_index.get()
            ))
        })
    }
}

druid_rdbc::impl_standard_wrapper!(PreparedParameterMetaDataSnapshot => ParameterMetaData);
