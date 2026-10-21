use super::*;

#[async_trait::async_trait]
impl druid_rdbc::connection::Connection for DruidPooledConnection {
    async fn exec(
        &mut self,
        sql: &str,
        params: Vec<druid_rdbc::types::Value>,
    ) -> Result<druid_rdbc::statement::ExecResult, RdbcError> {
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        let driver_params = params.clone();
        let connection = self.standard_mut()?;
        let result =
            policy
                .execute(
                    sql,
                    params,
                    0,
                    super::super::ExecOperation::Update,
                    move |effective_sql| async move {
                        connection.exec(&effective_sql, driver_params).await
                    },
                    |result| *result,
                )
                .await;
        self.finish_standard_policy_result(policy.as_ref(), result)
    }

    async fn fetch(
        &mut self,
        sql: &str,
        params: Vec<druid_rdbc::types::Value>,
    ) -> Result<Vec<druid_rdbc::types::Row>, RdbcError> {
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        let driver_params = params.clone();
        let connection = self.standard_mut()?;
        let result = policy
            .execute(
                sql,
                params,
                0,
                super::super::ExecOperation::Query,
                move |effective_sql| async move {
                    connection.fetch(&effective_sql, driver_params).await
                },
                |rows| druid_rdbc::statement::ExecResult {
                    row_count: Some(u64::try_from(rows.len()).unwrap_or(u64::MAX)),
                    ..druid_rdbc::statement::ExecResult::default()
                },
            )
            .await;
        self.finish_standard_policy_result(policy.as_ref(), result)
    }

    async fn exec_batch(
        &mut self,
        batch: Vec<(String, Vec<druid_rdbc::types::Value>)>,
    ) -> Result<Vec<i64>, RdbcError> {
        let result = self.standard_mut()?.exec_batch(batch).await;
        self.classify_result(result)
    }

