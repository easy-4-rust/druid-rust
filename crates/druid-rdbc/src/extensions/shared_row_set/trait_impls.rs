use super::*;

impl RowSetInternal for SharedRowSet {
    fn params(&self) -> Result<Vec<PreparedInputParameter>, RdbcError> {
        self.ensure_open()?;
        let (parameters, command, resolver) = {
            let state = self.lock();
            (
                state.parameters.clone(),
                state.properties.command().map(str::to_owned),
                state.resolver.clone(),
            )
        };
        parameters.resolve_params(|name| {
            resolver
                .as_ref()
                .ok_or_else(|| RowSetCursor::error("RowSet requires a named parameter resolver"))?(
                command.as_deref(),
                name,
            )
        })
    }
    fn connection(&mut self) -> Result<Option<&mut dyn Connection>, RdbcError> {
        self.ensure_open()?;
        Ok(None)
    }
    fn set_meta_data(&mut self, metadata: SharedRowSetMetaData) -> Result<(), RdbcError> {
        self.ensure_update()?;
        let cursor = self.cursor_handle();
        let mut cursor = cursor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !cursor.rows.is_empty() {
            return Err(RowSetCursor::error(
                "Cannot replace metadata after inserting rows",
            ));
        }
        cursor.rows = RowSetRowStore::new(metadata.column_count());
        cursor.metadata = metadata;
        Ok(())
    }
    fn original(&self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        let cursor = self.cursor_handle();
        let cursor = cursor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.original_result(cursor.rows.original_rows(), cursor.metadata.clone())
    }
    fn original_row(&self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        let cursor = self.cursor_handle();
        let cursor = cursor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.original_result(
            cursor.rows.original_row(Some(cursor.current_slot()?))?,
            cursor.metadata.clone(),
        )
    }
    fn row_set(&mut self) -> &mut dyn RowSet {
        self
    }
}

