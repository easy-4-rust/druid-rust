use super::*;

impl PoolInner {
    /// 创建新连接。
    pub async fn create_connection(&self) -> Result<DruidConnectionHolder, DruidError> {
        self.create_connection_to_limit(self.config.max_open).await
    }

    pub(crate) async fn create_connection_to_limit(
        &self,
        capacity_limit: usize,
    ) -> Result<DruidConnectionHolder, DruidError> {
        let result = self
            .create_connection_to_limit_internal(capacity_limit)
            .await;
        match &result {
            Ok(_) => self.record_create_success(),
            Err(DruidError::PoolExhausted | DruidError::DataSourceClosed { .. }) => {}
            Err(error) => self.record_create_error(error),
        }
        result
    }

    pub(crate) async fn create_connection_to_limit_internal(
        &self,
        capacity_limit: usize,
    ) -> Result<DruidConnectionHolder, DruidError> {
        self.ensure_not_closed()?;
        let reserved = self
            .total_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < capacity_limit).then_some(current + 1)
            })
            .is_ok();
        if !reserved {
            return Err(DruidError::PoolExhausted);
        }
        let mut reservation = ConnectionSlotReservation::new(&self.total_count);
        let _creating = CreatingTaskRegistration::new(&self.creating_count, &self.creating_idle);
        // 必须在调用 factory 之前冻结版本；若凭据在连接建立期间更新，使用旧
        // 凭据创建出的 holder 会保留旧版本并在借出/归还门禁被淘汰。
        let user_password_version = self.user_password_version();

        let connect_started_at = Instant::now();
        self.create_failure_state.lock().create_started_at = Some(connect_started_at);
        let filter_chain = self.filter_chain.read().clone();
        let mut connection_properties = (*self.config.connection_properties).clone();
        let create_result = match &filter_chain {
            Some(filter_chain) => {
                let mut next_connection_id = || self.next_id();
                filter_chain
                    .connection_open(
                        self.factory.as_ref(),
                        &mut connection_properties,
                        self.config.login_timeout,
                        &mut next_connection_id,
                    )
                    .await
            }
            None if self.config.login_timeout > 0 => {
                match tokio::time::timeout(
                    Duration::from_secs(self.config.login_timeout as u64),
                    self.factory.create_with_properties(&connection_properties),
                )
                .await
                {
                    Ok(result) => result
                        .map(|connection| {
                            ConnectionOpenResult::new(
                                ConnectionInfo::connected(connection, connect_started_at),
                                self.next_id(),
                            )
                        })
                        .map_err(DruidError::from),
                    Err(_) => Err(DruidError::LoginTimeout),
                }
            }
            None => self
                .factory
                .create_with_properties(&connection_properties)
                .await
                .map(|connection| {
                    ConnectionOpenResult::new(
                        ConnectionInfo::connected(connection, connect_started_at),
                        self.next_id(),
                    )
                })
                .map_err(DruidError::from),
        };

        match create_result {
            Ok(connect_result) => {
                let (mut connection_info, connection_id) = connect_result.into_parts();
                // Java 在原始驱动连接成功后立即增加 createCount，默认属性初始化失败
                // 也不回退该计数。
                self.create_count.fetch_add(1, Ordering::Relaxed);
                if let Err(error) = self.ensure_not_closed() {
                    self.close_connected_connection(
                        filter_chain.as_ref(),
                        &mut connection_info,
                        connection_id,
                        connect_started_at.elapsed(),
                    )
                    .await;
                    self.destroy_count.fetch_add(1, Ordering::Relaxed);
                    return Err(error);
                }

                let initialize_result = match connection_info.connection_mut() {
                    Some(connection) => self.initialize_connection(connection.as_mut()).await,
                    None => Err(DruidError::ConnectionDiscarded),
                };
                if let Err(error) = initialize_result {
                    // 对应 Java createConnection() 的异常路径：初始化失败时
                    // 关闭刚创建的物理连接，但它从未进入池，不增加 destroyCount。
                    self.close_connected_connection(
                        filter_chain.as_ref(),
                        &mut connection_info,
                        connection_id,
                        connect_started_at.elapsed(),
                    )
                    .await;
                    self.driver_connect_error_count
                        .fetch_add(1, Ordering::Relaxed);
                    return Err(error);
                }
                connection_info.mark_initialized();

                let init_sql_checked = match self
                    .initialize_sqls_and_variables(&mut connection_info)
                    .await
                {
                    Ok(checked) => checked,
                    Err(error) => {
                        self.close_connected_connection(
                            filter_chain.as_ref(),
                            &mut connection_info,
                            connection_id,
                            connect_started_at.elapsed(),
                        )
                        .await;
                        self.driver_connect_error_count
                            .fetch_add(1, Ordering::Relaxed);
                        return Err(error);
                    }
                };

                let validate_result = if init_sql_checked {
                    Ok(())
                } else {
                    match connection_info.connection_mut() {
                        Some(connection) => self.validate_connection(connection).await,
                        None => Err(DruidError::ConnectionDiscarded),
                    }
                };
                if let Err(error) = validate_result {
                    self.close_connected_connection(
                        filter_chain.as_ref(),
                        &mut connection_info,
                        connection_id,
                        connect_started_at.elapsed(),
                    )
                    .await;
                    self.driver_connect_error_count
                        .fetch_add(1, Ordering::Relaxed);
                    return Err(error);
                }
                connection_info.mark_validated();

                let mut holder = DruidConnectionHolder::with_connection_info(
                    connection_info,
                    connection_id,
                    user_password_version,
                )
                .await?;
                holder.set_connection_properties(Arc::new(connection_properties));
                holder.configure_statement_pool(
                    self.config.pool_prepared_statements,
                    self.config.max_pool_prepared_statements_per_connection,
                    self.config.share_prepared_statements,
                    self.config.use_oracle_implicit_cache,
                    self.prepared_statement_stats.clone(),
                );
                let restore_schema = self.config.db_type_name.as_deref().is_some_and(|db_type| {
                    [
                        "mysql",
                        "oceanbase",
                        "ads",
                        "drds",
                        "mariadb",
                        "tidb",
                        "h2",
                        "lealone",
                        "goldendb",
                        "polardbx",
                    ]
                    .iter()
                    .any(|candidate| db_type.eq_ignore_ascii_case(candidate))
                });
                holder.set_restore_schema_on_recycle(restore_schema);
                reservation.commit();
                Ok(holder)
            }
            Err(e) => {
                self.driver_connect_error_count
                    .fetch_add(1, Ordering::Relaxed);
                Err(e)
            }
        }
    }

    /// 关闭已经由 physical-connect 链创建、但尚未成功进入 holder 的连接。
    ///
    /// Java 初始化或校验失败时调用 `ConnectionProxy#close()`，因此必须重新从
    /// position 0 进入 physical-close 链，让 StatFilter/LogFilter 观察真实
    /// 关闭；该路径不增加池级 destroyCount。
    pub(crate) async fn close_connected_connection(
        &self,
        filter_chain: Option<&Arc<FilterChain>>,
        connection_info: &mut ConnectionInfo,
        connection_id: u64,
        connection_age: Duration,
    ) {
        let Some(connection) = connection_info.connection_mut() else {
            return;
        };
        let result = match filter_chain {
            Some(filter_chain) if !filter_chain.is_empty() => {
                filter_chain
                    .connection_close(
                        self.factory.as_ref(),
                        connection,
                        connection_id,
                        connection_age,
                    )
                    .await
            }
            _ => self
                .factory
                .close(connection)
                .await
                .map_err(DruidError::from),
        };
        if let Err(error) = result {
            tracing::warn!(
                %error,
                connection_id,
                "close failed physical connection failed"
            );
        }
    }

    /// 按 Java `DruidAbstractDataSource#initConnection` 顺序初始化连接。
    pub(crate) async fn initialize_connection(
        &self,
        connection: &mut dyn Connection,
    ) -> Result<(), DruidError> {
        let skip_auto_commit = self.config.db_type_name.as_deref() == Some("odps");
        if !skip_auto_commit
            && connection
                .auto_commit()
                .is_ok_and(|value| value != self.config.default_auto_commit)
        {
            connection
                .set_auto_commit(self.config.default_auto_commit)
                .await?;
        }

        if let Some(default_read_only) = self.config.default_read_only {
            if connection
                .read_only()
                .is_ok_and(|value| value != default_read_only)
            {
                connection.set_read_only(default_read_only).await?;
            }
        }

        // 对应 Java initConnection：隔离级别位于 readOnly 与 catalog 之间。
        // 原始 JDBC 数字通过标准类型保留，不能在 SPI 合并时遗漏配置应用。
        if let Some(default_transaction_isolation) = self.config.default_transaction_isolation {
            let isolation = druid_rdbc::connection::TransactionIsolation::from_jdbc(i32::from(
                default_transaction_isolation,
            ));
            if connection
                .transaction_isolation()
                .is_ok_and(|current| current != isolation)
            {
                connection.set_transaction_isolation(isolation).await?;
            }
        }

        if let Some(default_catalog) = self.config.default_catalog.as_deref() {
            // 显式配置 catalog 必须调用 setter；驱动可能支持 setter 而不支持 getter。
            // 保留 Java 非空 catalog 的初始化合同及驱动 setter 的原始错误。
            if !default_catalog.is_empty() {
                connection.set_catalog(Some(default_catalog)).await?;
            }
        }

        Ok(())
    }

    /// 在 raw connection 上执行初始化 SQL并按 Java 规则采集 `MySQL` 变量。
    ///
    /// 返回值对应 Java `initSqls(...)` 的 `checked`：只要执行过初始化 SQL或
    /// `MySQL` variables 查询，创建流程就不再追加 validation query。
    pub(crate) async fn initialize_sqls_and_variables(
        &self,
        connection_info: &mut ConnectionInfo,
    ) -> Result<bool, DruidError> {
        let mut variables = self.config.init_variants.then(HashMap::new);
        let mut global_variables = self.config.init_global_variants.then(HashMap::new);
        let connection = connection_info
            .connection_mut()
            .ok_or(DruidError::ConnectionDiscarded)?;
        let mut checked = false;

        for sql in &self.config.connection_init_sqls {
            Self::execute_init_sql(connection.as_mut(), sql).await?;
            checked = true;
        }

        let is_mysql_family = self
            .config
            .db_type_name
            .as_deref()
            .and_then(DbType::of)
            .is_some_and(RdbcUtils::is_mysql_db_type);
        if is_mysql_family {
            if let Some(values) = variables.as_mut() {
                let rows = Self::fetch_rows(connection.as_mut(), "show variables").await?;
                Self::collect_variable_rows(values, rows);
                checked = true;
            }
            if let Some(values) = global_variables.as_mut() {
                let rows = Self::fetch_rows(connection.as_mut(), "show global variables").await?;
                Self::collect_variable_rows(values, rows);
                checked = true;
            }
        }

        connection_info.set_variables(variables);
        connection_info.set_global_variables(global_variables);
        Ok(checked)
    }

    pub(crate) async fn execute_init_sql(
        connection: &mut dyn Connection,
        sql: &str,
    ) -> Result<(), DruidError> {
        let mut statement = connection
            .create_statement(druid_rdbc::statement::StatementOptions::default())
            .await?;
        statement
            .execute(sql, druid_rdbc::statement::GeneratedKeys::None)
            .await?;
        statement.close().await?;
        Ok(())
    }

    pub(crate) async fn fetch_rows(
        connection: &mut dyn Connection,
        sql: &str,
    ) -> Result<Vec<crate::core::Row>, DruidError> {
        let mut statement = connection
            .create_statement(druid_rdbc::statement::StatementOptions::default())
            .await?;
        let mut result_set = statement.execute_query(sql).await?;
        let width = result_set.meta_data()?.column_count();
        let mut rows = Vec::new();
        while result_set.next().await? {
            let mut row = Vec::with_capacity(width);
            for column in 1..=width {
                let index = druid_rdbc::result_set::ColumnIndex::new(column)?;
                row.push(result_set.value(index)?);
            }
            rows.push(crate::core::Row::new(row));
        }
        result_set.close().await?;
        statement.close().await?;
        Ok(rows)
    }

    pub(crate) fn collect_variable_rows(
        target: &mut HashMap<String, JsonValue>,
        rows: Vec<crate::core::Row>,
    ) {
        for row in rows {
            let Some(name) = row.get(0).and_then(Self::variable_name) else {
                continue;
            };
            let value = row.get(1).map_or(JsonValue::Null, Self::rdbc_value_to_json);
            target.insert(name, value);
        }
    }

    pub(crate) fn variable_name(value: &RdbcValue) -> Option<String> {
        match value {
            RdbcValue::Null => None,
            RdbcValue::String(value) => Some(value.clone()),
            RdbcValue::Bytes(value) => String::from_utf8(value.clone()).ok(),
            RdbcValue::Bool(value) => Some(value.to_string()),
            RdbcValue::Int(value) => Some(value.to_string()),
            RdbcValue::Float(value) => Some(value.to_string()),
            RdbcValue::Decimal(value) => Some(value.to_string()),
            RdbcValue::Date(value) => Some(value.to_string()),
            RdbcValue::Time(value) => Some(value.to_string()),
            RdbcValue::Timestamp(value) => Some(value.to_string()),
        }
    }

    pub(crate) fn rdbc_value_to_json(value: &RdbcValue) -> JsonValue {
        match value {
            RdbcValue::Null => JsonValue::Null,
            RdbcValue::Bool(value) => JsonValue::Bool(*value),
            RdbcValue::Int(value) => JsonValue::Number((*value).into()),
            RdbcValue::Float(value) => JsonNumber::from_f64(*value)
                .map_or_else(|| JsonValue::String(value.to_string()), JsonValue::Number),
            RdbcValue::Decimal(value) => JsonValue::String(value.to_string()),
            RdbcValue::Date(value) => JsonValue::String(value.to_string()),
            RdbcValue::Time(value) => JsonValue::String(value.to_string()),
            RdbcValue::Timestamp(value) => JsonValue::String(value.to_string()),
            RdbcValue::String(value) => JsonValue::String(value.clone()),
            RdbcValue::Bytes(value) => JsonValue::Array(
                value
                    .iter()
                    .map(|byte| JsonValue::Number((*byte).into()))
                    .collect(),
            ),
        }
    }
}
