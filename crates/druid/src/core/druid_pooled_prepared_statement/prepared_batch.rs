use super::DruidPooledPreparedStatement;
use crate::core::RdbcError;
use druid_rdbc::metadata::ResultSetMetaData;
use druid_rdbc::statement::{ParameterIndex, PreparedInputParameter, PreparedParameter};
use std::sync::Arc;

impl DruidPooledPreparedStatement {
    pub(super) fn add_prepared_batch_core(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let parameters = self.parameter_snapshot()?;
        let result = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .add_prepared_batch();
        self.classify_statement_mutation(result)?;
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .push(parameters);
        Ok(())
    }

    pub(super) fn add_input_parameter_batch_core(
        &mut self,
        mut parameters: Vec<(ParameterIndex, PreparedInputParameter)>,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .add_input_parameter_batch(parameters.clone());
        self.classify_statement_mutation(result)?;
        parameters.sort_by_key(|(index, _)| index.get());
        let values = parameters
            .into_iter()
            .enumerate()
            .map(|(offset, (index, parameter))| {
                if index.get() != offset + 1 {
                    return Err(RdbcError::InvalidArgument(
                        "prepared batch indexes must be contiguous and 1-based".to_owned(),
                    ));
                }
                Ok(parameter)
            })
            .collect::<Result<Vec<_>, RdbcError>>()?;
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .push(values);
        Ok(())
    }

    pub(super) fn add_parameter_batch_core(
        &mut self,
        parameters: Vec<PreparedParameter>,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .add_parameter_batch(parameters.clone());
        self.classify_statement_mutation(result)?;
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .push(
                parameters
                    .into_iter()
                    .map(|parameter| PreparedInputParameter::RustValue(parameter.value))
                    .collect(),
            );
        Ok(())
    }

    pub(super) fn clear_prepared_batch_core(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_prepared_batch();
        self.classify_statement_mutation(result)?;
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .clear();
        Ok(())
    }

    pub(super) async fn execute_prepared_batch_core(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.ensure_open()?;
        let parameter_sets = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .clone();
        let resource = Arc::clone(&self.resource);
        let result = self
            .statement_base
            .inner
            .policy
            .execute_prepared_batch(&self.sql, &parameter_sets, self.id(), move || async move {
                resource
                    .statement
                    .lock()
                    .await
                    .as_mut()
                    .ok_or(RdbcError::ConnectionDiscarded)?
                    .execute_prepared_batch()
                    .await
            })
            .await;
        if result.is_ok() {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .batch_parameter_sets
                .clear();
        }
        result
    }

    pub(super) fn result_set_meta_data_core(
        &self,
    ) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.ensure_open()?;
        self.resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .result_set_meta_data()
    }
}