#[async_trait::async_trait]
impl RowSet for SharedRowSet {
    fn set_parameter(
        &mut self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.ensure_idle()?;
        self.lock().parameters.set_index(index, parameter);
        Ok(())
    }
    fn set_named_parameter(
        &mut self,
        name: Option<&str>,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.ensure_idle()?;
        self.lock().parameters.set_named(name, parameter)
    }
    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.ensure_idle()?;
        self.lock().parameters.clear_parameters();
        Ok(())
    }
    async fn execute(&mut self) -> Result<(), RdbcError> {
        self.execute_reader(None).await
    }
    fn add_row_set_listener(&mut self, listener: Arc<dyn RowSetListener>) {
        let mut state = self.lock();
        if !state
            .listeners
            .iter()
            .any(|current| Arc::ptr_eq(current, &listener))
        {
            state.listeners.push(listener);
        }
    }
    fn remove_row_set_listener(&mut self, listener: &Arc<dyn RowSetListener>) {
        self.lock()
            .listeners
            .retain(|current| !Arc::ptr_eq(current, listener));
    }
    fn url(&self) -> Option<String> {
        self.lock().properties.url().map(str::to_owned)
    }
    fn set_url(&mut self, url: Option<&str>) -> Result<(), RdbcError> {
        self.modify_properties(|properties| properties.set_url(url))
    }
    fn data_source_name(&self) -> Option<String> {
        self.lock().properties.data_source_name().map(str::to_owned)
    }
    fn set_data_source_name(&mut self, name: Option<&str>) -> Result<(), RdbcError> {
        self.modify_properties(|properties| properties.set_data_source_name(name))
    }
    fn username(&self) -> Option<String> {
        self.lock().properties.username().map(str::to_owned)
    }
    fn set_username(&mut self, name: Option<&str>) -> Result<(), RdbcError> {
        self.modify_properties(|properties| properties.set_username(name))
    }
    fn password(&self) -> Option<String> {
        self.lock().properties.password().map(str::to_owned)
    }
    fn set_password(&mut self, password: Option<&str>) -> Result<(), RdbcError> {
        self.modify_properties(|properties| properties.set_password(password))
    }
    fn transaction_isolation(&self) -> TransactionIsolation {
        self.lock().properties.transaction_isolation()
    }
    fn set_transaction_isolation(&mut self, level: i32) -> Result<(), RdbcError> {
        self.modify_properties(|properties| {
            properties.set_transaction_isolation(TransactionIsolation::from_jdbc(level))
        })
    }
    fn type_map(&self) -> Arc<Mutex<RdbcTypeMap>> {
        self.lock().properties.type_map()
    }
    fn set_type_map(&mut self, map: Arc<Mutex<RdbcTypeMap>>) -> Result<(), RdbcError> {
        self.modify_properties(|properties| properties.set_type_map(map))
    }
    fn command(&self) -> Option<String> {
        self.lock().properties.command().map(str::to_owned)
    }
    fn set_command(&mut self, command: Option<&str>) -> Result<(), RdbcError> {
        self.modify_properties(|properties| properties.set_command(command))
    }
    fn is_read_only(&self) -> bool {
        self.lock().properties.is_read_only()
    }
    fn set_read_only(&mut self, value: bool) -> Result<(), RdbcError> {
        self.modify_properties(|properties| properties.set_read_only(value))
    }
    fn max_field_size(&self) -> i32 {
        self.lock().properties.max_field_size()
    }
    fn set_max_field_size(&mut self, max: i32) -> Result<(), RdbcError> {
        self.modify_properties(|properties| properties.set_max_field_size(max))
    }
    fn max_rows(&self) -> i32 {
        self.lock().properties.max_rows()
    }
    fn set_max_rows(&mut self, max: i32) -> Result<(), RdbcError> {
        self.modify_properties(|properties| properties.set_max_rows(max))
    }
    fn escape_processing(&self) -> bool {
        self.lock().properties.escape_processing()
    }
    fn set_escape_processing(&mut self, enable: bool) -> Result<(), RdbcError> {
        self.modify_properties(|properties| properties.set_escape_processing(enable))
    }
    fn query_timeout(&self) -> i32 {
        self.lock().properties.query_timeout()
    }
    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.modify_properties(|properties| properties.set_query_timeout(seconds))
    }
    fn set_type(&mut self, type_code: i32) -> Result<(), RdbcError> {
        self.modify_properties(|properties| properties.set_type(type_code))
    }
    fn set_concurrency(&mut self, concurrency: i32) -> Result<(), RdbcError> {
        self.modify_properties(|properties| properties.set_concurrency(concurrency))
    }
}

impl Wrapper for SharedRowSet {
    fn wrapper_view(&self) -> &dyn Wrapper {
        self
    }
    fn canonical_unwrap(&self, target: TypeId) -> Option<StandardUnwrapped<'_>> {
        if target == TypeId::of::<dyn RowSet>() || target == TypeId::of::<Self>() {
            Some(StandardUnwrapped::RowSet(self))
        } else if target == TypeId::of::<dyn ResultSet>() {
            Some(StandardUnwrapped::ResultSet(self))
        } else {
            None
        }
    }
    fn canonical_unwrap_mut(&mut self, target: TypeId) -> Option<StandardUnwrappedMut<'_>> {
        if target == TypeId::of::<dyn Wrapper>() {
            Some(StandardUnwrappedMut::Wrapper(self))
        } else if target == TypeId::of::<dyn RowSet>() || target == TypeId::of::<Self>() {
            Some(StandardUnwrappedMut::RowSet(self))
        } else if target == TypeId::of::<dyn ResultSet>() {
            Some(StandardUnwrappedMut::ResultSet(self))
        } else {
            None
        }
    }
}

