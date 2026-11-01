use super::*;

impl FilterChainImpl {
    pub fn new() -> Self {
        Self {
            before: Vec::new(),
            after: Vec::new(),
            result_set: Vec::new(),
            filter_class_names: Vec::new(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.before.is_empty() && self.after.is_empty() && self.result_set.is_empty()
    }

    pub fn add_before(&mut self, filter: Arc<dyn BeforeFilter>) {
        self.before.push(filter);
    }

    pub fn add_after(&mut self, filter: Arc<dyn AfterFilter>) {
        self.after.push(filter);
    }

    pub fn add_result_set(&mut self, filter: Arc<dyn ResultSetFilter>) {
        self.result_set.push(filter);
    }

    pub fn add_filter<T>(&mut self, filter: Arc<T>)
    where
        T: BeforeFilter + AfterFilter + ResultSetFilter + 'static,
    {
        self.filter_class_names
            .push(std::any::type_name::<T>().to_owned());
        self.before
            .push(Arc::clone(&filter) as Arc<dyn BeforeFilter>);
        self.after.push(Arc::clone(&filter) as Arc<dyn AfterFilter>);
        self.result_set.push(filter);
    }

    pub(crate) fn add_registered_filter(
        &mut self,
        class_name: String,
        before: Arc<dyn BeforeFilter>,
        after: Arc<dyn AfterFilter>,
        result_set: Arc<dyn ResultSetFilter>,
    ) {
        self.filter_class_names.push(class_name);
        self.before.push(before);
        self.after.push(after);
        self.result_set.push(result_set);
    }

    /// 判断链中是否已有精确类名，区分大小写。
    /// 对应 Java: FilterManager#existsFilter。
    /// 参数 `class_name` 为注册类名；返回是否已存在同一类身份。
    #[must_use]
    pub fn contains_filter_class_name(&self, class_name: &str) -> bool {
        self.filter_class_names
            .iter()
            .any(|existing| existing == class_name)
    }

    #[must_use]
    pub fn filter_class_names(&self) -> &[String] {
        &self.filter_class_names
    }

    #[must_use]
    pub fn before_count(&self) -> usize {
        self.before.len()
    }

    #[must_use]
    pub fn after_count(&self) -> usize {
        self.after.len()
    }

    #[must_use]
    pub fn result_set_count(&self) -> usize {
        self.result_set.len()
    }

    pub(crate) async fn init_filters(&self) -> Result<(), DruidError> {
        for filter in &self.before {
            filter.init().await?;
        }
        Ok(())
    }

    pub(crate) fn configure_filters(
        &self,
        properties: &HashMap<String, String>,
        system_properties: &HashMap<String, String>,
    ) -> Result<(), DruidError> {
        for filter in &self.before {
            filter.config_from_properties(properties)?;
            filter.config_from_system_properties(system_properties)?;
        }
        Ok(())
    }

    pub(crate) async fn destroy_filters(&self) {
        for filter in &self.before {
            if let Err(error) = filter.destroy().await {
                tracing::warn!(%error, filter = filter.name(), "destroy filter error");
            }
        }
    }

    pub async fn data_source_get_connection(
        &self,
        provider: &dyn DataSourceConnectionProvider,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        DataSourceGetConnectionFilterChain::new(&self.before, provider)
            .data_source_get_connection(max_wait)
            .await
    }

    pub async fn data_source_release_connection(
        &self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        DataSourceReleaseConnectionFilterChain::new(&self.before)
            .data_source_recycle(connection)
            .await
    }

    pub async fn connection_close(
        &self,
        factory: &dyn ConnectionFactory,
        connection: &mut Box<dyn Connection>,
        connection_id: u64,
        connection_age: Duration,
    ) -> Result<(), DruidError> {
        ConnectionCloseFilterChain::new(
            &self.before,
            factory,
            connection,
            ConnectionCloseContext {
                connection_id,
                connection_age,
            },
        )
        .connection_close()
        .await
    }

    /// 通过标准连接按注册顺序读取警告，允许过滤器包围或短路。
    pub async fn connection_get_warnings(
        &self,
        connection: &mut dyn Connection,
    ) -> Result<Option<SqlWarning>, DruidError> {
        StandardConnectionWarningFilterChain::new(&self.before, connection)
            .connection_get_warnings()
            .await
    }

    /// 通过标准连接按注册顺序清除警告，允许过滤器包围或短路。
    pub async fn connection_clear_warnings(
        &self,
        connection: &mut dyn Connection,
    ) -> Result<(), DruidError> {
        StandardConnectionWarningFilterChain::new(&self.before, connection)
            .connection_clear_warnings()
            .await
    }

    /// 按注册顺序检查标准语句属性读取；参数为 getter 事件，返回首个过滤器错误。
    pub(crate) fn standard_statement_property_read(
        &self,
        event: &super::super::StatementPropertyEvent,
    ) -> Result<(), DruidError> {
        for filter in &self.before {
            filter.standard_statement_property_read(event)?;
        }
        Ok(())
    }

    /// 包围标准语句批次清除；参数为底层标准语句，返回过滤或驱动结果。
    pub(crate) fn standard_statement_clear_batch(
        &self,
        statement: &mut dyn druid_rdbc::statement::Statement,
    ) -> Result<(), DruidError> {
        super::super::StandardStatementClearBatchFilterChain::new(&self.before, statement)
            .statement_clear_batch()
    }

    /// 包围语句设置或取消；参数为原始语句和操作，返回最终生效参数。
    pub(crate) fn standard_statement_mutation(
        &self,
        statement: &mut dyn druid_rdbc::statement::Statement,
        mutation: super::super::StatementMutation,
    ) -> Result<super::super::StatementMutation, DruidError> {
        super::super::StandardStatementMutationFilterChain::new(
            &self.before,
            statement,
            mutation.clone(),
        )
        .apply(mutation)
    }

    /// 包围 Callable NULL 状态读取；参数为延迟驱动读取，返回过滤后的结果。
    pub(crate) fn standard_callable_was_null(
        &self,
        terminal: &mut dyn FnMut() -> Result<bool, DruidError>,
    ) -> Result<bool, DruidError> {
        super::super::StandardCallableWasNullFilterChain::new(&self.before, terminal)
            .callable_statement_was_null()
    }

    /// 按注册顺序读取标准语句警告；参数为驱动语句，返回完整警告链。
    pub(crate) fn standard_statement_get_warnings(
        &self,
        statement: &mut dyn druid_rdbc::statement::Statement,
    ) -> Result<Option<SqlWarning>, DruidError> {
        super::super::StandardStatementWarningFilterChain::new(&self.before, statement)
            .statement_get_warnings()
    }

    /// 按注册顺序清除标准语句警告；参数为驱动语句，返回操作结果。
    pub(crate) fn standard_statement_clear_warnings(
        &self,
        statement: &mut dyn druid_rdbc::statement::Statement,
    ) -> Result<(), DruidError> {
        super::super::StandardStatementWarningFilterChain::new(&self.before, statement)
            .statement_clear_warnings()
    }

    /// 在标准连接上以注册顺序创建 Blob，返回已登记结果。
    pub(crate) async fn connection_create_blob(
        &self,
        connection: &mut dyn Connection,
        registry: Arc<StandardResourceRegistry>,
        admission: Arc<StandardResourceAdmission>,
        policy: Arc<StandardLogicalPolicy>,
    ) -> Result<RegisteredLob<dyn Blob>, DruidError> {
        StandardConnectionLobFilterChain::new(&self.before, connection, registry, policy, admission)
            .connection_create_blob()
            .await
    }

    /// 在标准连接上以注册顺序创建 Clob，返回已登记结果。
    pub(crate) async fn connection_create_clob(
        &self,
        connection: &mut dyn Connection,
        registry: Arc<StandardResourceRegistry>,
        admission: Arc<StandardResourceAdmission>,
        policy: Arc<StandardLogicalPolicy>,
    ) -> Result<RegisteredLob<dyn Clob>, DruidError> {
        StandardConnectionLobFilterChain::new(&self.before, connection, registry, policy, admission)
            .connection_create_clob()
            .await
    }

    /// 在标准连接上以注册顺序创建 NClob，返回已登记结果。
    pub(crate) async fn connection_create_n_clob(
        &self,
        connection: &mut dyn Connection,
        registry: Arc<StandardResourceRegistry>,
        admission: Arc<StandardResourceAdmission>,
        policy: Arc<StandardLogicalPolicy>,
    ) -> Result<RegisteredLob<dyn NClob>, DruidError> {
        StandardConnectionLobFilterChain::new(&self.before, connection, registry, policy, admission)
            .connection_create_n_clob()
            .await
    }

    pub async fn connection_open(
        &self,
        factory: &dyn ConnectionFactory,
        properties: &mut HashMap<String, String>,
        login_timeout_seconds: i32,
        next_connection_id: &mut (dyn FnMut() -> u64 + Send),
    ) -> Result<ConnectionOpenResult, DruidError> {
        ConnectionOpenFilterChain::new(
            &self.before,
            factory,
            login_timeout_seconds,
            next_connection_id,
        )
        .connection_connect(properties)
        .await
    }

    pub async fn clob_length(&self, wrapper: &dyn ClobProxy) -> Result<i64, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_length()
            .await
    }

    pub async fn clob_get_sub_string(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
        length: i32,
    ) -> Result<RdbcString, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_get_sub_string(position, length)
            .await
    }

