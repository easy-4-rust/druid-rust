use super::*;

#[async_trait::async_trait]
impl Statement for DruidPooledCallableStatement {
    /// 按 JDBC 整数处置推进结果；先检查逻辑关闭和租约，再验证参数且不改变游标。
    /// 对应 Java：DruidPooledCallableStatement#getMoreResults(int)。返回下个结果是否为游标。
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
        self.configure_standard_query_timeout(seconds)
    }

    async fn execute_query(&mut self, sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        let result = Statement::execute_query(&mut self.statement_base, sql).await;
        self.retain_cacheable_result(result)
    }

    async fn execute_update(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        let result = Statement::execute_update(&mut self.statement_base, sql, generated_keys).await;
        self.retain_cacheable_result(result)
    }

    async fn execute(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.ensure_open()?;
        let result = Statement::execute(&mut self.statement_base, sql, generated_keys).await;
        self.retain_cacheable_result(result)
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
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
        // 基类已完成一次分类和计错；这里只保留调用语句缓存失效约束。
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
        self.retain_cacheable_result(result)
    }

    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        self.retain_cacheable_result(
            self.statement_base
                .check_result_property(StatementPropertyEvent::GetGeneratedKeys),
        )?;
        let admission = self.statement_base.inner.begin_standard_admission()?;
        let raw = self
            .shared
            .resource
            .statement
            .lock()
            .await
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .generated_keys()
            .await;
        let raw = self.classify_statement_mutation(raw)?;
        Ok(Box::new(
            DruidPooledResultSet::new_standard_auxiliary(
                Arc::clone(&self.statement_base.inner),
                raw,
                &admission,
            )
            .await?,
        ))
    }

    /// 返回当前执行的共享结果集，无参数；当前为更新计数时返回 None。
    /// 对应 Java: DruidPooledCallableStatement#getResultSet。
    async fn get_result_set(&mut self) -> Result<Option<Box<dyn ResultSet>>, RdbcError> {
        self.ensure_open()?;
        let result = Statement::get_result_set(&mut self.statement_base).await;
        self.retain_cacheable_result(result)
    }

    /// 返回当前更新计数，无参数；当前无更新结果时返回 None。
    /// 对应 Java: DruidPooledCallableStatement#getUpdateCount。
    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        self.ensure_open()?;
        let result = Statement::get_update_count(&self.statement_base);
        self.retain_cacheable_result(result)
    }

    /// 按指定处置推进共享主结果序列，返回下一结果是否为游标。
    /// 对应 Java: DruidPooledCallableStatement#getMoreResults。
    async fn get_more_results(
        &mut self,
        disposition: CurrentResultDisposition,
    ) -> Result<bool, RdbcError> {
        self.ensure_open()?;
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
        self.shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .is_closed()
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
impl PreparedStatement for DruidPooledCallableStatement {
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
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
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
        index: ParameterIndex,
        parameter: PreparedInputParameter,
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
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
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
        parameters: Vec<(ParameterIndex, PreparedInputParameter)>,
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
            .await;
        let raw = self.retain_cacheable_result(raw)?;
        Ok(Box::new(
            DruidPooledResultSet::new_standard(
                Arc::clone(&self.statement_base.inner),
                raw,
                &admission,
            )
            .await?,
        ))
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        // 原生 Prepared/Callable 路径也必须绑定执行 SQL，供游标关闭统计和过滤器追踪。
        self.statement_base.begin_single_execution(
            &self.shared.sql,
            super::super::StatementExecuteType::ExecuteUpdate,
        );
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
        self.retain_cacheable_result(result)
    }

    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.execute_prepared_results().await
    }

    async fn execute_prepared_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        let result = self.execute_prepared_batch_core().await;
        self.retain_cacheable_result(result)
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
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .parameter_meta_data()
    }
}

impl CallableStatement for DruidPooledCallableStatement {
    /// 校验当前语句的原始索引，参数错误使真实句柄退出缓存并记录语句异常。
    /// 参数 parameter_index 保留调用者值；返回严格 selector 或关闭/索引错误。
    /// 对应 Java: DruidPooledCallableStatement#index_parameter / record_result。
    fn index_parameter(&self, parameter_index: usize) -> Result<CallableParameter, RdbcError> {
        self.ensure_open()?;
        let result = self.classify_statement_mutation(CallableParameter::by_index(parameter_index));
        if result.is_err() {
            self.statement_base.record_exception();
        }
        result
    }

    fn register_out_parameter(
        &mut self,
        parameter: CallableParameter,
        out_parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let out_index = match &parameter {
            CallableParameter::Index(index) => Some(index.get()),
            CallableParameter::Name(_) => None,
        };
        let result = self
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .register_out_parameter(parameter, out_parameter);
        self.classify_statement_mutation(result)?;
        if let Some(index) = out_index {
            let mut state = self
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.out_indexes.insert(index);
            if state.parameters.len() < index {
                state.parameters.resize_with(index, || None);
            }
        }
        Ok(())
    }

