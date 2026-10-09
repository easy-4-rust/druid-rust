use super::DruidPooledCallableStatement;
use crate::core::RdbcError;
use druid_rdbc::metadata::ResultSetMetaData;
use druid_rdbc::statement::{ParameterIndex, PreparedInputParameter, PreparedParameter};
use std::sync::Arc;

impl DruidPooledCallableStatement {
    pub(super) fn add_prepared_batch_core(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let parameters = self.parameter_snapshot()?;
        let result = self
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .add_prepared_batch();
        self.classify_statement_mutation(result)?;
        self.shared
            .state
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
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
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
                        "callable batch indexes must be contiguous and 1-based".to_owned(),
                    ));
                }
                Ok(parameter)
            })
            .collect::<Result<Vec<_>, RdbcError>>()?;
        self.shared
            .state
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
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .add_parameter_batch(parameters.clone());
        self.classify_statement_mutation(result)?;
        self.shared
            .state
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
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_prepared_batch();
        self.classify_statement_mutation(result)?;
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .clear();
        Ok(())
    }

    pub(super) async fn execute_prepared_batch_core(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.ensure_open()?;
        let parameter_sets = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .clone();
        let resource = Arc::clone(&self.shared.resource);
        let logical_state = &self.shared.state;
        let result = self
            .statement_base
            .inner
            .policy
            .execute_prepared_batch(
                &self.shared.sql,
                &parameter_sets,
                self.id(),
                move || async move {
                    let mut statement = resource.statement.lock().await;
                    let raw = statement.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
                    // Filter 已允许执行且 raw 锁已取得；与 adapter 的同步 take 同步消费快照。
                    // 若在 Filter 或取锁前取消，两层队列仍保留；开始 raw 执行后不重放资源。
                    logical_state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .batch_parameter_sets
                        .clear();
                    raw.execute_prepared_batch().await
                },
            )
            .await;
        result
    }

    pub(super) fn result_set_meta_data_core(
        &self,
    ) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.ensure_open()?;
        self.shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .result_set_meta_data()
    }
}
