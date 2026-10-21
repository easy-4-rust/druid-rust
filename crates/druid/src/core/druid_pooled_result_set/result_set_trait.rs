use super::*;

#[async_trait::async_trait]
impl ResultSet for DruidPooledResultSet {
    /// 返回共享的创建语句，保留 Prepared/Callable 叶子接口；metadata 结果返回 None。
    /// 对应 Java: java.sql.ResultSet#getStatement。
    fn get_statement(
        &self,
    ) -> Result<Option<Box<dyn druid_rdbc::statement::Statement>>, RdbcError> {
        // 创建语句身份属于逻辑所有权，不依赖 raw cursor 的开放状态。
        // BASE 允许在 Statement 级联关闭后仍查询同一关闭句柄；失效租约仍拒绝访问。
        if !self.statement.inner.lease_active.load(Ordering::Acquire) {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let mut terminal = || {
            if !self.has_creating_statement {
                return Ok(None);
            }
            let statement: Box<dyn druid_rdbc::statement::Statement> = match &self.logical_owner {
                Some(owner) => Arc::clone(owner).statement_handle(),
                None => Box::new(self.statement.clone()),
            };
            Ok(Some(statement))
        };
        let result = match &self.resource.filter_chain {
            Some(filters) => {
                filters.standard_result_set_statement(&self.filter_context, &mut terminal)
            }
            None => terminal(),
        }
        .map_err(RdbcError::from);
        // 查询属于逻辑身份，不要求原生游标开放；Filter失败仍分类并只记原创建语句一次。
        self.record_operation_result(
            self.resource
                .policy
                .classify(result, self.filter_context.sql()),
        )
    }

    fn known_row_count(&self) -> Option<u64> {
        self.ensure_open().ok()?;
        // Statement cascade 或 more-results 可先关闭共享 resource，而不修改此代理标志。
        if self.resource.is_resource_closed() {
            return None;
        }
        self.resource.known_row_count
    }

    async fn next(&mut self) -> Result<bool, RdbcError> {
        // 有效租约中的已关闭游标保留原错误；旧租约始终由策略优先拒绝。
        self.resource.policy.ensure_open()?;
        if self.shared.closed.load(Ordering::Acquire) {
            return Err(RdbcError::Other("result set is closed".to_owned()));
        }
        let more = self.record_operation_result(self.resource.next_core().await)?;
        if more {
            let cursor = self.shared.cursor_index.fetch_add(1, Ordering::AcqRel) + 1;
            self.filter_context.record_fetch_row_count(cursor);
        }
        Ok(more)
    }

    async fn navigate(
        &mut self,
        navigation: druid_rdbc::result_set::ResultSetNavigation,
    ) -> Result<bool, RdbcError> {
        if std::matches!(
            navigation,
            druid_rdbc::result_set::ResultSetNavigation::Next
        ) {
            return self.next().await;
        }
        self.ensure_open()?;
        let (present, native_row) =
            self.record_operation_result(self.resource.navigate_core(navigation).await)?;
        if let Some(row) = native_row {
            self.shared.cursor_index.store(row, Ordering::Release);
            if present {
                self.filter_context.record_fetch_row_count(row);
            }
        }
        Ok(present)
    }

    fn row(&self) -> Result<i32, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::Row,
            |raw| raw.row().map(super::super::ResultSetPropertyValue::Integer),
            super::super::ResultSetPropertyValue::into_integer,
        )
    }

    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.ensure_open()?;
        self.resource.value_core(self.driver_column_index(column)?)
    }

    fn string_value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.ensure_open()?;
        let result = self
            .resource
            .string_value_core(self.driver_column_index(column)?);
        // 仅成功的 getString 返回值计入 UTF-16 长度，普通 value/object 不计入。
        if let Ok(Value::String(value)) = &result {
            self.filter_context.add_read_string_length(value);
        }
        result
    }

    fn find_column(&self, column_label: &str) -> Result<ColumnIndex, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::FindColumn(column_label),
            |raw| {
                raw.find_column(column_label)
                    .map(super::super::ResultSetPropertyValue::Column)
            },
            |value| self.logical_column_index(value.into_column()?),
        )
    }

    fn object(&self, column: ColumnIndex) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.filtered_object(
            Some(column),
            None,
            &super::super::ResultSetObjectRead::Ordinary,
        )
    }

    /// 按原列名转发普通对象；参数为列名，返回原驱动对象或精确错误。
    /// 对应 Java: DruidPooledResultSet#getObject(String)。
    fn object_by_label(&self, label: &str) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.filtered_object(
            None,
            Some(label),
            &super::super::ResultSetObjectRead::Ordinary,
        )
    }

    /// 按逻辑列转发完整读取请求，保留 Filter、列治理与驱动错误分类。
    /// 参数为列和强类型重载，返回原资源或过滤后的标量。
    /// 对应 Java: DruidPooledResultSet#getBigDecimal/getDate/getTime/getTimestamp/getBlob 等。
    fn read_object(
        &self,
        column: ColumnIndex,
        request: &ResultSetRead,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            // 列治理失败也沿同一策略分类并向创建语句记错，保持双入口一致。
            let column = self.driver_column_index(column)?;
            if matches!(
                request,
                ResultSetRead::String
                    | ResultSetRead::NString
                    | ResultSetRead::Boolean
                    | ResultSetRead::Long
                    | ResultSetRead::Int
                    | ResultSetRead::Short
                    | ResultSetRead::Byte
                    | ResultSetRead::Double
                    | ResultSetRead::Float
                    | ResultSetRead::Bytes
                    | ResultSetRead::BigDecimal(_)
                    | ResultSetRead::Date(_)
                    | ResultSetRead::Time(_)
                    | ResultSetRead::Timestamp(_)
            ) {
                if let Some(filters) = &self.resource.filter_chain {
                    return filters
                        .standard_result_set_typed_value(
                            raw,
                            &self.filter_context,
                            Some(column),
                            request,
                            None,
                        )
                        .map(druid_rdbc::types::RdbcObject::Scalar)
                        .map_err(RdbcError::from);
                }
            }
            match &self.resource.filter_chain {
                Some(filters) => filters
                    .standard_result_set_resource(
                        raw,
                        &self.filter_context,
                        Some(column),
                        request,
                        None,
                    )
                    .map_err(RdbcError::from),
                None => raw.read_object(column, request),
            }
        });
        // getString 在标准描述符分派后仍只记录成功返回值的 UTF-16 长度。
        if matches!(request, ResultSetRead::String) {
            if let Ok(druid_rdbc::types::RdbcObject::Scalar(Value::String(value))) = &result {
                self.filter_context.add_read_string_length(value);
            }
        }
        self.record_operation_result(result)
    }

    /// 按原列名读取，配置列治理时先验证可见性；返回资源或标量。
    /// 对应 Java: DruidPooledResultSet#getXxx(String, ...)。
    fn read_object_by_label(
        &self,
        label: &str,
        request: &ResultSetRead,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            let scalar = matches!(
                request,
                ResultSetRead::String
                    | ResultSetRead::NString
                    | ResultSetRead::Boolean
                    | ResultSetRead::Long
                    | ResultSetRead::Int
                    | ResultSetRead::Short
                    | ResultSetRead::Byte
                    | ResultSetRead::Double
                    | ResultSetRead::Float
                    | ResultSetRead::Bytes
                    | ResultSetRead::BigDecimal(_)
                    | ResultSetRead::Date(_)
                    | ResultSetRead::Time(_)
                    | ResultSetRead::Timestamp(_)
            );
            let governed = self.shared.hidden_columns.read().is_some()
                || self.shared.logic_column_map.read().is_some()
                || self.shared.driver_column_map.read().is_some();
            let column = if governed {
                let driver = raw.find_column(label)?;
                let logical = self.logical_column_index(driver)?;
                Some(self.driver_column_index(logical)?)
            } else {
                None
            };
            if scalar {
                if let Some(filters) = &self.resource.filter_chain {
                    return filters
                        .standard_result_set_typed_value(
                            raw,
                            &self.filter_context,
                            column,
                            request,
                            Some(label),
                        )
                        .map(druid_rdbc::types::RdbcObject::Scalar)
                        .map_err(RdbcError::from);
                }
            }
            match &self.resource.filter_chain {
                Some(filters) => filters
                    .standard_result_set_resource(
                        raw,
                        &self.filter_context,
                        column,
                        request,
                        Some(label),
                    )
                    .map_err(RdbcError::from),
                None => raw.read_object_by_label(label, request),
            }
        });
        // getString 在标准描述符分派后仍只记录成功返回值的 UTF-16 长度。
        if matches!(request, ResultSetRead::String) {
            if let Ok(druid_rdbc::types::RdbcObject::Scalar(Value::String(value))) = &result {
                self.filter_context.add_read_string_length(value);
            }
        }
        self.record_operation_result(result)
    }

    /// 按驱动下标转发可空类型映射；返回原对象或原错误，并记录创建语句失败。
    /// 对应 Java: DruidPooledResultSet#getObject(int, Map)。
    fn object_with_type_map(
        &self,
        column: ColumnIndex,
        type_map: Option<&druid_rdbc::types::RdbcTypeMap>,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.filtered_object(
            Some(column),
            None,
            &super::super::ResultSetObjectRead::TypeMap(type_map),
        )
    }

    fn object_as(
        &self,
        column: ColumnIndex,
        target_type: &druid_rdbc::types::RdbcTargetType,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.filtered_object(
            Some(column),
            None,
            &super::super::ResultSetObjectRead::Typed(target_type),
        )
    }

    /// 按原列名转发可空类型映射；返回驱动对象并保留列治理与错误分类。
    /// 对应 Java: DruidPooledResultSet#getObject(String, Map)。
    fn object_with_type_map_by_label(
        &self,
        label: &str,
        type_map: Option<&druid_rdbc::types::RdbcTypeMap>,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.filtered_object(
            None,
            Some(label),
            &super::super::ResultSetObjectRead::TypeMap(type_map),
        )
    }

    /// 按原列名转发目标类型；返回驱动原对象，不强制重新转换返回值。
    /// 对应 Java: DruidPooledResultSet#getObject(String, Class)。
    fn object_as_by_label(
        &self,
        label: &str,
        target_type: &druid_rdbc::types::RdbcTargetType,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.filtered_object(
            None,
            Some(label),
            &super::super::ResultSetObjectRead::Typed(target_type),
        )
    }

    fn input_stream(
        &self,
        column: ColumnIndex,
        kind: druid_rdbc::result_set::ResultSetStreamKind,
    ) -> Result<Option<druid_rdbc::resource::RdbcInputStream>, RdbcError> {
        self.filtered_input_stream(Some(column), None, kind)
    }

    /// 按原标签转发字节流重载；参数为标签和流类型，返回原共享流或错误。
    /// 对应 Java: DruidPooledResultSet#getAsciiStream/getUnicodeStream/getBinaryStream(String)。
    fn input_stream_by_label(
        &self,
        label: &str,
        kind: druid_rdbc::result_set::ResultSetStreamKind,
    ) -> Result<Option<druid_rdbc::resource::RdbcInputStream>, RdbcError> {
        self.filtered_input_stream(None, Some(label), kind)
    }

    fn reader(
        &self,
        column: ColumnIndex,
        national: bool,
    ) -> Result<Option<druid_rdbc::resource::RdbcReader>, RdbcError> {
        self.filtered_reader(Some(column), None, national)
    }

    /// 按原标签转发字符流重载；参数保留标签及national标志，返回原共享Reader。
    /// 对应 Java: DruidPooledResultSet#getCharacterStream/getNCharacterStream(String)。
    fn reader_by_label(
        &self,
        label: &str,
        national: bool,
    ) -> Result<Option<druid_rdbc::resource::RdbcReader>, RdbcError> {
        self.filtered_reader(None, Some(label), national)
    }

    fn update_value(
        &mut self,
        column: ColumnIndex,
        update: druid_rdbc::result_set::ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            let column = self.driver_column_index(column)?;
            if let Some(filters) = &self.resource.filter_chain {
                filters
                    .standard_result_set_update(raw, &self.filter_context, column, update)
                    .map_err(RdbcError::from)
            } else {
                raw.update_value(column, update)
            }
        });
        if let Err(error) = &result {
            self.resource.policy.record_result_set_error(error);
            // 策略已分类，ResultSet统计与创建语句异常各记录一次。
            self.statement.record_exception();
        }
        result
    }

    /// 按列名转发完整更新描述符；返回驱动结果，保留列名调用身份。
    /// 对应 Java: DruidPooledResultSet#updateXxx(String, ...)。
    fn update_value_by_label(
        &mut self,
        column_label: &str,
        update: druid_rdbc::result_set::ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            // 仅配置了列治理时解析可见性；普通列名直接抵达驱动列名入口。
            let governed = self.shared.hidden_columns.read().is_some()
                || self.shared.logic_column_map.read().is_some()
                || self.shared.driver_column_map.read().is_some();
            if governed {
                let logical = self.logical_column_index(raw.find_column(column_label)?)?;
                self.driver_column_index(logical)?;
            }
            if let Some(filters) = &self.resource.filter_chain {
                filters
                    .standard_result_set_update_by_label(
                        raw,
                        &self.filter_context,
                        column_label,
                        update,
                    )
                    .map_err(RdbcError::from)
            } else {
                raw.update_value_by_label(column_label, update)
            }
        });
        if let Err(error) = &result {
            self.resource.policy.record_result_set_error(error);
            // 策略已分类，ResultSet统计与创建语句异常各记录一次。
            self.statement.record_exception();
        }
        result
    }

    fn meta_data(&self) -> Result<Arc<dyn druid_rdbc::metadata::ResultSetMetaData>, RdbcError> {
        self.ensure_open()?;
        let result = self
            .meta_data_proxy_core()
            .map(|metadata| Arc::new(metadata) as Arc<dyn druid_rdbc::metadata::ResultSetMetaData>);
        self.record_operation_result(result)
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::WasNull,
            |raw| {
                // 默认属性委托保留原 wasNull before/after 链，不能绕过已有观察。
                let value = match &self.resource.filter_chain {
                    Some(filters) => filters
                        .standard_result_set_was_null(raw, &self.filter_context)
                        .map_err(RdbcError::from),
                    None => raw.was_null(),
                }?;
                Ok(super::super::ResultSetPropertyValue::Boolean(value))
            },
            super::super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn is_before_first(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::IsBeforeFirst,
            |raw| {
                raw.is_before_first()
                    .map(super::super::ResultSetPropertyValue::Boolean)
            },
            super::super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn is_after_last(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::IsAfterLast,
            |raw| {
                raw.is_after_last()
                    .map(super::super::ResultSetPropertyValue::Boolean)
            },
            super::super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn is_first(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::IsFirst,
            |raw| {
                raw.is_first()
                    .map(super::super::ResultSetPropertyValue::Boolean)
            },
            super::super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn is_last(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::IsLast,
            |raw| {
                raw.is_last()
                    .map(super::super::ResultSetPropertyValue::Boolean)
            },
            super::super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn set_fetch_direction(
        &mut self,
        direction: druid_rdbc::statement::FetchDirection,
    ) -> Result<(), RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::SetFetchDirection(direction),
            |raw| {
                raw.set_fetch_direction(direction)
                    .map(|()| super::super::ResultSetPropertyValue::Unit)
            },
            super::super::ResultSetPropertyValue::into_unit,
        )
    }

    fn fetch_direction(&self) -> Result<druid_rdbc::statement::FetchDirection, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::FetchDirection,
            |raw| {
                raw.fetch_direction()
                    .map(super::super::ResultSetPropertyValue::FetchDirection)
            },
            super::super::ResultSetPropertyValue::into_direction,
        )
    }

    fn set_fetch_size(&mut self, rows: i32) -> Result<(), RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::SetFetchSize(rows),
            |raw| {
                raw.set_fetch_size(rows)
                    .map(|()| super::super::ResultSetPropertyValue::Unit)
            },
            super::super::ResultSetPropertyValue::into_unit,
        )
    }

    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::FetchSize,
            |raw| {
                raw.fetch_size()
                    .map(super::super::ResultSetPropertyValue::Integer)
            },
            super::super::ResultSetPropertyValue::into_integer,
        )
    }

    fn result_set_type(&self) -> Result<druid_rdbc::statement::ResultSetType, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::ResultSetType,
            |raw| {
                raw.result_set_type()
                    .map(super::super::ResultSetPropertyValue::ResultSetType)
            },
            super::super::ResultSetPropertyValue::into_result_set_type,
        )
    }

    fn concurrency(&self) -> Result<druid_rdbc::statement::ResultSetConcurrency, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::Concurrency,
            |raw| {
                raw.concurrency()
                    .map(super::super::ResultSetPropertyValue::Concurrency)
            },
            super::super::ResultSetPropertyValue::into_concurrency,
        )
    }

    fn holdability(&self) -> Result<druid_rdbc::statement::ResultSetHoldability, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::Holdability,
            |raw| {
                raw.holdability()
                    .map(super::super::ResultSetPropertyValue::Holdability)
            },
            super::super::ResultSetPropertyValue::into_holdability,
        )
    }

    fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            let mut terminal = || raw.warnings().map_err(super::super::DruidError::from);
            match &self.resource.filter_chain {
                Some(filters) => filters
                    .standard_result_set_get_warnings(&self.filter_context, &mut terminal)
                    .map_err(RdbcError::from),
                None => terminal().map_err(RdbcError::from),
            }
        });
        self.record_operation_result(result)
    }

    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            let mut terminal = || raw.clear_warnings().map_err(super::super::DruidError::from);
            match &self.resource.filter_chain {
                Some(filters) => filters
                    .standard_result_set_clear_warnings(&self.filter_context, &mut terminal)
                    .map_err(RdbcError::from),
                None => terminal().map_err(RdbcError::from),
            }
        });
        self.record_operation_result(result)
    }

    fn cursor_name(&self) -> Result<Option<String>, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::CursorName,
            |raw| {
                raw.cursor_name()
                    .map(super::super::ResultSetPropertyValue::String)
            },
            super::super::ResultSetPropertyValue::into_string,
        )
    }

    fn row_updated(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::RowUpdated,
            |raw| {
                raw.row_updated()
                    .map(super::super::ResultSetPropertyValue::Boolean)
            },
            super::super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn row_inserted(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::RowInserted,
            |raw| {
                raw.row_inserted()
                    .map(super::super::ResultSetPropertyValue::Boolean)
            },
            super::super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn row_deleted(&self) -> Result<bool, RdbcError> {
        self.filtered_property(
            super::super::ResultSetPropertyOperation::RowDeleted,
            |raw| {
                raw.row_deleted()
                    .map(super::super::ResultSetPropertyValue::Boolean)
            },
            super::super::ResultSetPropertyValue::into_boolean,
        )
    }

    fn insert_row(&mut self) -> Result<(), RdbcError> {
        self.filtered_row_operation(super::super::ResultSetRowOperation::InsertRow)
    }

    fn update_row(&mut self) -> Result<(), RdbcError> {
        self.filtered_row_operation(super::super::ResultSetRowOperation::UpdateRow)
    }

    fn delete_row(&mut self) -> Result<(), RdbcError> {
        self.filtered_row_operation(super::super::ResultSetRowOperation::DeleteRow)
    }

    fn refresh_row(&mut self) -> Result<(), RdbcError> {
        self.filtered_row_operation(super::super::ResultSetRowOperation::RefreshRow)
    }

    fn cancel_row_updates(&mut self) -> Result<(), RdbcError> {
        self.filtered_row_operation(super::super::ResultSetRowOperation::CancelRowUpdates)
    }

    fn move_to_insert_row(&mut self) -> Result<(), RdbcError> {
        self.filtered_row_operation(super::super::ResultSetRowOperation::MoveToInsertRow)
    }

    fn move_to_current_row(&mut self) -> Result<(), RdbcError> {
        self.filtered_row_operation(super::super::ResultSetRowOperation::MoveToCurrentRow)
    }

    /// Stream 唯一拥有型消费结束后交接原 registry；无参数、无返回值。
    /// 对应 Java: DruidPooledResultSet#close 的 Rust 异步 Drop 适配。
    fn release_owned(self: Box<Self>) {
        if !self.shared.closed.load(Ordering::Acquire) {
            if let Some(registration) = self.resource.registration.get() {
                registration.request_cleanup();
            }
        }
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if self.shared.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        self.resource.close_resource().await?;
        self.shared.closed.store(true, Ordering::Release);
        self.statement
            .record_fetch_row_count(self.filter_context.fetch_row_count());
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        let closed =
            self.shared.closed.load(Ordering::Acquire) || self.resource.is_resource_closed();
        if closed || !self.statement.inner.lease_active.load(Ordering::Acquire) {
            return Ok(closed);
        }
        self.filtered_property(
            super::super::ResultSetPropertyOperation::IsClosed,
            |_| Ok(super::super::ResultSetPropertyValue::Boolean(closed)),
            super::super::ResultSetPropertyValue::into_boolean,
        )
    }
}