    fn set_named_parameter(
        &mut self,
        parameter_name: &str,
        parameter: CallableInputParameter,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        // 对应 Java Callable 命名 setter：空名称在逻辑层拒绝，不能交给 driver 接受。
        if parameter_name.is_empty() {
            return self.classify_statement_mutation(Err(RdbcError::InvalidArgument(
                "callable parameter name is empty".to_owned(),
            )));
        }
        let result = self
            .shared
            .resource
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("callable statement is busy".to_owned()))?
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .set_named_parameter(parameter_name, parameter);
        self.classify_statement_mutation(result)
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.read_out_result(|raw| raw.out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn out_parameter_with_type_map(
        &self,
        _parameter: &CallableParameter,
        _type_map: Option<&druid_rdbc::types::RdbcTypeMap>,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.read_out_result(|raw| raw.out_parameter_with_type_map(_parameter, _type_map))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn out_parameter_as(
        &self,
        _parameter: &CallableParameter,
        _target_type: &druid_rdbc::types::RdbcTargetType,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.read_out_result(|raw| raw.out_parameter_as(_parameter, _target_type))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn out_value(&self, _index: ParameterIndex) -> Result<Value, RdbcError> {
        self.read_out_result(|raw| raw.out_value(_index))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn was_null(&self) -> Result<bool, RdbcError> {
        self.read_out_result(|raw| {
            if let Some(filters) = &self.statement_base.inner.filter_chain {
                // 让过滤器先于驱动读取；错误统一由外层读取策略分类、计数和撤销缓存资格。
                filters
                    .standard_callable_was_null(&mut || {
                        raw.was_null().map_err(super::super::DruidError::from)
                    })
                    .map_err(RdbcError::from)
            } else {
                raw.was_null()
            }
        })
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn string_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<String>, RdbcError> {
        self.read_out_result(|raw| raw.string_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn n_string_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<String>, RdbcError> {
        self.read_out_result(|raw| raw.n_string_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn url_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcUrl>, RdbcError> {
        self.read_out_result(|raw| raw.url_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn ref_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcRef>, RdbcError> {
        self.read_out_result(|raw| raw.ref_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn array_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcArray>, RdbcError> {
        self.read_out_result(|raw| raw.array_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn row_id_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcRowId>, RdbcError> {
        self.read_out_result(|raw| raw.row_id_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn sql_xml_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcSqlXml>, RdbcError> {
        self.read_out_result(|raw| raw.sql_xml_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn boolean_out_parameter(&self, parameter: &CallableParameter) -> Result<bool, RdbcError> {
        self.read_out_result(|raw| raw.boolean_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn byte_out_parameter(&self, parameter: &CallableParameter) -> Result<i8, RdbcError> {
        self.read_out_result(|raw| raw.byte_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn short_out_parameter(&self, parameter: &CallableParameter) -> Result<i16, RdbcError> {
        self.read_out_result(|raw| raw.short_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn int_out_parameter(&self, parameter: &CallableParameter) -> Result<i32, RdbcError> {
        self.read_out_result(|raw| raw.int_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn long_out_parameter(&self, parameter: &CallableParameter) -> Result<i64, RdbcError> {
        self.read_out_result(|raw| raw.long_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn float_out_parameter(&self, parameter: &CallableParameter) -> Result<f32, RdbcError> {
        self.read_out_result(|raw| raw.float_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn double_out_parameter(&self, parameter: &CallableParameter) -> Result<f64, RdbcError> {
        self.read_out_result(|raw| raw.double_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn bytes_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<Vec<u8>>, RdbcError> {
        self.read_out_result(|raw| raw.bytes_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn blob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcBlob>, RdbcError> {
        self.read_out_result(|raw| raw.blob_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn clob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcClob>, RdbcError> {
        self.read_out_result(|raw| raw.clob_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn n_clob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcNClob>, RdbcError> {
        self.read_out_result(|raw| raw.n_clob_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn character_stream_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcReader>, RdbcError> {
        self.read_out_result(|raw| raw.character_stream_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn n_character_stream_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<druid_rdbc::types::RdbcReader>, RdbcError> {
        self.read_out_result(|raw| raw.n_character_stream_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn big_decimal_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<bigdecimal::BigDecimal>, RdbcError> {
        self.read_out_result(|raw| raw.big_decimal_out_parameter(parameter))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn big_decimal_out_parameter_with_scale(
        &self,
        parameter: &CallableParameter,
        scale: i32,
    ) -> Result<Option<bigdecimal::BigDecimal>, RdbcError> {
        self.read_out_result(|raw| raw.big_decimal_out_parameter_with_scale(parameter, scale))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn date_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &druid_rdbc::types::RdbcCalendarArgument,
    ) -> Result<Option<chrono::NaiveDate>, RdbcError> {
        self.read_out_result(|raw| raw.date_out_parameter(parameter, _calendar))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn time_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &druid_rdbc::types::RdbcCalendarArgument,
    ) -> Result<Option<chrono::NaiveTime>, RdbcError> {
        self.read_out_result(|raw| raw.time_out_parameter(parameter, _calendar))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn timestamp_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &druid_rdbc::types::RdbcCalendarArgument,
    ) -> Result<Option<chrono::NaiveDateTime>, RdbcError> {
        self.read_out_result(|raw| raw.timestamp_out_parameter(parameter, _calendar))
    }

    /// 保留底层 OUT getter 与参数描述符；失败使当前语句退出缓存。
    fn integer_out_parameter(&self, parameter: &CallableParameter) -> Result<i64, RdbcError> {
        self.read_out_result(|raw| raw.integer_out_parameter(parameter))
    }
}
