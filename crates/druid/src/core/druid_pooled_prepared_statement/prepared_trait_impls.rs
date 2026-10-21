use super::*;

#[async_trait::async_trait]
impl Statement for DruidPooledPreparedStatement {
    /// 按 JDBC 整数处置推进结果；先检查逻辑关闭和租约，再验证参数且不改变游标。
    /// 对应 Java：DruidPooledPreparedStatement#getMoreResults(int)。返回下个结果是否为游标。
    async fn get_more_results_current(&mut self, current: i32) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let result = self.statement_base.get_more_results_current(current).await;
        self.retain_cacheable_result(result)
    }

    delegate_statement_state_methods!(statement_base);
    delegate_cached_statement_property_reads!(statement_base);

    fn query_timeout(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        self.retain_cacheable_result(Statement::query_timeout(&self.statement_base))
    }

    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.configure_standard_query_timeout(seconds)
    }

    async fn execute_query(&mut self, sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        self.begin_execution_result();
        let result = Statement::execute_query(&mut self.statement_base, sql).await;
        self.complete_execution_result(result)
    }

    async fn execute_update(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        self.begin_execution_result();
        let result = Statement::execute_update(&mut self.statement_base, sql, generated_keys).await;
        self.complete_execution_result(result)
    }

    async fn execute(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.ensure_open()?;
        self.begin_execution_result();
        let result = Statement::execute(&mut self.statement_base, sql, generated_keys).await;
        self.complete_execution_result(result)
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let parameters = self.parameter_snapshot()?;
        let result = self
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .add_batch(sql);
        self.classify_statement_mutation(result)?;
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .push(parameters);
        Ok(())
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .statement_base
            .clear_batch_with_sql(Some(&self.shared.sql));
        // 基类已完成一次分类和计错；这里只保留预编译缓存失效约束。
        self.retain_cacheable_result(result)?;
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .clear();
        Ok(())
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.ensure_open()?;
        self.begin_execution_result();
        let parameter_sets = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .batch_parameter_sets
            .clone();
        let resource = Arc::clone(&self.shared.resource);
        let result = self
            .statement_base
            .inner
            .policy
            .execute_prepared_batch(
                &self.shared.sql,
                &parameter_sets,
                self.id(),
                move || async move {
                    resource
                        .statement
                        .lock()
                        .await
                        .as_mut()
                        .ok_or(RdbcError::ConnectionDiscarded)?
                        .execute_batch()
                        .await
                },
            )
            .await;
        if result.is_ok() {
            self.shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .batch_parameter_sets
                .clear();
        }
        self.complete_execution_result(result)
    }

    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        self.retain_cacheable_result(
            self.statement_base
                .check_result_property(StatementPropertyEvent::GetGeneratedKeys),
        )?;
        let admission = self.statement_base.inner.begin_standard_admission()?;
        let raw: Box<dyn ResultSet> = if self.shared.has_execution_result.load(Ordering::Acquire) {
            let result = self
                .shared
                .resource
                .statement
                .lock()
                .await
                .as_mut()
                .ok_or(RdbcError::ConnectionDiscarded)?
                .generated_keys()
                .await;
            // getter 驱动错误与参数变更共用分类、失效和缓存淘汰策略。
            self.classify_statement_mutation(result)?
        } else {
            Box::new(druid_rdbc::extensions::BufferedRowSet::with_column_labels(
                Vec::new(),
                vec!["GENERATED_KEY".to_owned()],
            ))
        };
        let result = DruidPooledResultSet::new_standard_auxiliary(
            Arc::clone(&self.statement_base.inner),
            raw,
            &admission,
        )
        .await
        .map(|result_set| Box::new(result_set) as Box<dyn ResultSet>);
        self.retain_cacheable_result(result)
    }

    /// 返回当前执行结果的共享游标，无参数；返回 None 表示当前为更新计数。
    /// 对应 Java: DruidPooledPreparedStatement#getResultSet。
    async fn get_result_set(&mut self) -> Result<Option<Box<dyn ResultSet>>, RdbcError> {
        self.ensure_open()?;
        let result = Statement::get_result_set(&mut self.statement_base).await;
        self.retain_cacheable_result(result)
    }

    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        let count =
            self.retain_cacheable_result(Statement::get_update_count(&self.statement_base))?;
        // 缓存命中继承 raw handle，不继承前一个逻辑租约的执行结果。
        Ok(
            if self.shared.has_execution_result.load(Ordering::Acquire) {
                count
            } else {
                None
            },
        )
    }

    async fn get_more_results(
        &mut self,
        disposition: CurrentResultDisposition,
    ) -> Result<bool, RdbcError> {
        let result = Statement::get_more_results(&mut self.statement_base, disposition).await;
        self.retain_cacheable_result(result)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.close_logical().await
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        if self.is_closed() {
            return Ok(true);
        }
        let raw = self
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?;
        match raw.as_ref() {
            Some(statement) => statement.is_closed(),
            // raw 在清理失败时可以已经释放；逻辑 close 尚未成功，保留 BASE 可观察状态。
            None => Ok(self.statement_base.is_closed()),
        }
    }

    fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.ensure_open()?;
        let result = self
            .statement_base
            .warnings_with_sql(Some(&self.shared.sql));
        self.retain_cacheable_result(result)
    }

    fn warning_messages(&self) -> Result<Vec<String>, RdbcError> {
        let mut messages = Vec::new();
        let mut warning = self.warnings()?;
        while let Some(current) = warning {
            messages.push(current.message().to_owned());
            warning = current.next_warning().cloned();
        }
        Ok(messages)
    }

    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .statement_base
            .clear_warnings_with_sql(Some(&self.shared.sql));
        self.retain_cacheable_result(result)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for DruidPooledPreparedStatement {
    fn sql(&self) -> Result<&str, RdbcError> {
        self.ensure_open()?;
        Ok(&self.shared.sql)
    }

    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let index = parameter.index.get() - 1;
        let result = self
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .set_parameter(parameter.clone());
        self.classify_statement_mutation(result)?;
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.parameters.len() <= index {
            state.parameters.resize_with(index + 1, || None);
        }
        state.parameters[index] = Some(PreparedInputParameter::RustValue(parameter.value));
        Ok(())
    }

    fn set_input_parameter(
        &mut self,
        index: druid_rdbc::statement::ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .set_input_parameter(index, parameter.clone());
        self.classify_statement_mutation(result)?;
        let offset = index.get() - 1;
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.parameters.len() <= offset {
            state.parameters.resize_with(offset + 1, || None);
        }
        state.parameters[offset] = Some(parameter);
        Ok(())
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clear_parameters();
        self.classify_statement_mutation(result)?;
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .parameters
            .clear();
        Ok(())
    }

    fn add_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.add_prepared_batch_core()
    }

    fn add_parameter_batch(&mut self, parameters: Vec<PreparedParameter>) -> Result<(), RdbcError> {
        self.add_parameter_batch_core(parameters)
    }

    fn add_input_parameter_batch(
        &mut self,
        parameters: Vec<(
            druid_rdbc::statement::ParameterIndex,
            PreparedInputParameter,
        )>,
    ) -> Result<(), RdbcError> {
        self.add_input_parameter_batch_core(parameters)
    }

    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.clear_prepared_batch_core()
    }

    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        // 原生 Prepared/Callable 路径也必须绑定执行 SQL，供游标关闭统计和过滤器追踪。
        self.statement_base.begin_single_execution(
            &self.shared.sql,
            super::super::StatementExecuteType::ExecuteQuery,
        );
        self.begin_execution_result();
        let admission = self.statement_base.inner.begin_standard_admission()?;
        let parameters = self.parameter_snapshot()?;
        let resource = Arc::clone(&self.shared.resource);
        let raw = self
            .statement_base
            .inner
            .policy
            .execute_prepared(
                &self.shared.sql,
                parameters,
                self.id(),
                ExecOperation::Query,
                move |_| async move {
                    resource
                        .statement
                        .lock()
                        .await
                        .as_mut()
                        .ok_or(RdbcError::ConnectionDiscarded)?
                        .execute_prepared_query()
                        .await
                },
                |_| ExecResult::default(),
            )
            .await?;
        let result = DruidPooledResultSet::new_standard(
            Arc::clone(&self.statement_base.inner),
            raw,
            &admission,
        )
        .await
        .map(|rows| Box::new(rows) as Box<dyn ResultSet>);
        self.complete_execution_result(result)
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        // 原生 Prepared/Callable 路径也必须绑定执行 SQL，供游标关闭统计和过滤器追踪。
        self.statement_base.begin_single_execution(
            &self.shared.sql,
            super::super::StatementExecuteType::ExecuteUpdate,
        );
        self.begin_execution_result();
        let parameters = self.parameter_snapshot()?;
        let resource = Arc::clone(&self.shared.resource);
        let result = self
            .statement_base
            .inner
            .policy
            .execute_prepared(
                &self.shared.sql,
                parameters,
                self.id(),
                ExecOperation::Update,
                move |_| async move {
                    resource
                        .statement
                        .lock()
                        .await
                        .as_mut()
                        .ok_or(RdbcError::ConnectionDiscarded)?
                        .execute_prepared_update()
                        .await
                },
                |count| ExecResult {
                    rows_affected: u64::try_from(*count).unwrap_or_default(),
                    ..ExecResult::default()
                },
            )
            .await;
        self.complete_execution_result(result)
    }

    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.execute_prepared_results().await
    }

    async fn execute_prepared_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.execute_prepared_batch_core().await
    }

    fn result_set_meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.result_set_meta_data_core()
    }

    fn parameter_meta_data(&self) -> Result<Arc<dyn ParameterMetaData>, RdbcError> {
        self.ensure_open()?;
        self.shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("prepared statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .parameter_meta_data()
    }
}
