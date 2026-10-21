//! 从 `sqlx_prepared_statement.rs` 拆出的实现块。

use super::*;

#[async_trait::async_trait]
impl druid_rdbc::statement::Statement for SqlxPreparedStatement {
    fn options(&self) -> Result<StatementOptions, RdbcError> {
        self.actor()?;
        Ok(self.options)
    }

    fn max_field_size(&self) -> Result<i32, RdbcError> {
        self.actor()?;
        Ok(self.max_field_size.load(Ordering::Acquire))
    }

    fn set_max_field_size(&mut self, max: i32) -> Result<(), RdbcError> {
        self.actor()?;
        if max < 0 {
            return Err(RdbcError::InvalidArgument(
                "max field size must not be negative".to_owned(),
            ));
        }
        self.max_field_size.store(max, Ordering::Release);
        Ok(())
    }

    fn max_rows(&self) -> Result<i64, RdbcError> {
        self.actor()?;
        Ok(self.max_rows.load(Ordering::Acquire))
    }

    fn set_max_rows(&mut self, max: i64) -> Result<(), RdbcError> {
        self.actor()?;
        if max < 0 {
            return Err(RdbcError::InvalidArgument(
                "max rows must not be negative".to_owned(),
            ));
        }
        self.max_rows.store(max, Ordering::Release);
        Ok(())
    }

    fn set_escape_processing(&mut self, _enabled: bool) -> Result<(), RdbcError> {
        self.actor()?;
        Ok(())
    }

    fn query_timeout(&self) -> Result<i32, RdbcError> {
        self.actor()?;
        Ok(self.query_timeout.load(Ordering::Acquire))
    }

    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.actor()?;
        if seconds < 0 {
            return Err(RdbcError::InvalidArgument(
                "query timeout must not be negative".to_owned(),
            ));
        }
        self.query_timeout.store(seconds, Ordering::Release);
        Ok(())
    }

    fn set_fetch_direction(&mut self, direction: FetchDirection) -> Result<(), RdbcError> {
        self.actor()?;
        *self
            .fetch_direction
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = direction;
        Ok(())
    }

    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        self.actor()?;
        Ok(*self
            .fetch_direction
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))
    }

    fn set_fetch_size(&mut self, rows: i32) -> Result<(), RdbcError> {
        self.actor()?;
        if rows < 0 {
            return Err(RdbcError::InvalidArgument(
                "fetch size must not be negative".to_owned(),
            ));
        }
        self.fetch_size.store(rows, Ordering::Release);
        Ok(())
    }

    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.actor()?;
        Ok(self.fetch_size.load(Ordering::Acquire))
    }

    async fn execute_query(
        &mut self,
        sql: &str,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        let _execution = self.reset_execution_results()?;
        let actor = Arc::clone(self.actor()?);
        self.execute_query_values(&actor, sql, Vec::new(), true)
            .await
    }

    async fn execute_update(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        let _execution = self.reset_execution_results()?;
        let actor = Arc::clone(self.actor()?);
        let rows_affected = self
            .execute_update_values(sql, Vec::new(), &generated_keys)
            .await?;
        actor.task4_trace_statement_update();
        Ok(rows_affected)
    }

    async fn execute(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        let _execution = self.reset_execution_results()?;
        let actor = Arc::clone(self.actor()?);
        if actor
            .statement_returns_rows_with_control(
                sql,
                self.query_timeout.load(Ordering::Acquire),
                self.cancellation.interruption(),
            )
            .await?
        {
            self.last_update_count.store(-1, Ordering::Release);
            Ok(vec![ExecutionResult::ResultSet(
                actor
                    .execute_query_with_timeout(
                        sql,
                        Vec::new(),
                        self.query_timeout.load(Ordering::Acquire),
                        self.cancellation.interruption(),
                    )
                    .await?,
            )])
        } else {
            let count = self
                .execute_update_values(sql, Vec::new(), &generated_keys)
                .await?;
            Ok(vec![ExecutionResult::UpdateCount(count)])
        }
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.actor()?;
        self.statement_batch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(sql);
        Ok(())
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.actor()?;
        self.statement_batch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        Ok(())
    }

    fn batch(&self) -> Result<Vec<String>, RdbcError> {
        self.actor()?;
        Ok(self
            .statement_batch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        let _execution = self.reset_execution_results()?;
        self.actor()?;
        let batch = std::mem::take(
            &mut *self
                .statement_batch
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        let mut counts = Vec::with_capacity(batch.len());
        for sql in batch {
            match self
                .execute_update_values(&sql, Vec::new(), &GeneratedKeys::None)
                .await
            {
                Ok(count) => counts.push(count),
                Err(cause) => {
                    return Err(RdbcError::BatchUpdateException {
                        update_counts: counts.into(),
                        cause: Box::new(cause),
                    });
                }
            }
        }
        Ok(counts)
    }

    async fn generated_keys(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        self.actor()?;
        let keys = std::mem::take(
            &mut *self
                .generated_key
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        let rows = keys
            .into_iter()
            .map(|value| druid_rdbc::types::Row::new(vec![Value::Int(value)]))
            .collect();
        Ok(Box::new(BufferedRowSet::with_column_labels(
            rows,
            vec!["GENERATED_KEY".to_owned()],
        )))
    }

    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        self.actor()?;
        let count = self.last_update_count.load(Ordering::Acquire);
        Ok((count >= 0).then_some(count))
    }

    fn set_poolable(&mut self, poolable: bool) -> Result<(), RdbcError> {
        self.actor()?;
        self.poolable.store(poolable, Ordering::Release);
        Ok(())
    }

    fn is_poolable(&self) -> Result<bool, RdbcError> {
        self.actor()?;
        Ok(self.poolable.load(Ordering::Acquire))
    }

    fn close_on_completion(&mut self) -> Result<(), RdbcError> {
        self.actor()?;
        self.close_on_completion.store(true, Ordering::Release);
        Ok(())
    }

    fn is_close_on_completion(&self) -> Result<bool, RdbcError> {
        self.actor()?;
        Ok(self.close_on_completion.load(Ordering::Acquire))
    }

    fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.actor()?;
        Ok(None)
    }

    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.actor()?;
        Ok(())
    }

    /// 取消当前活动游标；不支持中断活动执行的驱动会在下一次执行前消费请求。
    /// 对应 Java: Statement#cancel。语句属性和连接保持可复用。
    fn cancel(&mut self) -> Result<(), RdbcError> {
        let actor = self.actor()?;
        self.cancellation
            .cancel_current_or_next(actor.backend_key() == "sqlite")
    }

    fn cancellation_control(&self) -> Result<Arc<dyn StatementCancellation>, RdbcError> {
        let actor = self.actor()?;
        if actor.backend_key() != "sqlite" {
            return Err(RdbcError::feature_not_supported(
                "sqlx_statement_cancellation_control",
            ));
        }
        self.cancellation.control()
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        // 子游标持有同一物理连接；语句关闭或 Sender Drop 都会终止其任务。
        self.cancellation.close();
        self.closed.store(true, Ordering::Release);
        self.actor.take();
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed.load(Ordering::Acquire))
    }

    async fn execute_values(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        let _execution = self.reset_execution_results()?;
        let actor = Arc::clone(self.actor()?);
        self.execute_query_values(&actor, sql, values, true).await
    }
}
