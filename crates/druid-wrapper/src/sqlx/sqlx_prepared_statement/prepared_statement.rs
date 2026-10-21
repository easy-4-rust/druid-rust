//! 从 `sqlx_prepared_statement.rs` 拆出的实现块。

use super::*;

#[async_trait::async_trait]
impl druid_rdbc::statement::PreparedStatement for SqlxPreparedStatement {
    fn set_input_parameter(
        &mut self,
        index: druid_rdbc::statement::ParameterIndex,
        parameter: druid_rdbc::statement::PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        let actor = Arc::clone(self.actor()?);
        let parameter = if actor.backend_key() == "sqlite"
            && matches!(
                parameter,
                druid_rdbc::statement::PreparedInputParameter::AsciiStream { .. }
                    | druid_rdbc::statement::PreparedInputParameter::UnicodeStream { .. }
                    | druid_rdbc::statement::PreparedInputParameter::BinaryStream { .. }
                    | druid_rdbc::statement::PreparedInputParameter::BlobStream { .. }
                    | druid_rdbc::statement::PreparedInputParameter::CharacterStream { .. }
                    | druid_rdbc::statement::PreparedInputParameter::NCharacterStream { .. }
                    | druid_rdbc::statement::PreparedInputParameter::ClobReader { .. }
                    | druid_rdbc::statement::PreparedInputParameter::NClobReader { .. }
            ) {
            // SQLite 保留 BASE 同步绑定：底层保存读取后的值，执行及缓存复用不再次读流。
            let value = crate::prepared_parameter_materializer::PreparedParameterMaterializer::materialize_immediate(&parameter)?
                .ok_or_else(|| RdbcError::Other("synchronous stream binding requires a value".to_owned()))?;
            druid_rdbc::statement::PreparedInputParameter::RustValue(value)
        } else {
            parameter
        };
        self.parameter_state.set_input(index, parameter)?;
        actor.task4_trace_parameter_set();
        Ok(())
    }

    fn sql(&self) -> Result<&str, RdbcError> {
        self.actor()?;
        Ok(&self.sql)
    }

    fn set_parameter(
        &mut self,
        parameter: druid_rdbc::statement::PreparedParameter,
    ) -> Result<(), RdbcError> {
        let actor = Arc::clone(self.actor()?);
        self.parameter_state.set_standard(parameter)?;
        actor.task4_trace_parameter_set();
        Ok(())
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.actor()?;
        self.parameter_state.clear_parameters();
        Ok(())
    }

    fn add_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.actor()?;
        let values = self.parameter_state.inputs(self.parameter_count)?;
        self.prepared_batch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(values);
        Ok(())
    }

    fn add_parameter_batch(&mut self, parameters: Vec<PreparedParameter>) -> Result<(), RdbcError> {
        self.actor()?;
        let values = self.parameter_values(parameters)?;
        self.prepared_batch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(
                values
                    .into_iter()
                    .map(druid_rdbc::statement::PreparedInputParameter::RustValue)
                    .collect(),
            );
        Ok(())
    }

    fn add_input_parameter_batch(
        &mut self,
        mut parameters: Vec<(
            druid_rdbc::statement::ParameterIndex,
            druid_rdbc::statement::PreparedInputParameter,
        )>,
    ) -> Result<(), RdbcError> {
        self.actor()?;
        parameters.sort_by_key(|(index, _)| index.get());
        if parameters.len() != self.parameter_count
            || parameters
                .iter()
                .enumerate()
                .any(|(offset, (index, _))| index.get() != offset + 1)
        {
            return Err(RdbcError::InvalidArgument(
                "prepared batch indexes must match contiguous prepared width".to_owned(),
            ));
        }
        self.prepared_batch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(parameters.into_iter().map(|(_, input)| input).collect());
        Ok(())
    }

    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.actor()?;
        self.prepared_batch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        Ok(())
    }

    async fn execute_prepared_query(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        let _execution = self.reset_execution_results()?;
        let actor = Arc::clone(self.actor()?);
        let values = self.parameter_state.values(self.parameter_count).await?;
        let result = self
            .execute_prepared_query_values(&actor, values, true)
            .await?;
        actor.task4_trace_prepared_query();
        Ok(result)
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        let _execution = self.reset_execution_results()?;
        self.actor()?;
        let values = self.parameter_state.values(self.parameter_count).await?;
        self.execute_update_values(&self.sql, values, &self.prepared_generated_keys)
            .await
    }

    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        let _execution = self.reset_execution_results()?;
        let actor = Arc::clone(self.actor()?);
        let values = self.parameter_state.values(self.parameter_count).await?;
        if actor
            .statement_returns_rows_with_control(
                &self.sql,
                self.query_timeout.load(Ordering::Acquire),
                self.cancellation.interruption(),
            )
            .await?
        {
            Ok(vec![ExecutionResult::ResultSet(
                self.execute_prepared_query_values(&actor, values, false)
                    .await?,
            )])
        } else {
            self.execute_update_values(&self.sql, values, &self.prepared_generated_keys)
                .await
                .map(|count| vec![ExecutionResult::UpdateCount(count)])
        }
    }

    async fn execute_prepared_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        let _execution = self.reset_execution_results()?;
        self.actor()?;
        let parameter_sets = std::mem::take(
            &mut *self
                .prepared_batch
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        let mut counts = Vec::with_capacity(parameter_sets.len());
        let mut keys = Vec::with_capacity(parameter_sets.len());
        for inputs in parameter_sets {
            let execution = async {
                let values = PreparedParameterState::materialize(&inputs).await?;
                self.execute_update_values_result(&self.sql, values, &self.prepared_generated_keys)
                    .await
            };
            match execution.await {
                Ok((count, key)) => {
                    counts.push(count);
                    keys.extend(key);
                }
                Err(cause) => {
                    return Err(RdbcError::BatchUpdateException {
                        update_counts: counts.into(),
                        cause: Box::new(cause),
                    });
                }
            }
        }
        *self
            .generated_key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = keys;
        Ok(counts)
    }

    fn result_set_meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.actor()?;
        Ok(Arc::clone(&self.result_set_meta_data) as Arc<dyn ResultSetMetaData>)
    }

    fn parameter_meta_data(&self) -> Result<Arc<dyn ParameterMetaData>, RdbcError> {
        self.actor()?;
        Ok(Arc::new(PreparedParameterMetaDataSnapshot::unknown_inputs(
            self.parameter_count,
        )))
    }
}