    async fn create_statement(
        &mut self,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::Statement>, RdbcError> {
        let resources = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        let admission = resources.begin_admission(policy.resource_generation())?;
        let result = self.standard_mut()?.create_statement(options).await;
        let statement = self.classify_result(result)?;
        let statement_id = self.next_statement_id();
        let statement = DruidPooledStatement::new_standard(
            statement,
            options,
            Arc::clone(&resources),
            policy,
            self.id,
            statement_id,
            Arc::clone(&self.result_set_id_seed),
            Arc::clone(&self.metadata_id_seed),
            Arc::clone(&self.lease_active),
            self.filter_chain.clone(),
            self.stats_collector.clone(),
        );
        resources
            .register(&admission, Arc::clone(&statement.inner))
            .await?;
        let query_timeout = self.effective_query_timeout();
        if query_timeout > 0 {
            let result = statement.configure_standard_query_timeout(query_timeout);
            self.classify_result(result)?;
        }
        let event_result = DruidExecutionFilterPolicy::complete_statement_creation(
            self.filter_chain.as_deref(),
            self.stats_collector.as_ref(),
            self.id,
            statement_id,
            &super::super::StatementEvent::CreateStatement,
        )
        .await;
        self.classify_result(event_result)?;
        Ok(Box::new(statement))
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        self.prepare_statement_with_generated_keys(
            sql,
            options,
            druid_rdbc::statement::GeneratedKeys::None,
        )
        .await
    }

    async fn prepare_statement_with_generated_keys(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
        generated_keys: druid_rdbc::statement::GeneratedKeys,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        super::standard_connection_helpers::prepare_statement_with_generated_keys(
            self,
            sql,
            options,
            generated_keys,
        )
        .await
    }

    async fn prepare_call(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::CallableStatement>, RdbcError> {
        let effective_sql =
            DruidExecutionFilterPolicy::prepare_statement_sql(self.filter_chain.as_deref(), sql)?;
        let method_type = if options == druid_rdbc::statement::StatementOptions::default() {
            PreparedStatementMethodType::Precall1
        } else {
            PreparedStatementMethodType::Precall2
        };
        let key = PreparedStatementKey::standard(
            &effective_sql,
            self.current_catalog(),
            method_type,
            options,
            druid_rdbc::statement::GeneratedKeys::None,
        )?;
        let holder = self.holder.as_ref().ok_or(RdbcError::ConnectionDiscarded)?;
        let cache = holder.standard_statement_pool();
        let cache_stats = Arc::clone(holder.prepared_statement_stats());
        let resources = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        let admission = resources.begin_admission(policy.resource_generation())?;
        let (cached, cache_enabled) = {
            let mut cache = cache.lock().await;
            (cache.take_callable(&key), cache.is_enabled())
        };
        let cache_hit = cached.is_some();
        let statement = if let Some(statement) = cached {
            cache_stats.record_cache_checkout();
            cache_stats.record_hit();
            statement
        } else {
            let result = self
                .standard_mut()?
                .prepare_call(&effective_sql, options)
                .await;
            let statement = self.classify_result_with_sql(result, Some(&effective_sql))?;
            cache_stats.record_prepare();
            if cache_enabled {
                cache_stats.record_miss();
            }
            statement
        };
        let statement_id = self.next_statement_id();
        let statement = DruidPooledCallableStatement::new_standard(
            statement,
            &effective_sql,
            options,
            key,
            cache,
            cache_stats,
            cache_hit,
            resources,
            &admission,
            policy,
            self.id,
            statement_id,
            Arc::clone(&self.result_set_id_seed),
            Arc::clone(&self.metadata_id_seed),
            Arc::clone(&self.lease_active),
            self.filter_chain.clone(),
            self.stats_collector.clone(),
        )
        .await?;
        let query_timeout = self.effective_query_timeout();
        if query_timeout > 0 {
            let result = statement.configure_standard_query_timeout(query_timeout);
            self.classify_result_with_sql(result, Some(&effective_sql))?;
        }
        let event_result = DruidExecutionFilterPolicy::complete_statement_creation(
            self.filter_chain.as_deref(),
            self.stats_collector.as_ref(),
            self.id,
            statement_id,
            &super::super::StatementEvent::PrepareCall(effective_sql),
        )
        .await;
        self.classify_result(event_result)?;
        Ok(Box::new(statement))
    }

    async fn database_meta_data(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, RdbcError> {
        standard_connection_helpers::database_meta_data(self).await
    }

    /// 创建受当前租约监管的标准 Blob，返回独占资源或驱动错误。
    /// 对应 Java: DruidPooledConnection#createBlob。
    async fn create_blob(&mut self) -> Result<Box<dyn druid_rdbc::resource::Blob>, RdbcError> {
        self.standard_ref()?;
        let registry = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        policy.ensure_open()?;
        let admission = Arc::new(registry.begin_admission(policy.resource_generation())?);
        let filter_chain = self.filter_chain.clone();
        if let Some(chain) = filter_chain.filter(|chain| chain.before_count() > 0) {
            let result = chain
                .connection_create_blob(
                    self.standard_mut()?,
                    Arc::clone(&registry),
                    Arc::clone(&admission),
                    Arc::clone(&policy),
                )
                .await
                .map(super::super::registered_lob::RegisteredLob::into_inner)
                .map_err(RdbcError::from);
            return self.classify_result(result);
        }
        let raw_result = self.standard_mut()?.create_blob().await;
        let raw = self.classify_result(raw_result)?;
        // 驱动返回后直到 registry 接管前不再 await，取消不会丢失已创建资源。
        let id = super::super::resource_id::ResourceId::local();
        let owner = Arc::new(super::super::druid_resource_owner::DruidResourceOwner::new(
            id.clone(),
            super::super::resource_kind::ResourceKind::Blob,
            policy,
        ));
        let context = Arc::new(super::super::resource_context::ResourceContext::new(
            id,
            super::super::resource_kind::ResourceKind::Blob,
            super::super::resource_runtime_capabilities::ResourceRuntimeCapabilities::large_object(
            ),
            owner,
        ));
        let resource =
            super::super::druid_blob::DruidBlob::register(raw, context, registry, &admission)
                .await?;
        Ok(Box::new(resource))
    }

    /// 创建受当前租约监管的标准 Clob，返回独占资源或驱动错误。
    /// 对应 Java: DruidPooledConnection#createClob。
    async fn create_clob(&mut self) -> Result<Box<dyn druid_rdbc::resource::Clob>, RdbcError> {
        self.standard_ref()?;
        let registry = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        policy.ensure_open()?;
        let admission = Arc::new(registry.begin_admission(policy.resource_generation())?);
        let filter_chain = self.filter_chain.clone();
        if let Some(chain) = filter_chain.filter(|chain| chain.before_count() > 0) {
            let result = chain
                .connection_create_clob(
                    self.standard_mut()?,
                    Arc::clone(&registry),
                    Arc::clone(&admission),
                    Arc::clone(&policy),
                )
                .await
                .map(super::super::registered_lob::RegisteredLob::into_inner)
                .map_err(RdbcError::from);
            return self.classify_result(result);
        }
        let raw_result = self.standard_mut()?.create_clob().await;
        let raw = self.classify_result(raw_result)?;
        // 驱动返回后直到 registry 接管前不再 await，取消不会丢失已创建资源。
        let id = super::super::resource_id::ResourceId::local();
        let owner = Arc::new(super::super::druid_resource_owner::DruidResourceOwner::new(
            id.clone(),
            super::super::resource_kind::ResourceKind::Clob,
            policy,
        ));
        let context = Arc::new(super::super::resource_context::ResourceContext::new(
            id,
            super::super::resource_kind::ResourceKind::Clob,
            super::super::resource_runtime_capabilities::ResourceRuntimeCapabilities::large_object(
            ),
            owner,
        ));
        let resource =
            super::super::druid_clob::DruidClob::register(raw, context, registry, &admission)
                .await?;
        Ok(Box::new(resource))
    }

    /// 创建受当前租约监管的标准 NClob，返回独占资源或驱动错误。
    /// 对应 Java: DruidPooledConnection#createNClob。
    async fn create_n_clob(&mut self) -> Result<Box<dyn druid_rdbc::resource::NClob>, RdbcError> {
        self.standard_ref()?;
        let registry = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        policy.ensure_open()?;
        let admission = Arc::new(registry.begin_admission(policy.resource_generation())?);
        let filter_chain = self.filter_chain.clone();
        if let Some(chain) = filter_chain.filter(|chain| chain.before_count() > 0) {
            let result = chain
                .connection_create_n_clob(
                    self.standard_mut()?,
                    Arc::clone(&registry),
                    Arc::clone(&admission),
                    Arc::clone(&policy),
                )
                .await
                .map(super::super::registered_lob::RegisteredLob::into_inner)
                .map_err(RdbcError::from);
            return self.classify_result(result);
        }
        let raw_result = self.standard_mut()?.create_n_clob().await;
        let raw = self.classify_result(raw_result)?;
        // 驱动返回后直到 registry 接管前不再 await，取消不会丢失已创建资源。
        let id = super::super::resource_id::ResourceId::local();
        let owner = Arc::new(super::super::druid_resource_owner::DruidResourceOwner::new(
            id.clone(),
            super::super::resource_kind::ResourceKind::NClob,
            policy,
        ));
        let context = Arc::new(super::super::resource_context::ResourceContext::new(
            id,
            super::super::resource_kind::ResourceKind::NClob,
            super::super::resource_runtime_capabilities::ResourceRuntimeCapabilities::large_object(
            ),
            owner,
        ));
        let resource =
            super::super::druid_n_clob::DruidNClob::register(raw, context, registry, &admission)
                .await?;
        Ok(Box::new(resource))
    }

    /// 创建受当前租约监管的标准 SqlXml，返回独占资源或驱动错误。
    /// 对应 Java: DruidPooledConnection#createSqlXml。
    async fn create_sql_xml(&mut self) -> Result<Box<dyn druid_rdbc::resource::SqlXml>, RdbcError> {
        self.standard_ref()?;
        let registry = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        policy.ensure_open()?;
        let admission = registry.begin_admission(policy.resource_generation())?;
        let raw_result = self.standard_mut()?.create_sql_xml().await;
        let raw = self.classify_result(raw_result)?;
        // 驱动返回后直到 registry 接管前不再 await，取消不会丢失已创建资源。
        let id = super::super::resource_id::ResourceId::local();
        let owner = Arc::new(super::super::druid_resource_owner::DruidResourceOwner::new(
            id.clone(),
            super::super::resource_kind::ResourceKind::SqlXml,
            policy,
        ));
        let context = Arc::new(super::super::resource_context::ResourceContext::new(
            id,
            super::super::resource_kind::ResourceKind::SqlXml,
            super::super::resource_runtime_capabilities::ResourceRuntimeCapabilities::READ
                | super::super::resource_runtime_capabilities::ResourceRuntimeCapabilities::WRITE
                | super::super::resource_runtime_capabilities::ResourceRuntimeCapabilities::STREAM
                | super::super::resource_runtime_capabilities::ResourceRuntimeCapabilities::FREE,
            owner,
        ));
        let resource =
            super::super::druid_sql_xml::DruidSqlXml::register(raw, context, registry, &admission)
                .await?;
        Ok(Box::new(resource))
    }

    /// 创建独立拥有的SQL结构对象，保留nullable名称与有序属性；返回原驱动对象或分类错误。
    /// 对应 Java: DruidPooledConnection#createStruct。Struct无free且默认随应用引用有效，不能加入LOB释放状态机。
    async fn create_struct(
        &mut self,
        type_name: Option<&str>,
        attributes: Option<Vec<druid_rdbc::types::RdbcObject>>,
    ) -> Result<Box<dyn druid_rdbc::resource::Struct>, RdbcError> {
        self.standard_ref()?;
        let result = self
            .standard_mut()?
            .create_struct(type_name, attributes)
            .await;
        self.classify_result(result)
    }

    /// 创建当前租约监管的 SQL 数组，保留可空类型名和有序元素；返回资源或驱动错误。
    /// 对应 Java: DruidPooledConnection#createArrayOf。
    async fn create_array_of(
        &mut self,
        type_name: Option<&str>,
        elements: Option<Vec<druid_rdbc::types::RdbcObject>>,
    ) -> Result<Box<dyn druid_rdbc::resource::Array>, RdbcError> {
        self.standard_ref()?;
        let registry = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        policy.ensure_open()?;
        let admission = registry.begin_admission(policy.resource_generation())?;
        let raw_result = self
            .standard_mut()?
            .create_array_of(type_name, elements)
            .await;
        let raw = self.classify_result(raw_result)?;
        let id = super::super::resource_id::ResourceId::local();
        let owner = Arc::new(super::super::druid_resource_owner::DruidResourceOwner::new(
            id.clone(),
            super::super::resource_kind::ResourceKind::Array,
            policy.clone(),
        ));
        let context = Arc::new(super::super::resource_context::ResourceContext::new(
            id,
            super::super::resource_kind::ResourceKind::Array,
            super::super::resource_runtime_capabilities::ResourceRuntimeCapabilities::READ
                | super::super::resource_runtime_capabilities::ResourceRuntimeCapabilities::RANGE
                | super::super::resource_runtime_capabilities::ResourceRuntimeCapabilities::TYPE_MAP
                | super::super::resource_runtime_capabilities::ResourceRuntimeCapabilities::RESULT_SET
                | super::super::resource_runtime_capabilities::ResourceRuntimeCapabilities::FREE,
            owner,
        ));
        let resource = super::super::druid_array::DruidArray::register(
            raw,
            context,
            registry,
            &admission,
            self.id,
            Arc::clone(&self.result_set_id_seed),
            Arc::clone(&self.lease_active),
            self.filter_chain.clone(),
            self.stats_collector.clone(),
            policy,
        )
        .await?;
        Ok(Box::new(resource))
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        // Java checkState 在统计提交尝试之前执行；致命拒绝不产生新的提交计数。
        self.standard_ref()?;
        let event = ConnectionEvent::Commit;
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        if let Some(policy) = &self.standard_policy {
            policy.record_transaction_boundary(true);
        }
        let result = self.standard_mut()?.commit().await;
        self.classify_result(result)?;
        self.after_connection_event(&event, started_at).await
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        let event = ConnectionEvent::Rollback;
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        if let Some(policy) = &self.standard_policy {
            policy.record_transaction_boundary(false);
        }
        let result = self.standard_mut()?.rollback().await;
        self.classify_result(result)?;
        self.after_connection_event(&event, started_at).await
    }

    async fn rollback_to(
        &mut self,
        savepoint: &druid_rdbc::types::Savepoint,
    ) -> Result<(), RdbcError> {
        let result = self.standard_mut()?.rollback_to(savepoint).await;
        self.classify_result(result)
    }

    async fn set_savepoint(&mut self) -> Result<druid_rdbc::types::Savepoint, RdbcError> {
        let result = self.standard_mut()?.set_savepoint().await;
        self.classify_result(result)
    }

    async fn set_savepoint_named(
        &mut self,
        name: &str,
    ) -> Result<druid_rdbc::types::Savepoint, RdbcError> {
        let result = self.standard_mut()?.set_savepoint_named(name).await;
        self.classify_result(result)
    }

    async fn release_savepoint(
        &mut self,
        savepoint: &druid_rdbc::types::Savepoint,
    ) -> Result<(), RdbcError> {
        let result = self.standard_mut()?.release_savepoint(savepoint).await;
        self.classify_result(result)
    }

    async fn set_auto_commit(&mut self, enabled: bool) -> Result<(), RdbcError> {
        let event = ConnectionEvent::SetAutoCommit(enabled);
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        let result = self.standard_mut()?.set_auto_commit(enabled).await;
        self.classify_result(result)?;
        if let Some(policy) = &self.standard_policy {
            policy.set_auto_commit_state(enabled);
        }
        self.after_connection_event(&event, started_at).await
    }

    fn auto_commit(&self) -> Result<bool, RdbcError> {
        self.standard_ref()?.auto_commit()
    }

    fn read_only(&self) -> Result<bool, RdbcError> {
        self.standard_ref()?.read_only()
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), RdbcError> {
        let event = ConnectionEvent::SetReadOnly(read_only);
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        self.invalidate_prepared_statement_cache().await?;
        let result = self.standard_mut()?.set_read_only(read_only).await;
        self.classify_result(result)?;
        self.after_connection_event(&event, started_at).await
    }

    fn transaction_isolation(
        &self,
    ) -> Result<druid_rdbc::connection::TransactionIsolation, RdbcError> {
        self.standard_ref()?.transaction_isolation()
    }

    async fn set_transaction_isolation(
        &mut self,
        level: druid_rdbc::connection::TransactionIsolation,
    ) -> Result<(), RdbcError> {
        self.invalidate_prepared_statement_cache().await?;
        let result = self.standard_mut()?.set_transaction_isolation(level).await;
        self.classify_result(result)
    }

    fn holdability(&self) -> Result<druid_rdbc::statement::ResultSetHoldability, RdbcError> {
        self.standard_ref()?.holdability()
    }

    async fn set_holdability(
        &mut self,
        holdability: druid_rdbc::statement::ResultSetHoldability,
    ) -> Result<(), RdbcError> {
        self.invalidate_prepared_statement_cache().await?;
        let result = self.standard_mut()?.set_holdability(holdability).await;
        self.classify_result(result)
    }

    async fn set_client_info(&mut self, name: &str, value: Option<&str>) -> Result<(), RdbcError> {
        self.prepare_client_info_mutation().await.map_err(|error| {
            error.with_client_info_failure(HashMap::from([(
                Some(name.to_owned()),
                Some(druid_rdbc::connection::ClientInfoStatus::ReasonUnknown),
            )]))
        })?;
        let result = match self.standard_mut() {
            Ok(connection) => connection.set_client_info(name, value).await,
            Err(error) => Err(error),
        }
        .map_err(|error| {
            error.with_client_info_failure(HashMap::from([(
                Some(name.to_owned()),
                Some(druid_rdbc::connection::ClientInfoStatus::ReasonUnknown),
            )]))
        });
        self.classify_result(result)
    }

    fn client_info(&self, name: &str) -> Result<Option<String>, RdbcError> {
        self.standard_ref()?.client_info(name)
    }

    async fn client_info_properties(&mut self) -> Result<HashMap<String, String>, RdbcError> {
        let result = self.standard_mut()?.client_info_properties().await;
        self.classify_result(result)
    }

    async fn set_client_info_properties(
        &mut self,
        properties: Option<&HashMap<String, String>>,
    ) -> Result<(), RdbcError> {
        self.prepare_client_info_mutation().await.map_err(|error| {
            error.with_client_info_failure(
                properties
                    .into_iter()
                    .flat_map(|values| values.keys())
                    .map(|name| {
                        (
                            Some(name.clone()),
                            Some(druid_rdbc::connection::ClientInfoStatus::ReasonUnknown),
                        )
                    })
                    .collect(),
            )
        })?;
        let pending = || {
            properties
                .into_iter()
                .flat_map(|values| values.keys())
                .map(|name| {
                    (
                        Some(name.clone()),
                        Some(druid_rdbc::connection::ClientInfoStatus::ReasonUnknown),
                    )
                })
                .collect()
        };
        let result = match self.standard_mut() {
            Ok(connection) => connection.set_client_info_properties(properties).await,
            Err(error) => Err(error),
        }
        .map_err(|error| error.with_client_info_failure(pending()));
        self.classify_result(result)
    }

    async fn native_sql(&mut self, sql: &str) -> Result<String, RdbcError> {
        self.standard_mut()?.native_sql(sql).await
    }

    async fn set_network_timeout(&mut self, timeout: std::time::Duration) -> Result<(), RdbcError> {
        let result = self.standard_mut()?.set_network_timeout(timeout).await;
        self.classify_result(result)
    }

    fn network_timeout(&self) -> Result<std::time::Duration, RdbcError> {
        self.standard_ref()?.network_timeout()
    }

    fn type_map(&self) -> Result<Option<druid_rdbc::types::RdbcTypeMap>, RdbcError> {
        self.standard_ref()?.type_map()
    }

    async fn set_type_map(
        &mut self,
        type_map: druid_rdbc::types::RdbcTypeMap,
    ) -> Result<(), RdbcError> {
        self.invalidate_prepared_statement_cache().await?;
        let result = self.standard_mut()?.set_type_map(type_map).await;
        self.classify_result(result)
    }

    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        self.standard_ref()?.catalog()
    }

    async fn set_catalog(&mut self, catalog: Option<&str>) -> Result<(), RdbcError> {
        let event = ConnectionEvent::SetCatalog(catalog.unwrap_or_default().to_string());
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        self.invalidate_prepared_statement_cache().await?;
        let result = self.standard_mut()?.set_catalog(catalog).await;
        self.classify_result(result)?;
        self.after_connection_event(&event, started_at).await
    }

    fn schema(&self) -> Result<Option<String>, RdbcError> {
        self.standard_ref()?.schema()
    }

    async fn set_schema(&mut self, schema: Option<&str>) -> Result<(), RdbcError> {
        let event = ConnectionEvent::SetSchema(schema.unwrap_or_default().to_string());
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        let initial_schema = self.standard_ref()?.schema()?;
        if let Some(holder) = self.holder.as_ref() {
            holder.remember_initial_schema(initial_schema);
        }
        self.invalidate_prepared_statement_cache().await?;
        let result = self.standard_mut()?.set_schema(schema).await;
        self.classify_result(result)?;
        self.after_connection_event(&event, started_at).await
    }

    async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        DruidPooledConnection::warnings(self).await
    }

    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        let event = ConnectionEvent::ClearWarnings;
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        DruidPooledConnection::clear_warnings(self).await?;
        self.after_connection_event(&event, started_at).await
    }

    fn capabilities(&self) -> druid_rdbc::extensions::ConnectionCapabilities {
        self.standard_ref()
            .map(druid_rdbc::connection::Connection::capabilities)
            .unwrap_or_default()
    }

    async fn is_valid(&mut self, timeout_seconds: u32) -> Result<bool, RdbcError> {
        let event = ConnectionEvent::IsValid;
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        let result = self.standard_mut()?.is_valid(timeout_seconds).await;
        let valid = self.classify_result(result)?;
        self.after_connection_event(&event, started_at).await?;
        Ok(valid)
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        let result = self.standard_mut()?.ping().await;
        self.classify_result(result)
    }

    async fn abort(&mut self) -> Result<(), RdbcError> {
        let result = self.standard_mut()?.abort().await;
        self.classify_result(result)
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        self.standard_ref()?.driver_name()
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.close_logical_once().await
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.recycled || !self.lease_active.load(Ordering::Acquire))
    }
}
