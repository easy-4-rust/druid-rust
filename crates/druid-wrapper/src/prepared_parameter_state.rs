//! Canonical prepared-parameter slots shared by wrapper adapters.

use crate::prepared_parameter_materializer::PreparedParameterMaterializer;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{ParameterIndex, PreparedInputParameter, PreparedParameter};
use druid_rdbc::types::Value;
use std::sync::Mutex;

/// 保留绑定参数的描述符，只在 adapter 执行边界读取资源。
/// 对应 Java: java.sql.PreparedStatement 的参数槽与批次快照。
pub(crate) struct PreparedParameterState {
    parameters: Mutex<Vec<Option<PreparedInputParameter>>>,
}

impl PreparedParameterState {
    pub(crate) fn new() -> Self {
        Self {
            parameters: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn set_standard(&self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        let (index, value) = PreparedParameterMaterializer::materialize_standard(parameter);
        self.set_input(
            ParameterIndex::new(index)?,
            PreparedInputParameter::RustValue(value),
        )
    }

    /// 保存 1-based 参数和资源句柄；不移动流或 Reader 游标。
    pub(crate) fn set_input(
        &self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        let index = index.get();
        let offset = index
            .checked_sub(1)
            .ok_or_else(|| RdbcError::InvalidArgument("parameter index is 1-based".to_owned()))?;
        let mut parameters = self
            .parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if parameters.len() <= offset {
            parameters.resize(offset + 1, None);
        }
        parameters[offset] = Some(parameter);
        Ok(())
    }

    /// 校验全部槽位后返回描述符快照；缺参时不读取任何资源。
    pub(crate) fn inputs(&self, count: usize) -> Result<Vec<PreparedInputParameter>, RdbcError> {
        let parameters = self
            .parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (0..count)
            .map(|offset| {
                parameters
                    .get(offset)
                    .and_then(Clone::clone)
                    .ok_or_else(|| {
                        RdbcError::InvalidArgument(format!(
                            "parameter {} has not been set",
                            offset + 1
                        ))
                    })
            })
            .collect()
    }

    /// 返回从 1 到最高槽位的快照，拒绝中间空洞。
    pub(crate) fn inputs_contiguous(&self) -> Result<Vec<PreparedInputParameter>, RdbcError> {
        let count = self
            .parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len();
        self.inputs(count)
    }

    /// 在 adapter 的异步执行边界将参数快照转换为驱动值。
    pub(crate) async fn materialize(
        inputs: &[PreparedInputParameter],
    ) -> Result<Vec<Value>, RdbcError> {
        let mut values = Vec::with_capacity(inputs.len());
        for input in inputs {
            values.push(PreparedParameterMaterializer::materialize(input).await?);
        }
        Ok(values)
    }

    /// 校验全部槽位后物化参数；锁在资源读取和 await 前释放。
    pub(crate) async fn values(&self, count: usize) -> Result<Vec<Value>, RdbcError> {
        Self::materialize(&self.inputs(count)?).await
    }

    /// 物化连续槽位，保持不跨 await 持有状态锁。
    pub(crate) async fn values_contiguous(&self) -> Result<Vec<Value>, RdbcError> {
        Self::materialize(&self.inputs_contiguous()?).await
    }

    pub(crate) fn clear_parameters(&self) {
        self.parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}
