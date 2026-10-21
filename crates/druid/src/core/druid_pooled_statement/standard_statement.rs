use super::*;

#[async_trait::async_trait]
impl Statement for DruidPooledStatement {
    /// 按 JDBC 整数处置推进结果；先检查逻辑关闭和租约，再验证参数且不改变游标。
    /// 对应 Java：DruidPooledStatement#getMoreResults(int)。返回下个结果是否为游标。
    async fn get_more_results_current(&mut self, current: i32) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let disposition = match CurrentResultDisposition::try_from(current) {
            Ok(disposition) => disposition,
            Err(error) => {
                // Java 过滤器先看到原始调用；拒绝不应被代理的整数校验掩盖。
                self.check_result_property(StatementPropertyEvent::GetMoreResults)?;
                return Err(error);
            }
        };
        self.get_more_results(disposition).await
    }

    fn options(&self) -> Result<StatementOptions, RdbcError> {
        self.ensure_open()?;
        Ok(self.state().options)
    }

    fn result_set_type(&self) -> Result<ResultSetType, RdbcError> {
        let cached = self.state().options.result_set_type();
        self.read_statement_property(
            StatementPropertyEvent::GetResultSetType,
            "statement_result_set_type",
            cached,
            Statement::result_set_type,
        )
    }

    fn result_set_concurrency(&self) -> Result<ResultSetConcurrency, RdbcError> {
        let cached = self.state().options.concurrency();
        self.read_statement_property(
            StatementPropertyEvent::GetResultSetConcurrency,
            "statement_result_set_concurrency",
            cached,
            Statement::result_set_concurrency,
        )
    }

    fn result_set_holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        let cached = self.state().options.holdability();
        self.read_statement_property(
            StatementPropertyEvent::GetResultSetHoldability,
            "statement_result_set_holdability",
            cached,
            Statement::result_set_holdability,
        )
    }

    fn max_field_size(&self) -> Result<i32, RdbcError> {
        let cached = self.state().max_field_size;
        self.read_statement_property(
            StatementPropertyEvent::GetMaxFieldSize,
            "statement_max_field_size",
            cached,
            Statement::max_field_size,
        )
    }

    fn set_max_field_size(&mut self, max: i32) -> Result<(), RdbcError> {
        self.apply_statement_mutation(StatementMutation::SetMaxFieldSize(max))
    }

    fn max_rows(&self) -> Result<i64, RdbcError> {
        let cached = self.state().max_rows;
        self.read_statement_property(
            StatementPropertyEvent::GetMaxRows,
            "statement_max_rows",
            cached,
            Statement::max_rows,
        )
    }

    fn set_max_rows(&mut self, max: i64) -> Result<(), RdbcError> {
        self.apply_statement_mutation(StatementMutation::SetMaxRows(max))
    }

    fn set_escape_processing(&mut self, enabled: bool) -> Result<(), RdbcError> {
        self.apply_statement_mutation(StatementMutation::SetEscapeProcessing(enabled))
    }

    fn query_timeout(&self) -> Result<i32, RdbcError> {
        let cached = self.state().query_timeout;
        self.read_statement_property(
            StatementPropertyEvent::GetQueryTimeout,
            "statement_query_timeout",
            cached,
            Statement::query_timeout,
        )
    }

    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.configure_standard_query_timeout(seconds)
    }

    fn cancel(&mut self) -> Result<(), RdbcError> {
        self.apply_statement_mutation(StatementMutation::Cancel)
    }

    fn cancellation_control(
        &self,
    ) -> Result<Arc<dyn druid_rdbc::statement::StatementCancellation>, RdbcError> {
        self.ensure_open()?;
        let control = self
            .inner
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?
            .cancellation_control()?;
        Ok(Arc::new(PooledStatementCancellation {
            inner: Arc::downgrade(&self.inner),
            control,
        }))
    }

    fn set_cursor_name(&mut self, name: &str) -> Result<(), RdbcError> {
        self.apply_statement_mutation(StatementMutation::SetCursorName(name.to_owned()))
    }

    fn set_cursor_name_nullable(&mut self, name: Option<&str>) -> Result<(), RdbcError> {
        self.apply_statement_mutation(StatementMutation::SetCursorNameNullable(
            name.map(str::to_owned),
        ))
    }

    fn set_fetch_direction(&mut self, direction: FetchDirection) -> Result<(), RdbcError> {
        self.apply_statement_mutation(StatementMutation::SetFetchDirection(direction))
    }

    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        let cached = self.state().fetch_direction;
        self.read_statement_property(
            StatementPropertyEvent::GetFetchDirection,
            "statement_fetch_direction",
            cached,
            Statement::fetch_direction,
        )
    }

    fn set_fetch_size(&mut self, rows: i32) -> Result<(), RdbcError> {
        self.apply_statement_mutation(StatementMutation::SetFetchSize(rows))
    }

    fn fetch_size(&self) -> Result<i32, RdbcError> {
        let cached = self.state().fetch_size;
        self.read_statement_property(
            StatementPropertyEvent::GetFetchSize,
            "statement_fetch_size",
            cached,
            Statement::fetch_size,
        )
    }

    fn set_poolable(&mut self, poolable: bool) -> Result<(), RdbcError> {
        self.apply_statement_mutation(StatementMutation::SetPoolable(poolable))
    }

    fn is_poolable(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        Ok(self.state().poolable)
    }

    fn close_on_completion(&mut self) -> Result<(), RdbcError> {
        self.apply_statement_mutation(StatementMutation::CloseOnCompletion)
    }

    fn is_close_on_completion(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        Ok(self.state().close_on_completion)
    }

    async fn execute_query(
        &mut self,
        sql: &str,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        self.ensure_open()?;
        let admission = self.inner.begin_standard_admission()?;
        self.begin_single_execution(sql, StatementExecuteType::ExecuteQuery);
        let started = Instant::now();
        let inner = Arc::clone(&self.inner);
        let raw = self
            .inner
            .policy
            .execute(
                sql,
                Vec::new(),
                self.id(),
                ExecOperation::Query,
                move |effective_sql| async move {
                    inner
                        .statement
                        .lock()
                        .await
                        .execute_query(&effective_sql)
                        .await
                },
                |_| ExecResult::default(),
            )
            .await;
        let raw = match raw {
            Ok(raw) => raw,
            Err(error) => {
                self.record_exception();
                return Err(error);
            }
        };
        self.state_mut().last_execute_elapsed = Some(started.elapsed());
        Ok(Box::new(
            DruidPooledResultSet::new_standard(Arc::clone(&self.inner), raw, &admission).await?,
        ))
    }

    async fn execute_update(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        self.begin_single_execution(sql, StatementExecuteType::ExecuteUpdate);
        let started = Instant::now();
        let inner = Arc::clone(&self.inner);
        let result = self
            .inner
            .policy
            .execute(
                sql,
                Vec::new(),
                self.id(),
                ExecOperation::Update,
                move |effective_sql| async move {
                    inner
                        .statement
                        .lock()
                        .await
                        .execute_update(&effective_sql, generated_keys)
                        .await
                },
                |count| ExecResult {
                    rows_affected: u64::try_from(*count).unwrap_or_default(),
                    ..ExecResult::default()
                },
            )
            .await;
        match result {
            Ok(count) => {
                let mut state = self.state_mut();
                state.last_execute_elapsed = Some(started.elapsed());
                state.update_count = count;
                Ok(count)
            }
            Err(error) => {
                self.record_exception();
                Err(error)
            }
        }
    }

    async fn execute(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.ensure_open()?;
        let admission = self.inner.begin_standard_admission()?;
        self.begin_single_execution(sql, StatementExecuteType::Execute);
        let inner = Arc::clone(&self.inner);
        let results = self
            .inner
            .policy
            .execute(
                sql,
                Vec::new(),
                self.id(),
                ExecOperation::Execute,
                move |effective_sql| async move {
                    inner
                        .statement
                        .lock()
                        .await
                        .execute(&effective_sql, generated_keys)
                        .await
                },
                |results| match results.first() {
                    Some(ExecutionResult::UpdateCount(count)) => ExecResult {
                        rows_affected: u64::try_from(*count).unwrap_or_default(),
                        ..ExecResult::default()
                    },
                    Some(ExecutionResult::ResultSet(rows)) => ExecResult {
                        row_count: rows.known_row_count(),
                        ..ExecResult::default()
                    },
                    None => ExecResult::default(),
                },
            )
            .await;
        let results = match results {
            Ok(results) => results,
            Err(error) => {
                self.record_exception();
                return Err(error);
            }
        };
        DruidPooledResultSet::new_standard_results(Arc::clone(&self.inner), results, &admission)
            .await
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let effective_sql = self.classify_statement_access(
            DruidExecutionFilterPolicy::statement_add_batch_sql(
                self.inner.filter_chain.as_deref(),
                &sql,
            )
            .map_err(RdbcError::from),
            Some(&sql),
        )?;
        let result = self
            .inner
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?
            .add_batch(effective_sql.clone());
        self.classify_statement_access(result, Some(&effective_sql))?;
        self.state_mut().batch.push(effective_sql);
        Ok(())
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.clear_batch_with_sql(None)
    }

    fn batch(&self) -> Result<Vec<String>, RdbcError> {
        self.ensure_open()?;
        Ok(self.state().batch.clone())
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.ensure_open()?;
        self.begin_batch_execution();
        let statements = self.state().batch.clone();
        let sql = statements.join("\n;\n");
        let inner = Arc::clone(&self.inner);
        let result = self
            .inner
            .policy
            .execute_statement_batch(&sql, &statements, self.id(), move || async move {
                inner.statement.lock().await.execute_batch().await
            })
            .await;
        match result {
            Ok(counts) => {
                let mut state = self.state_mut();
                state.batch.clear();
                state.update_count = if counts.len() == 1 { counts[0] } else { -1 };
                Ok(counts)
            }
            Err(error) => {
                self.record_exception();
                Err(error)
            }
        }
    }

    async fn generated_keys(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        self.check_result_property(StatementPropertyEvent::GetGeneratedKeys)?;
        let admission = self.inner.begin_standard_admission()?;
        let raw = self.inner.statement.lock().await.generated_keys().await;
        let raw = self.classify_statement_mutation(raw)?;
        Ok(Box::new(
            DruidPooledResultSet::new_standard_auxiliary(Arc::clone(&self.inner), raw, &admission)
                .await?,
        ))
    }

    async fn get_result_set(
        &mut self,
    ) -> Result<Option<Box<dyn druid_rdbc::result_set::ResultSet>>, RdbcError> {
        self.check_result_property(StatementPropertyEvent::GetResultSet)?;
        // 即使只恢复共享视图，也保留原 getter 的 registry generation 入场检查。
        let admission = self.inner.begin_standard_admission()?;
        // 已登记的游标保留共享身份；raw getter 仍参与驱动错误分类和拥有型交接。
        let tracked = {
            let sequence = self
                .inner
                .result_sequence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            sequence
                .as_ref()
                .map(|sequence| match sequence.results.get(sequence.current) {
                    Some(TrackedStatementResult::ResultSet(resource)) => resource.upgrade(),
                    Some(TrackedStatementResult::UpdateCount(_)) | None => None,
                })
        };
        let raw = self.inner.statement.lock().await.get_result_set().await;
        let raw = match raw {
            Err(RdbcError::UnsupportedOperation {
                operation: "statement_get_result_set",
            }) if tracked.is_some() => None,
            result => self.classify_statement_mutation(result)?,
        };
        if let Some(Some(resource)) = tracked {
            if let Some(raw) = raw {
                resource.adopt_current_view(raw).await?;
            }
            return DruidPooledResultSet::from_tracked_resource(Arc::clone(&self.inner), resource)
                .map(
                    |result| Some(Box::new(result) as Box<dyn druid_rdbc::result_set::ResultSet>),
                );
        }
        match raw {
            Some(raw) => Ok(Some(Box::new(
                DruidPooledResultSet::new_standard(Arc::clone(&self.inner), raw, &admission)
                    .await?,
            ))),
            None => Ok(None),
        }
    }

    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        self.check_result_property(StatementPropertyEvent::GetUpdateCount)?;
        let raw = {
            let statement = self
                .inner
                .statement
                .try_lock()
                .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?;
            statement.get_update_count()
        };
        let raw = match raw {
            Err(RdbcError::UnsupportedOperation {
                operation: "statement_get_update_count",
            }) => None,
            result => self.classify_statement_mutation(result)?,
        };
        let count = self.state().update_count;
        // eager execute 已登记完整混合序列；driver getter 仍须接受错误分类，
        // 但它的单次执行计数不能覆盖逻辑序列当前的 ResultSet/UpdateCount。
        let has_sequence = self
            .inner
            .result_sequence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        if has_sequence || count >= 0 {
            Ok((count >= 0).then_some(count))
        } else {
            Ok(raw)
        }
    }

    async fn get_more_results(
        &mut self,
        disposition: CurrentResultDisposition,
    ) -> Result<bool, RdbcError> {
        self.check_result_property(StatementPropertyEvent::GetMoreResults)?;
        let (has_sequence, current) = {
            let sequence = self
                .inner
                .result_sequence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let current = sequence.as_ref().and_then(|sequence| {
                match sequence.results.get(sequence.current) {
                    Some(TrackedStatementResult::ResultSet(result_set)) => Some(result_set.clone()),
                    Some(TrackedStatementResult::UpdateCount(_)) | None => None,
                }
            });
            (sequence.is_some(), current)
        };
        self.inner.dispose_result_sets(disposition, current).await?;
        let raw = self
            .inner
            .statement
            .lock()
            .await
            .get_more_results(disposition)
            .await;
        let raw = match raw {
            Err(RdbcError::UnsupportedOperation {
                operation: "statement_get_more_results",
            }) if has_sequence => false,
            result => self.classify_statement_mutation(result)?,
        };
        if has_sequence {
            let (has_more, update_count) = {
                let mut sequence_guard = self
                    .inner
                    .result_sequence
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let Some(sequence) = sequence_guard.as_mut() else {
                    return Ok(false);
                };
                sequence.current = sequence.current.saturating_add(1);
                let next = match sequence.results.get(sequence.current) {
                    Some(TrackedStatementResult::ResultSet(_)) => Some((true, -1)),
                    Some(TrackedStatementResult::UpdateCount(count)) => Some((false, *count)),
                    None => None,
                };
                if let Some(next) = next {
                    next
                } else {
                    // 保留已耗尽的逻辑执行身份，后续 getter/推进不能退回 raw 默认实现。
                    sequence.current = sequence.results.len();
                    (false, -1)
                }
            };
            let mut state = self.state_mut();
            state.first_result_set = has_more;
            state.update_count = update_count;
            return Ok(has_more);
        }
        let has_more = raw;
        let mut state = self.state_mut();
        state.first_result_set = has_more;
        state.update_count = -1;
        Ok(has_more)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.inner.close_standard_core().await
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        if self.is_closed() || !self.inner.lease_active.load(Ordering::Acquire) {
            return Ok(true);
        }
        self.inner
            .statement
            .try_lock()
            .map_err(|_| RdbcError::Other("statement is busy".to_owned()))?
            .is_closed()
    }

    fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.warnings_with_sql(self.last_sql().as_deref())
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
        self.clear_warnings_with_sql(self.last_sql().as_deref())
    }
}
