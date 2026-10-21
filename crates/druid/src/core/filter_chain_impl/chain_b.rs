use super::*;

impl FilterChainImpl {
    pub async fn before_batch(&self, context: &mut BatchExecContext<'_>) -> Result<(), DruidError> {
        for (index, filter) in self.before.iter().enumerate() {
            if let Err(error) = filter.before_batch(context).await {
                for completed in self.before[..index].iter().rev() {
                    let _ = completed.before_batch_error(context, &error).await;
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub async fn after_batch(
        &self,
        context: &BatchExecContext<'_>,
        result: &Result<Vec<i32>, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let mut first_error = None;
        for filter in self.after.iter().rev() {
            if let Err(error) = filter.after_batch(context, result, elapsed).await {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub fn prepare_statement_sql(&self, sql: &str) -> Result<String, DruidError> {
        self.before
            .iter()
            .try_fold(sql.to_owned(), |current, filter| {
                filter.prepare_statement_sql(&current)
            })
    }

    pub fn statement_add_batch_sql(&self, sql: &str) -> Result<String, DruidError> {
        self.before
            .iter()
            .try_fold(sql.to_owned(), |current, filter| {
                filter.statement_add_batch_sql(&current)
            })
    }

    pub async fn before_connection_event_with_identity(
        &self,
        connection_id: u64,
        event: &ConnectionEvent,
    ) -> Result<(), DruidError> {
        let context = ConnectionEventContext {
            connection_id,
            event,
        };
        for filter in &self.before {
            filter.on_connection_event_context(&context).await?;
        }
        Ok(())
    }

    pub async fn after_connection_event_with_identity(
        &self,
        connection_id: u64,
        event: &ConnectionEvent,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let context = ConnectionEventContext {
            connection_id,
            event,
        };
        for filter in self.after.iter().rev() {
            filter
                .after_connection_event_context(&context, elapsed)
                .await?;
        }
        Ok(())
    }

    pub async fn after_statement_event_with_identity(
        &self,
        connection_id: u64,
        statement_id: u64,
        event: &StatementEvent,
    ) -> Result<(), DruidError> {
        let context = StatementEventContext {
            connection_id,
            statement_id,
            event,
        };
        for filter in self.before.iter().rev() {
            filter.on_statement_event_context(&context).await?;
        }
        Ok(())
    }

    pub fn after_statement_close_with_identity(
        &self,
        connection_id: u64,
        statement_id: u64,
    ) -> Result<(), DruidError> {
        let event = StatementEvent::Close;
        let context = StatementEventContext {
            connection_id,
            statement_id,
            event: &event,
        };
        for filter in self.before.iter().rev() {
            filter.on_statement_close_context(&context)?;
        }
        Ok(())
    }

    /// 从首个过滤器包围完整标准 metadata 获取；参数为驱动连接及逻辑 ID。
    pub(crate) async fn standard_connection_meta_data(
        &self,
        connection: &mut dyn Connection,
        connection_id: u64,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, DruidError> {
        super::super::StandardConnectionMetaDataFilterChain::new(
            &self.before,
            connection,
            connection_id,
        )
        .connection_get_meta_data()
        .await
    }

    pub(crate) fn standard_result_set_open_after_with_proxy(
        &self,
        context: &mut super::super::StandardResultSetOpenContext<'_>,
    ) -> Result<(), DruidError> {
        for filter in self.result_set.iter().rev() {
            filter.standard_result_set_open_after_with_proxy(context)?;
        }
        Ok(())
    }

    /// 包围原导航参数；普通默认链不读取 row，池化路径显式启用实际行号跟踪。
    /// 参数为同一驱动、上下文、导航和跟踪开关；返回结果及可空真实行号。
    /// 对应 Java: FilterChainImpl 的默认导航与 DruidPooledResultSet 的游标状态治理。
    pub(crate) async fn standard_result_set_navigation(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        navigation: druid_rdbc::result_set::ResultSetNavigation,
        capture_row: bool,
    ) -> Result<(bool, Option<i32>), DruidError> {
        let mut chain = if capture_row {
            StandardResultSetFilterChain::new_with_navigation_row(
                &self.result_set,
                result_set,
                context,
            )
        } else {
            StandardResultSetFilterChain::new(&self.result_set, result_set, context)
        };
        let present = chain.result_set_navigation(navigation).await?;
        Ok((present, chain.navigation_row()))
    }

    /// 按注册顺序包围原行操作；参数为上下文、操作及惰性驱动末端，返回原结果。
    /// 对应 Java: FilterChainImpl 的七个 ResultSet 行操作。
    pub(crate) fn standard_result_set_row_operation(
        &self,
        context: &ResultSetFilterContext,
        operation: super::super::ResultSetRowOperation,
        terminal: &mut dyn FnMut() -> Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        fn invoke(
            filters: &[Arc<dyn super::super::ResultSetFilter>],
            context: &ResultSetFilterContext,
            operation: super::super::ResultSetRowOperation,
            terminal: &mut dyn FnMut() -> Result<(), DruidError>,
        ) -> Result<(), DruidError> {
            match filters.split_first() {
                Some((filter, remaining)) => {
                    filter.standard_result_set_row_operation(context, operation, &mut || {
                        invoke(remaining, context, operation, terminal)
                    })
                }
                None => terminal(),
            }
        }
        invoke(&self.result_set, context, operation, terminal)
    }

    /// 按注册顺序读取结果集警告；参数为上下文及驱动末端，返回完整警告链。
    pub(crate) fn standard_result_set_get_warnings(
        &self,
        context: &ResultSetFilterContext,
        terminal: &mut dyn FnMut() -> Result<Option<SqlWarning>, DruidError>,
    ) -> Result<Option<SqlWarning>, DruidError> {
        fn invoke(
            filters: &[Arc<dyn super::super::ResultSetFilter>],
            context: &ResultSetFilterContext,
            terminal: &mut dyn FnMut() -> Result<Option<SqlWarning>, DruidError>,
        ) -> Result<Option<SqlWarning>, DruidError> {
            match filters.split_first() {
                Some((filter, remaining)) => filter
                    .standard_result_set_get_warnings(context, &mut || {
                        invoke(remaining, context, terminal)
                    }),
                None => terminal(),
            }
        }
        invoke(&self.result_set, context, terminal)
    }

    /// 按注册顺序清除结果集警告；参数为上下文及驱动末端，返回原结果。
    pub(crate) fn standard_result_set_clear_warnings(
        &self,
        context: &ResultSetFilterContext,
        terminal: &mut dyn FnMut() -> Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        fn invoke(
            filters: &[Arc<dyn super::super::ResultSetFilter>],
            context: &ResultSetFilterContext,
            terminal: &mut dyn FnMut() -> Result<(), DruidError>,
        ) -> Result<(), DruidError> {
            match filters.split_first() {
                Some((filter, remaining)) => filter
                    .standard_result_set_clear_warnings(context, &mut || {
                        invoke(remaining, context, terminal)
                    }),
                None => terminal(),
            }
        }
        invoke(&self.result_set, context, terminal)
    }

    /// 按注册顺序包围属性调用；参数为原请求和末端，保留短路及错误反向退出。
    /// 对应 Java: FilterChainImpl 的 ResultSet 属性方法。
    pub(crate) fn standard_result_set_property(
        &self,
        context: &ResultSetFilterContext,
        request: super::super::ResultSetPropertyOperation<'_>,
        terminal: &mut dyn FnMut() -> Result<super::super::ResultSetPropertyValue, DruidError>,
    ) -> Result<super::super::ResultSetPropertyValue, DruidError> {
        fn invoke(
            filters: &[Arc<dyn super::super::ResultSetFilter>],
            context: &ResultSetFilterContext,
            request: super::super::ResultSetPropertyOperation<'_>,
            terminal: &mut dyn FnMut() -> Result<super::super::ResultSetPropertyValue, DruidError>,
        ) -> Result<super::super::ResultSetPropertyValue, DruidError> {
            match filters.split_first() {
                Some((filter, remaining)) => {
                    filter.standard_result_set_property(context, request, &mut || {
                        invoke(remaining, context, request, terminal)
                    })
                }
                None => terminal(),
            }
        }
        invoke(&self.result_set, context, request, terminal)
    }

    pub(crate) async fn standard_result_set_next(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_next()
            .await
    }

    pub(crate) async fn standard_result_set_close(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_close()
            .await
    }

    pub(crate) fn standard_result_set_value(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: ColumnIndex,
    ) -> Result<super::super::Value, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_value(column)
    }

    /// 保留 value around-hook 顺序，末端使用标准驱动的 getString 语义。
    pub(crate) fn standard_result_set_string_value(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: ColumnIndex,
    ) -> Result<super::super::Value, DruidError> {
        StandardResultSetFilterChain::new_string(&self.result_set, result_set, context)
            .result_set_value(column)
    }

    /// 包围逻辑创建语句查询；参数为上下文和原所有者末端，返回原或替换标准句柄。
    /// 对应 Java: FilterChainImpl#resultSet_getStatement；不借用或公开原生游标。
    pub(crate) fn standard_result_set_statement(
        &self,
        context: &ResultSetFilterContext,
        terminal: &mut dyn FnMut() -> Result<
            Option<Box<dyn druid_rdbc::statement::Statement>>,
            DruidError,
        >,
    ) -> Result<Option<Box<dyn druid_rdbc::statement::Statement>>, DruidError> {
        fn invoke(
            filters: &[Arc<dyn super::super::ResultSetFilter>],
            context: &ResultSetFilterContext,
            terminal: &mut dyn FnMut() -> Result<
                Option<Box<dyn druid_rdbc::statement::Statement>>,
                DruidError,
            >,
        ) -> Result<Option<Box<dyn druid_rdbc::statement::Statement>>, DruidError> {
            if let Some((filter, remaining)) = filters.split_first() {
                filter.standard_result_set_statement(context, &mut || {
                    invoke(remaining, context, terminal)
                })
            } else {
                terminal()
            }
        }
        invoke(&self.result_set, context, terminal)
    }

    /// 从首个Filter包围完整对象重载，返回未扁平化的驱动或短路对象。
    /// 参数为驱动、上下文、治理列、原请求和原列名；对应 Java: FilterChainImpl#getObject。
    pub(crate) fn standard_result_set_object(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        request: &super::super::ResultSetObjectRead<'_>,
        label: Option<&str>,
    ) -> Result<druid_rdbc::types::RdbcObject, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_object(column, label, request)
    }

    /// 从首个Filter包围资源对象请求，返回完整原生句柄或精确错误。
    /// 参数为驱动、上下文、治理列、原请求和标签。对应 Java: FilterChainImpl#resultSet_getXxx。
    pub(crate) fn standard_result_set_resource(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        request: &druid_rdbc::result_set::ResultSetRead,
        label: Option<&str>,
    ) -> Result<druid_rdbc::types::RdbcObject, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_resource(column, label, request)
    }

    /// 包围字节流请求，默认保留原生重载；参数为驱动和完整读取身份，返回原流。
    /// 对应 Java: FilterChainImpl#resultSet_getAsciiStream/getUnicodeStream/getBinaryStream。
    pub(crate) fn standard_result_set_input_stream(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        kind: druid_rdbc::result_set::ResultSetStreamKind,
    ) -> Result<Option<druid_rdbc::resource::RdbcInputStream>, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_input_stream(column, label, kind)
    }

    /// 包围字符流请求，不预读或复制Reader；参数为驱动和完整重载，返回原Reader。
    /// 对应 Java: FilterChainImpl#resultSet_getCharacterStream/getNCharacterStream。
    pub(crate) fn standard_result_set_reader(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        national: bool,
    ) -> Result<Option<druid_rdbc::resource::RdbcReader>, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_reader(column, label, national)
    }

    /// 强类型标量保留完整读取 around-hook；默认委托继续既有 value hooks。
    /// 参数含驱动、上下文、列、请求和可空原列名，返回过滤后的值。
    /// 对应 Java: FilterChainImpl#resultSet_getBigDecimal/getDate/getTime/getTimestamp。
    pub(crate) fn standard_result_set_typed_value(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        request: &druid_rdbc::result_set::ResultSetRead,
        label: Option<&str>,
    ) -> Result<super::super::Value, DruidError> {
        StandardResultSetFilterChain::new_typed(
            &self.result_set,
            result_set,
            context,
            request,
            label,
        )
        .result_set_read(column)
    }

    /// 从position0执行完整下标更新链；参数为驱动、上下文、列和原描述符，返回执行结果。
    /// 对应 Java: FilterChainImpl#resultSet_updateXxx(int, ...)。
    pub(crate) fn standard_result_set_update(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: ColumnIndex,
        update: druid_rdbc::result_set::ResultSetUpdate,
    ) -> Result<(), DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_update(column, update)
    }

    /// 从position0执行完整原列名更新链；参数为驱动、上下文、列名和描述符，返回结果。
    /// 对应 Java: FilterChainImpl#resultSet_updateXxx(String, ...)。
    pub(crate) fn standard_result_set_update_by_label(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        update: druid_rdbc::result_set::ResultSetUpdate,
    ) -> Result<(), DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_update_by_label(label, update)
    }

    pub(crate) fn standard_result_set_meta_data(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<Arc<dyn druid_rdbc::metadata::ResultSetMetaData>, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_meta_data()
    }

    pub(crate) fn standard_result_set_was_null(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_was_null()
    }
}
