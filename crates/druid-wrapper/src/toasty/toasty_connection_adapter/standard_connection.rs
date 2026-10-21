//! 从 `toasty_connection_adapter.rs` 拆出的实现块。

use super::*;

#[async_trait::async_trait]
impl druid_rdbc::connection::Connection for ToastyConnectionAdapter {
    async fn exec(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<druid_rdbc::statement::ExecResult, RdbcError> {
        let result = if let Some(executor) = &self.standard_executor {
            executor.execute_update_result(sql, params).await?
        } else {
            self.execute_update_core(sql, params).await?
        };
        Ok(druid_rdbc::statement::ExecResult {
            rows_affected: result.rows_affected,
            last_insert_id: result.last_insert_id,
            row_count: result.row_count,
        })
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        if let Some(executor) = &self.standard_executor {
            executor
                .execute_query(sql, params)
                .await
                .map(|result| result.rows)
        } else {
            self.execute_query_core(sql, params).await
        }
    }

    async fn begin(&mut self) -> Result<(), RdbcError> {
        if !self.auto_commit {
            return Err(RdbcError::DriverError(
                "Toasty transaction is already active".to_owned(),
            ));
        }
        if let Some(executor) = &self.standard_executor {
            executor.execute_update("BEGIN", Vec::new()).await?;
        } else {
            self.execute_operation(
                Transaction::Start {
                    isolation: self.isolation,
                    read_only: self.read_only,
                    mode: TransactionMode::Default,
                }
                .into(),
            )
            .await?;
        }
        self.auto_commit = false;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        self.ensure_transaction()?;
        if let Some(executor) = &self.standard_executor {
            executor.execute_update("COMMIT", Vec::new()).await?;
        } else {
            self.execute_operation(Transaction::Commit.into()).await?;
        }
        self.auto_commit = true;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        self.ensure_transaction()?;
        if let Some(executor) = &self.standard_executor {
            executor.execute_update("ROLLBACK", Vec::new()).await?;
        } else {
            self.execute_operation(Transaction::Rollback.into()).await?;
        }
        self.auto_commit = true;
        Ok(())
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), RdbcError> {
        self.ensure_transaction()?;
        let name = Self::savepoint_name(savepoint);
        Self::validate_savepoint_name(&name)?;
        if self.standard_executor.is_some() {
            self.execute_transaction_sql(&format!("ROLLBACK TO SAVEPOINT {name}"))
                .await
        } else {
            self.execute_operation(Transaction::RollbackToSavepoint(name).into())
                .await
                .map(|_| ())
        }
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, RdbcError> {
        self.ensure_transaction()?;
        self.savepoint_sequence = self.savepoint_sequence.saturating_add(1);
        let savepoint = Savepoint {
            id: self.savepoint_sequence,
            name: None,
        };
        let name = Self::savepoint_name(&savepoint);
        if self.standard_executor.is_some() {
            self.execute_transaction_sql(&format!("SAVEPOINT {name}"))
                .await?;
        } else {
            self.execute_operation(Transaction::Savepoint(name).into())
                .await?;
        }
        Ok(savepoint)
    }

    async fn set_savepoint_named(&mut self, name: &str) -> Result<Savepoint, RdbcError> {
        self.ensure_transaction()?;
        Self::validate_savepoint_name(name)?;
        self.savepoint_sequence = self.savepoint_sequence.saturating_add(1);
        if self.standard_executor.is_some() {
            self.execute_transaction_sql(&format!("SAVEPOINT {name}"))
                .await?;
        } else {
            self.execute_operation(Transaction::Savepoint(name.to_owned()).into())
                .await?;
        }
        Ok(Savepoint {
            id: self.savepoint_sequence,
            name: Some(name.to_owned()),
        })
    }

    async fn release_savepoint(&mut self, savepoint: &Savepoint) -> Result<(), RdbcError> {
        self.ensure_transaction()?;
        let name = Self::savepoint_name(savepoint);
        Self::validate_savepoint_name(&name)?;
        if self.standard_executor.is_some() {
            self.execute_transaction_sql(&format!("RELEASE SAVEPOINT {name}"))
                .await
        } else {
            self.execute_operation(Transaction::ReleaseSavepoint(name).into())
                .await
                .map(|_| ())
        }
    }

    async fn set_auto_commit(&mut self, enabled: bool) -> Result<(), RdbcError> {
        match (self.auto_commit, enabled) {
            (true, false) => self.begin().await,
            (false, true) => self.commit().await,
            _ => Ok(()),
        }
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), RdbcError> {
        if !self.auto_commit {
            return Err(RdbcError::InvalidArgument(
                "read_only cannot change while a transaction is active".to_owned(),
            ));
        }
        if self.is_closed()? {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let Some(sql) = read_only_session_sql(self.driver_name, read_only) else {
            if read_only {
                return Err(RdbcError::UnsupportedOperation {
                    operation: if self.is_sqlite() {
                        "toasty_sqlite_read_only_transaction"
                    } else {
                        "toasty_driver_read_only_transaction"
                    },
                });
            }
            self.read_only = false;
            return Ok(());
        };
        if self.standard_executor.is_some() {
            self.execute_transaction_sql(sql).await?;
        } else {
            self.execute_operation(Self::raw_sql(sql, Vec::new(), RawSqlRet::None)?)
                .await?;
        }
        self.read_only = read_only;
        Ok(())
    }

    async fn set_transaction_isolation(
        &mut self,
        level: TransactionIsolation,
    ) -> Result<(), RdbcError> {
        if !self.auto_commit {
            return Err(RdbcError::InvalidArgument(
                "transaction isolation cannot change while a transaction is active".to_owned(),
            ));
        }
        let level = level.jdbc_code();
        if self.is_sqlite() && level != TransactionIsolation::SERIALIZABLE.jdbc_code() {
            return Err(RdbcError::InvalidArgument(
                "Toasty SQLite only supports RDBC TRANSACTION_SERIALIZABLE (8)".to_owned(),
            ));
        }
        self.isolation = Some(match level {
            1 => IsolationLevel::ReadUncommitted,
            2 => IsolationLevel::ReadCommitted,
            4 => IsolationLevel::RepeatableRead,
            8 => IsolationLevel::Serializable,
            _ => {
                return Err(RdbcError::InvalidArgument(format!(
                    "unsupported RDBC transaction isolation level: {level}"
                )));
            }
        });
        Ok(())
    }

    async fn create_statement(
        &mut self,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::Statement>, RdbcError> {
        Ok(Box::new(StandardAdapterStatement::new(
            self.standard_executor_core()?,
            options,
        )))
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        let executor = self.standard_executor_core()?;
        let parameter_count = executor.parameter_count(sql).await?;
        Ok(Box::new(
            StandardAdapterPreparedStatement::new(
                sql.to_owned(),
                executor,
                parameter_count,
                options,
            )
            .with_eager_resource_binding(self.is_sqlite()),
        ))
    }

    /// SQLite 的自动生成键沿用同一连接中的 last_insert_rowid 执行路径。
    /// 按列选择仍需真实投影实现，不能将其静默转换为自动生成键。
    async fn prepare_statement_with_generated_keys(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
        generated_keys: druid_rdbc::statement::GeneratedKeys,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        match generated_keys {
            druid_rdbc::statement::GeneratedKeys::None => {
                self.prepare_statement(sql, options).await
            }
            druid_rdbc::statement::GeneratedKeys::ReturnGeneratedKeys
            | druid_rdbc::statement::GeneratedKeys::ColumnIndexes(_)
            | druid_rdbc::statement::GeneratedKeys::ColumnNames(_)
                if self.is_sqlite() =>
            {
                // 对应 BASE execute_prepared：SQLite prepared 的列请求返回单 rowid。
                // 普通 Statement 的列选择仍明确不支持，不混淆两个重载合同。
                self.prepare_statement(sql, options).await
            }
            _ => Err(RdbcError::UnsupportedOperation {
                operation: "connection_prepare_statement_generated_keys",
            }),
        }
    }

    async fn database_meta_data(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, RdbcError> {
        if let Some(executor) = &self.standard_executor {
            executor.task4_trace_database_metadata();
        } else if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_database_metadata();
        }
        let snapshot = super::super::ToastyDatabaseMetaData::owned(
            self.url.clone(),
            self.driver_name.to_owned(),
            self.read_only,
        );
        Ok(Box::new(snapshot))
    }

    fn auto_commit(&self) -> Result<bool, RdbcError> {
        Ok(self.auto_commit)
    }

    fn read_only(&self) -> Result<bool, RdbcError> {
        Ok(self.read_only)
    }

    fn transaction_isolation(&self) -> Result<TransactionIsolation, RdbcError> {
        Ok(TransactionIsolation::from_jdbc(match self.isolation {
            Some(IsolationLevel::ReadUncommitted) => 1,
            Some(IsolationLevel::RepeatableRead) => 4,
            Some(IsolationLevel::Serializable) => 8,
            None if self.driver_name.eq_ignore_ascii_case("sqlite") => 8,
            Some(IsolationLevel::ReadCommitted) | None => 2,
        }))
    }

    fn capabilities(&self) -> ConnectionCapabilities {
        ConnectionCapabilities {
            transactions: true,
            savepoints: true,
            catalogs: false,
            schemas: false,
            callable_statements: false,
        }
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        if let Some(executor) = &self.standard_executor {
            executor.execute_query("SELECT 1", Vec::new()).await?;
            Ok(())
        } else {
            self.connection_mut()?
                .ping()
                .await
                .map_err(|error| Self::driver_error(&error))
        }
    }

    async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        if self.is_closed()? {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(None)
        }
    }

    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        if self.is_closed()? {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok(self.driver_name.to_owned())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if let Some(executor) = &self.standard_executor {
            executor.close().await
        } else {
            self.close_connection_core().await
        }
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.standard_executor.as_ref().map_or_else(
            || self.connection.is_none(),
            |executor| executor.is_closed(),
        ))
    }
}