    pub async fn clob_get_character_stream(
        &self,
        wrapper: &dyn ClobProxy,
    ) -> Result<RdbcReader, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_get_character_stream()
            .await
    }

    pub async fn clob_get_ascii_stream(
        &self,
        wrapper: &dyn ClobProxy,
    ) -> Result<RdbcInputStream, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_get_ascii_stream()
            .await
    }

    pub async fn clob_get_character_stream_range(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_get_character_stream_range(position, length)
            .await
    }

    pub async fn clob_position_string(
        &self,
        wrapper: &dyn ClobProxy,
        pattern: &RdbcString,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_position_string(pattern, start)
            .await
    }

    pub async fn clob_position_clob(
        &self,
        wrapper: &dyn ClobProxy,
        pattern: &RdbcClob,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_position_clob(pattern, start)
            .await
    }

    pub async fn clob_set_string(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
        value: &RdbcString,
    ) -> Result<i32, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_set_string(position, value)
            .await
    }

    pub async fn clob_set_string_range(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
        value: &RdbcString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_set_string_range(position, value, offset, length)
            .await
    }

    pub async fn clob_set_ascii_stream(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
    ) -> Result<RdbcOutputStream, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_set_ascii_stream(position)
            .await
    }

    pub async fn clob_set_character_stream(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
    ) -> Result<RdbcWriter, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_set_character_stream(position)
            .await
    }

    pub async fn clob_truncate(
        &self,
        wrapper: &dyn ClobProxy,
        length: i64,
    ) -> Result<(), DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_truncate(length)
            .await
    }

    pub async fn clob_free(&self, wrapper: &dyn ClobProxy) -> Result<(), DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_free()
            .await
    }

    pub async fn before_execute(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        for (index, filter) in self.before.iter().enumerate() {
            if let Err(error) = filter.before(context).await {
                for completed in self.before[..index].iter().rev() {
                    let _ = completed.before_execute_error(context, &error).await;
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub async fn after_execute(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let mut first_error = None;
        for filter in self.after.iter().rev() {
            if let Err(error) = filter.after(context, result, elapsed).await {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}
