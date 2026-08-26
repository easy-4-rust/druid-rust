//! Canonical prepared-parameter slots shared by wrapper adapters.

use crate::prepared_parameter_materializer::PreparedParameterMaterializer;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::PreparedParameter;
use druid_rdbc::types::Value;
use std::sync::Mutex;

pub(crate) struct PreparedParameterState {
    parameters: Mutex<Vec<Option<Value>>>,
}

impl PreparedParameterState {
    pub(crate) fn new() -> Self {
        Self {
            parameters: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn set_standard(&self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        let (index, value) = PreparedParameterMaterializer::materialize_standard(parameter);
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
        parameters[offset] = Some(value);
        Ok(())
    }

    pub(crate) fn values(&self, count: usize) -> Result<Vec<Value>, RdbcError> {
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

    pub(crate) fn values_contiguous(&self) -> Result<Vec<Value>, RdbcError> {
        let count = self
            .parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len();
        self.values(count)
    }

    pub(crate) fn clear_parameters(&self) {
        self.parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}