#[async_trait::async_trait]
impl ResultSet for SharedRowSet {
    fn known_row_count(&self) -> Option<u64> {
        self.ensure_open().ok()?;
        let cursor = self.cursor_handle();
        let count = cursor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .rows
            .visible_slots()
            .len();
        u64::try_from(count).ok()
    }
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.navigate(ResultSetNavigation::Next).await
    }
    async fn navigate(&mut self, navigation: ResultSetNavigation) -> Result<bool, RdbcError> {
        let present = {
            // 声明能力与状态准入在同一 owner 临界区校验，失败不改变游标或发送事件。
            let state = self.lock();
            if state.closed || (state.busy.is_some() && self.staged.is_none()) {
                return Err(RowSetCursor::error(
                    "RowSet cursor cannot move in this state",
                ));
            }
            if state.properties.result_set_type() == ResultSetType::ForwardOnly
                && navigation != ResultSetNavigation::Next
            {
                return Err(RowSetCursor::error("Forward-only RowSet cannot scroll"));
            }
            let handle = self
                .staged
                .clone()
                .unwrap_or_else(|| Arc::clone(&state.cursor));
            let mut cursor = handle
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if cursor.insert.is_some() {
                return Err(RowSetCursor::error("Cannot navigate from insert row"));
            }
            let len = cursor.rows.visible_slots().len() as i64;
            let next = match navigation {
                ResultSetNavigation::Next => cursor.cursor.saturating_add(1),
                ResultSetNavigation::Previous => cursor.cursor.saturating_sub(1),
                ResultSetNavigation::First => 0,
                ResultSetNavigation::Last => len - 1,
                ResultSetNavigation::BeforeFirst | ResultSetNavigation::Absolute(0) => -1,
                ResultSetNavigation::AfterLast => len,
                ResultSetNavigation::Absolute(row) if row > 0 => i64::from(row) - 1,
                ResultSetNavigation::Absolute(row) => len.saturating_add(i64::from(row)),
                ResultSetNavigation::Relative(rows) => {
                    cursor.cursor.saturating_add(i64::from(rows))
                }
            };
            cursor.cursor = next.clamp(-1, len);
            cursor.was_null = false;
            cursor.cursor >= 0 && cursor.cursor < len
        };
        self.notify('C');
        Ok(present)
    }
    fn row(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        let handle = self.cursor_handle();
        let cursor = handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if cursor.current_slot().is_ok() {
            i32::try_from(cursor.cursor + 1)
                .map_err(|_| RowSetCursor::error("RowSet row exceeds i32"))
        } else {
            Ok(0)
        }
    }
    fn object(&self, column: ColumnIndex) -> Result<RdbcObject, RdbcError> {
        self.ensure_open()?;
        let handle = self.cursor_handle();
        let mut cursor = handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let value = cursor.rows.value(cursor.current_slot()?, column)?;
        cursor.was_null = value.is_null();
        Ok(value)
    }
    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        Ok(match self.object(column)? {
            RdbcObject::Scalar(value) => value,
            RdbcObject::Boolean(value) => Value::Bool(value),
            RdbcObject::Byte(value) => Value::Int(i64::from(value)),
            RdbcObject::Short(value) => Value::Int(i64::from(value)),
            RdbcObject::Integer(value) => Value::Int(i64::from(value)),
            RdbcObject::Long(value) => Value::Int(value),
            RdbcObject::Float(value) => Value::Float(f64::from(value)),
            RdbcObject::Double(value) => Value::Float(value),
            RdbcObject::BigDecimal(value) => Value::Decimal(value),
            RdbcObject::Date(value) => Value::Date(value),
            RdbcObject::Time(value) => Value::Time(value),
            RdbcObject::Timestamp(value) => Value::Timestamp(value),
            RdbcObject::String(value) | RdbcObject::NString(value) => Value::String(value),
            RdbcObject::Bytes(value) => Value::Bytes(value),
            _ => {
                return Err(RowSetCursor::error(
                    "Rich RowSet value requires object getter",
                ))
            }
        })
    }
    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        let meta = self.row_set_meta_data()?;
        for index in 1..=meta.column_count() {
            let column = ColumnIndex::new(index)?;
            if meta.column_label(column)?.eq_ignore_ascii_case(label) {
                return Ok(column);
            }
        }
        Err(RowSetCursor::error("Unknown RowSet column label"))
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(Arc::new(self.row_set_meta_data()?))
    }
    fn was_null(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let handle = self.cursor_handle();
        let value = handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .was_null;
        Ok(value)
    }
    fn reader(&self, column: ColumnIndex, national: bool) -> Result<Option<RdbcReader>, RdbcError> {
        match self.object(column)? {
            value if value.is_null() => Ok(None),
            RdbcObject::CharacterStream(reader) if !national => Ok(Some(reader)),
            RdbcObject::NCharacterStream(reader) => Ok(Some(reader)),
            RdbcObject::String(value) | RdbcObject::NString(value) => {
                Ok(Some(RdbcReader::from_string(value)))
            }
            _ => Err(RowSetCursor::error("RowSet column is not character data")),
        }
    }
    fn input_stream(
        &self,
        column: ColumnIndex,
        kind: ResultSetStreamKind,
    ) -> Result<Option<RdbcInputStream>, RdbcError> {
        match self.object(column)? {
            value if value.is_null() => Ok(None),
            RdbcObject::Bytes(bytes) | RdbcObject::Scalar(Value::Bytes(bytes)) => {
                Ok(Some(RdbcInputStream::from_bytes(bytes)))
            }
            RdbcObject::String(value) | RdbcObject::NString(value) => {
                let bytes = match kind {
                    ResultSetStreamKind::Unicode => {
                        value.encode_utf16().flat_map(u16::to_be_bytes).collect()
                    }
                    _ => value.into_bytes(),
                };
                Ok(Some(RdbcInputStream::from_bytes(bytes)))
            }
            _ => Err(RowSetCursor::error("RowSet column is not byte stream data")),
        }
    }
    fn update_value(
        &mut self,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.ensure_update()?;
        let value = Self::update_object(update)?;
        let handle = self.cursor_handle();
        let mut cursor = handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if column.get() > cursor.metadata.column_count() {
            return Err(RowSetCursor::error("Invalid RowSet column index"));
        }
        if let Some(insert) = &mut cursor.insert {
            insert.insert(column.get(), value);
            Ok(())
        } else {
            let slot = cursor.current_slot()?;
            cursor.rows.stage_update(slot, column, value)
        }
    }
    fn update_value_by_label(
        &mut self,
        label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.update_value(self.find_column(label)?, update)
    }
    fn move_to_insert_row(&mut self) -> Result<(), RdbcError> {
        self.ensure_update()?;
        let handle = self.cursor_handle();
        let mut cursor = handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if cursor.insert.is_none() {
            cursor.saved_cursor = cursor.cursor;
        }
        cursor.insert = Some(BTreeMap::new());
        Ok(())
    }
    fn move_to_current_row(&mut self) -> Result<(), RdbcError> {
        // 状态检查与游标修改共用 owner 锁，阻止外部 clone 在两者之间开始执行。
        let state = self.lock();
        if state.closed || (state.busy.is_some() && self.staged.is_none()) {
            return Err(RowSetCursor::error(
                "RowSet cursor cannot move in this state",
            ));
        }
        let handle = self
            .staged
            .clone()
            .unwrap_or_else(|| Arc::clone(&state.cursor));
        let mut cursor = handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let insert = cursor.insert.take();
        if insert.is_some() {
            cursor.cursor = cursor.saved_cursor;
        }
        // 参数对象的最后一个引用可能执行外部析构；先释放全部内部锁。
        drop(cursor);
        drop(state);
        drop(insert);
        Ok(())
    }
    fn insert_row(&mut self) -> Result<(), RdbcError> {
        self.ensure_update()?;
        {
            let handle = self.cursor_handle();
            let mut cursor = handle
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let insert = cursor
                .insert
                .as_ref()
                .ok_or_else(|| RowSetCursor::error("RowSet cursor is not on insert row"))?;
            let width = cursor.metadata.column_count();
            let mut values = Vec::new();
            values
                .try_reserve_exact(width)
                .map_err(|_| RowSetCursor::error("RowSet row allocation failed"))?;
            for index in 1..=width {
                values.push(
                    insert
                        .get(&index)
                        .cloned()
                        .unwrap_or(RdbcObject::Scalar(Value::Null)),
                );
            }
            cursor.rows.insert_row(values)?;
            cursor.insert = Some(BTreeMap::new());
        }
        self.notify('R');
        Ok(())
    }
    fn update_row(&mut self) -> Result<(), RdbcError> {
        self.ensure_update()?;
        {
            let handle = self.cursor_handle();
            let mut cursor = handle
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let slot = cursor.current_slot()?;
            cursor.rows.update_row(slot)?;
        }
        self.notify('R');
        Ok(())
    }
    fn delete_row(&mut self) -> Result<(), RdbcError> {
        self.ensure_update()?;
        {
            let handle = self.cursor_handle();
            let mut cursor = handle
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let slot = cursor.current_slot()?;
            cursor.rows.delete_row(slot)?;
        }
        self.notify('R');
        Ok(())
    }
    fn cancel_row_updates(&mut self) -> Result<(), RdbcError> {
        self.ensure_update()?;
        let handle = self.cursor_handle();
        let mut cursor = handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let slot = cursor.current_slot()?;
        cursor.rows.cancel_row_updates(slot)
    }
    fn row_updated(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let handle = self.cursor_handle();
        let cursor = handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(cursor.rows.row_flags(cursor.current_slot()?)?.0)
    }
    fn row_inserted(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let handle = self.cursor_handle();
        let cursor = handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(cursor.rows.row_flags(cursor.current_slot()?)?.1)
    }
    fn row_deleted(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let handle = self.cursor_handle();
        let cursor = handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cursor.rows.is_deleted(cursor.current_slot()?)
    }
    fn is_before_first(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let handle = self.cursor_handle();
        let cursor = handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(cursor.insert.is_none() && !cursor.rows.visible_slots().is_empty() && cursor.cursor < 0)
    }
    fn is_after_last(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let handle = self.cursor_handle();
        let cursor = handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let len = cursor.rows.visible_slots().len();
        Ok(cursor.insert.is_none() && len > 0 && cursor.cursor >= len as i64)
    }
    fn is_first(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        Ok(self.row()? == 1)
    }
    fn is_last(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let row = self.row()?;
        Ok(row > 0 && u64::try_from(row).ok() == self.known_row_count())
    }
    fn result_set_type(&self) -> Result<ResultSetType, RdbcError> {
        self.ensure_open()?;
        Ok(self.lock().properties.result_set_type())
    }
    fn concurrency(&self) -> Result<ResultSetConcurrency, RdbcError> {
        self.ensure_open()?;
        Ok(self.lock().properties.concurrency())
    }
    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        self.ensure_open()?;
        Ok(ResultSetHoldability::HoldOverCommit)
    }
    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        self.ensure_open()?;
        let handle = self.cursor_handle();
        let value = handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fetch_direction;
        Ok(value)
    }
    fn set_fetch_direction(&mut self, direction: FetchDirection) -> Result<(), RdbcError> {
        // 对应 Java ResultSet#setFetchDirection：前向类型仅接受 FETCH_FORWARD。
        let state = self.lock();
        if state.closed || state.busy.is_some() {
            return Err(RowSetCursor::error(
                "RowSet fetch direction cannot change in this state",
            ));
        }
        if state.properties.result_set_type() == ResultSetType::ForwardOnly
            && direction != FetchDirection::Forward
        {
            return Err(RowSetCursor::error(
                "Forward-only RowSet requires forward fetch direction",
            ));
        }
        let handle = self
            .staged
            .clone()
            .unwrap_or_else(|| Arc::clone(&state.cursor));
        handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fetch_direction = direction;
        Ok(())
    }
    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        let handle = self.cursor_handle();
        let value = handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fetch_size;
        Ok(value)
    }
    fn set_fetch_size(&mut self, rows: i32) -> Result<(), RdbcError> {
        self.ensure_idle()?;
        if rows < 0 {
            return Err(RowSetCursor::error("Negative fetch size"));
        }
        let handle = self.cursor_handle();
        handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fetch_size = rows;
        Ok(())
    }
    fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        self.ensure_open()?;
        Ok(None)
    }
    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()
    }
    fn cursor_name(&self) -> Result<Option<String>, RdbcError> {
        self.ensure_open()?;
        Ok(None)
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        let mut state = self.lock();
        if !state.closed {
            state.closed = true;
            state.listeners.clear();
            state.reader = None;
            state.writer = None;
            state.parameters.clear_parameters();
        }
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.lock().closed)
    }
}
