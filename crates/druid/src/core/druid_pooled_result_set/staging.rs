use super::*;

impl StagedResultSet {
    pub(crate) fn new(statement: Arc<DruidPooledStatementInner>, raw: Box<dyn ResultSet>) -> Self {
        let statement_handle = DruidPooledStatement::from_inner(Arc::clone(&statement));
        let id = statement.result_set_id_seed.fetch_add(1, Ordering::AcqRel);
        let filter_context = Arc::new(
            ResultSetFilterContext::with_identity_sql_and_execute_elapsed(
                statement.connection_id,
                statement.id,
                id,
                statement_handle.last_sql(),
                statement_handle.last_execute_elapsed(),
            ),
        );
        let filter_chain = statement.filter_chain.clone();
        let resource = StandardResultSetResource::new(
            raw,
            filter_chain.clone(),
            Arc::clone(&filter_context),
            Arc::clone(&statement.policy),
        );
        statement.track_result_set(&resource);
        Self {
            id,
            logical_owner: statement_handle.logical_owner(),
            statement: statement_handle,
            resource,
            filter_chain,
            filter_context,
        }
    }

    pub(crate) fn into_opened(self) -> Result<DruidPooledResultSet, StagedOpenError> {
        if let Some(error) = self.resource.registry_cleanup_error() {
            return Err(StagedOpenError {
                error,
                cleanup_required: false,
            });
        }
        // 无 Filter 时不预读元数据，保留稀疏驱动在 getMetaData 调用点拒绝的语义。
        let (driver_column_map, logic_column_map, hidden_columns) = if let Some(filter_chain) =
            &self.filter_chain
        {
            let meta_data =
                self.resource
                    .open_meta_data_core()
                    .map_err(|error| StagedOpenError {
                        error,
                        cleanup_required: true,
                    })?;
            let mut context =
                super::super::StandardResultSetOpenContext::new(&self.filter_context, meta_data);
            self.resource
                .policy
                .classify(
                    filter_chain
                        .standard_result_set_open_after_with_proxy(&mut context)
                        .map_err(RdbcError::from),
                    self.filter_context.sql(),
                )
                .map_err(|error| StagedOpenError {
                    error,
                    cleanup_required: true,
                })?;
            context.into_column_mappings()
        } else {
            (None, None, None)
        };
        if let Some(error) = self.resource.registry_cleanup_error() {
            return Err(StagedOpenError {
                error,
                cleanup_required: false,
            });
        }
        let shared = Arc::new(shared_state::SharedResultSetState {
            id: self.id,
            attributes: ProxyAttributes::default(),
            closed: AtomicBool::new(false),
            cursor_index: AtomicI32::new(0),
            logic_column_map: parking_lot::RwLock::new(None),
            driver_column_map: parking_lot::RwLock::new(None),
            hidden_columns: parking_lot::RwLock::new(None),
        });
        let result_set = DruidPooledResultSet {
            id: self.id,
            statement: self.statement,
            logical_owner: self.logical_owner,
            has_creating_statement: true,
            resource: self.resource,
            shared,
            filter_context: self.filter_context,
        };
        // 初始 Filter 配置与后续设置共用同一状态入口，发布视图前完成安装。
        result_set.set_logic_column_map(driver_column_map);
        result_set.set_physical_column_map(logic_column_map);
        result_set.set_hidden_columns(hidden_columns);
        // 资源只保存不含 statement/owner 的游标视图，避免 registry 强引用环。
        let _ = result_set.resource.view.set(Arc::clone(&result_set.shared));
        result_set.resource.opened.store(true, Ordering::Release);
        Ok(result_set)
    }
}

#[async_trait::async_trait]
impl Statement for MetadataStatement {
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}
