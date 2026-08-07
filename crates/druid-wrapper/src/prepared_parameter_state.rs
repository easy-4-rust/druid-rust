//! 扩展 Adapter 的物理 Prepared 参数槽。

use crate::prepared_parameter_materializer::PreparedParameterMaterializer;
use druid::core::{DruidError, PreparedInputParameter, Value};
use std::sync::Mutex;

#[derive(Clone)]
enum StoredPreparedParameter {
    Materialized(Value),
    Deferred(PreparedInputParameter),
}

impl StoredPreparedParameter {
    async fn materialize(self) -> Result<Value, DruidError> {
        match self {
            Self::Materialized(value) => Ok(value),
            Self::Deferred(parameter) => {
                PreparedParameterMaterializer::materialize(&parameter).await
            }
        }
    }
}

enum StoredPreparedBatch {
    Parameters(Vec<StoredPreparedParameter>),
    Values(Vec<Value>),
}

/// 保存物理 setter 已物化值及必须延迟到异步执行边界的 RDBC 资源句柄。
pub(crate) struct PreparedParameterState {
    parameters: Mutex<Vec<Option<StoredPreparedParameter>>>,
    batches: Mutex<Vec<StoredPreparedBatch>>,
}

impl PreparedParameterState {
    /// 创建空参数状态。
    pub(crate) fn new() -> Self {
        Self {
            parameters: Mutex::new(Vec::new()),
            batches: Mutex::new(Vec::new()),
        }
    }

    /// 在 setter 时物化本地值；仅异步 RDBC 资源保留参数描述符。
    pub(crate) fn set(
        &self,
        parameter_index: usize,
        parameter: &PreparedInputParameter,
    ) -> Result<(), DruidError> {
        if parameter_index == 0 {
            return Err(DruidError::InvalidArgument(
                "parameterIndex must be at least 1".to_string(),
            ));
        }
        let stored = PreparedParameterMaterializer::materialize_immediate(parameter)?.map_or_else(
            || StoredPreparedParameter::Deferred(parameter.clone()),
            StoredPreparedParameter::Materialized,
        );
        let mut parameters = self
            .parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if parameters.len() < parameter_index {
            parameters.resize(parameter_index, None);
        }
        parameters[parameter_index - 1] = Some(stored);
        Ok(())
    }

    /// 返回连续参数槽的物理值。
    pub(crate) async fn values(&self, parameter_count: usize) -> Result<Vec<Value>, DruidError> {
        let parameters = self
            .parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let parameters = (0..parameter_count)
            .map(|index| {
                parameters.get(index).and_then(Clone::clone).ok_or_else(|| {
                    DruidError::InvalidArgument(format!("parameter {} has not been set", index + 1))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut values = Vec::with_capacity(parameters.len());
        for parameter in parameters {
            values.push(parameter.materialize().await?);
        }
        Ok(values)
    }

    /// 清空当前参数槽。
    pub(crate) fn clear_parameters(&self) {
        self.parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    /// 保存 Rust 显式值 batch。
    pub(crate) fn add_values(&self, params: &[Value]) {
        self.batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(StoredPreparedBatch::Values(params.to_vec()));
    }

    /// 保存当前物理参数槽快照。
    pub(crate) fn add_parameters(
        &self,
        params: &[PreparedInputParameter],
    ) -> Result<(), DruidError> {
        let parameters = self
            .parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let snapshot = (0..params.len())
            .map(|index| {
                parameters.get(index).and_then(Clone::clone).ok_or_else(|| {
                    DruidError::InvalidArgument(format!("parameter {} has not been set", index + 1))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(StoredPreparedBatch::Parameters(snapshot));
        Ok(())
    }

    /// 清空 batch。
    pub(crate) fn clear_batches(&self) {
        self.batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    /// 按 wrapper 数量消费物理 batch。
    pub(crate) async fn take_batches(
        &self,
        expected_count: usize,
    ) -> Result<Option<Vec<Vec<Value>>>, DruidError> {
        let batches = {
            let mut batches = self
                .batches
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if batches.is_empty() {
                return Ok(None);
            }
            if batches.len() != expected_count {
                return Err(DruidError::InvalidArgument(format!(
                    "physical prepared batch count {}, wrapper batch count {expected_count}",
                    batches.len(),
                )));
            }
            std::mem::take(&mut *batches)
        };
        let mut values = Vec::with_capacity(batches.len());
        for batch in batches {
            match batch {
                StoredPreparedBatch::Values(batch) => values.push(batch),
                StoredPreparedBatch::Parameters(batch) => {
                    let mut materialized = Vec::with_capacity(batch.len());
                    for parameter in batch {
                        materialized.push(parameter.materialize().await?);
                    }
                    values.push(materialized);
                }
            }
        }
        Ok(Some(values))
    }
}

#[cfg(test)]
mod tests {
    use super::PreparedParameterState;
    use druid::core::{PreparedInputParameter, Value};

    #[tokio::test]
    async fn preserves_one_based_slots_batch_snapshots_and_mismatch_errors() {
        let state = PreparedParameterState::new();
        assert!(state.set(0, &PreparedInputParameter::Int(1)).is_err());

        state
            .set(2, &PreparedInputParameter::String(Some("two".to_string())))
            .unwrap();
        assert!(state.values(2).await.is_err());
        state.set(1, &PreparedInputParameter::Int(1)).unwrap();
        assert_eq!(
            state.values(2).await.unwrap(),
            vec![Value::Int(1), Value::String("two".to_string())]
        );

        state
            .add_parameters(&[
                PreparedInputParameter::Int(1),
                PreparedInputParameter::String(Some("two".to_string())),
            ])
            .unwrap();
        assert!(state.take_batches(2).await.is_err());
        assert_eq!(
            state.take_batches(1).await.unwrap(),
            Some(vec![vec![Value::Int(1), Value::String("two".to_string())]])
        );
        assert_eq!(state.take_batches(0).await.unwrap(), None);

        state.add_values(&[Value::Bool(true)]);
        state.clear_batches();
        assert_eq!(state.take_batches(0).await.unwrap(), None);

        state.clear_parameters();
        assert!(state.values(1).await.is_err());
    }
}
