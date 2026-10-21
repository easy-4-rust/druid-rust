use super::row_set_cursor::RowSetCursor;
use super::row_set_execution_guard::RowSetExecutionGuard;
use super::row_set_provider_context::RowSetProviderContext;
use super::row_set_state::RowSetState;
use super::{
    RowSet, RowSetEvent, RowSetInternal, RowSetListener, RowSetParameterResolver, RowSetProperties,
    RowSetReader, RowSetRowStore, RowSetWriter, SharedRowSetMetaData,
};
use crate::connection::{Connection, TransactionIsolation};
use crate::error::{RdbcError, SqlException, SqlWarning};
use crate::metadata::ResultSetMetaData;
use crate::result_set::{
    ColumnIndex, ResultSet, ResultSetNavigation, ResultSetStreamKind, ResultSetUpdate,
};
use crate::statement::{
    FetchDirection, ParameterIndex, PreparedInputParameter, ResultSetConcurrency,
    ResultSetHoldability, ResultSetType,
};
use crate::types::{RdbcInputStream, RdbcObject, RdbcReader, RdbcTypeMap, Value};
use crate::wrapper::{StandardUnwrapped, StandardUnwrappedMut, Wrapper};
use std::any::TypeId;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

mod trait_impls;

/// 真实可执行、可监听和可写回的共享 RowSet；clone 保留逻辑 source 身份。
/// 对应 Java: javax.sql.RowSet、RowSetInternal；具体实现是 Rust provider 扩展。
/// Reader 原子发布，Writer 显式成功才更新原始基线；不自动关闭借用连接。
#[derive(Clone)]
pub struct SharedRowSet {
    owner: Arc<Mutex<RowSetState>>,
    // provider 视图路由到本次暂存 cursor；外部 clone 仍观察 owner 的已发布 cursor。
    staged: Option<Arc<Mutex<RowSetCursor>>>,
}
impl Default for SharedRowSet {
    fn default() -> Self {
        Self::new()
    }
}
impl PartialEq for SharedRowSet {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.owner, &other.owner)
    }
}
impl Eq for SharedRowSet {}
impl std::fmt::Debug for SharedRowSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedRowSet")
            .field("closed", &self.lock().closed)
            .finish_non_exhaustive()
    }
}
impl SharedRowSet {
    /// 创建尚未配置 provider 的空行集；无参数，不建立数据库连接。
    pub fn new() -> Self {
        Self {
            owner: Arc::new(Mutex::new(RowSetState::new())),
            staged: None,
        }
    }
    fn lock(&self) -> MutexGuard<'_, RowSetState> {
        self.owner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn cursor_handle(&self) -> Arc<Mutex<RowSetCursor>> {
        self.staged
            .clone()
            .unwrap_or_else(|| Arc::clone(&self.lock().cursor))
    }
    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.lock().closed {
            Err(RowSetCursor::error("RowSet is closed"))
        } else {
            Ok(())
        }
    }
    fn ensure_idle(&self) -> Result<(), RdbcError> {
        let state = self.lock();
        if state.closed {
            Err(RowSetCursor::error("RowSet is closed"))
        } else if state.busy.is_some() {
            Err(RowSetCursor::error("RowSet is executing"))
        } else {
            Ok(())
        }
    }
    fn ensure_update(&self) -> Result<(), RdbcError> {
        let state = self.lock();
        if state.closed {
            return Err(RowSetCursor::error("RowSet is closed"));
        }
        if self.staged.is_some() {
            return Ok(());
        }
        if state.busy.is_some() {
            return Err(RowSetCursor::error("RowSet is executing"));
        }
        if state.properties.is_read_only()
            || state.properties.concurrency() != ResultSetConcurrency::Updatable
        {
            return Err(RowSetCursor::error("RowSet is read only"));
        }
        Ok(())
    }
    fn modify_properties<F>(&self, change: F) -> Result<(), RdbcError>
    where
        F: FnOnce(&mut RowSetProperties) -> Result<(), RdbcError>,
    {
        let mut state = self.lock();
        if state.closed || (state.busy.is_some() && self.staged.is_none()) {
            return Err(RowSetCursor::error(
                "RowSet properties cannot change in this state",
            ));
        }
        change(&mut state.properties)
    }
    /// 安装参数 reader，返回成功或关闭/执行中错误；不执行或消费 provider。
    pub fn set_reader(&mut self, reader: Option<Box<dyn RowSetReader>>) -> Result<(), RdbcError> {
        self.ensure_idle()?;
        self.lock().reader = reader.map(|reader| Arc::new(tokio::sync::Mutex::new(reader)));
        Ok(())
    }
    /// 安装参数 writer，返回成功或状态错误；不接受原始基线。
    pub fn set_writer(&mut self, writer: Option<Box<dyn RowSetWriter>>) -> Result<(), RdbcError> {
        self.ensure_idle()?;
        self.lock().writer = writer.map(|writer| Arc::new(tokio::sync::Mutex::new(writer)));
        Ok(())
    }
    /// 设置参数 resolver 的显式名称解析；返回成功或状态错误，不按字典顺序猜测位置。
    pub fn set_named_parameter_resolver(
        &mut self,
        resolver: Option<RowSetParameterResolver>,
    ) -> Result<(), RdbcError> {
        self.ensure_idle()?;
        self.lock().resolver = resolver;
        Ok(())
    }
    /// 返回已安装 metadata 的同一共享对象；无参数，不创建可变状态副本。
    pub fn row_set_meta_data(&self) -> Result<SharedRowSetMetaData, RdbcError> {
        self.ensure_open()?;
        let cursor = self.cursor_handle();
        let metadata = cursor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .metadata
            .clone();
        Ok(metadata)
    }
    /// 借用参数 connection 执行 Reader；返回执行结果，RowSet 不获得 close 权限。
    pub async fn execute_with_connection(
        &mut self,
        connection: &mut dyn Connection,
    ) -> Result<(), RdbcError> {
        self.execute_reader(Some(connection)).await
    }
    /// 调用 Writer 写回本地修改；无参数，true 时才接受新的原始行基线。
    pub async fn accept_changes(&mut self) -> Result<bool, RdbcError> {
        self.write_changes(None).await
    }
    /// 使用参数 connection 的借用执行 Writer；返回冲突/成功或原错误，不关闭连接。
    pub async fn accept_changes_with_connection(
        &mut self,
        connection: &mut dyn Connection,
    ) -> Result<bool, RdbcError> {
        self.write_changes(Some(connection)).await
    }
    fn begin(&self) -> Result<RowSetExecutionGuard, RdbcError> {
        let mut state = self.lock();
        if state.closed || state.busy.is_some() {
            return Err(RowSetCursor::error(
                "RowSet cannot recursively execute or execute after close",
            ));
        }
        state.next_operation = state
            .next_operation
            .checked_add(1)
            .ok_or_else(|| RowSetCursor::error("RowSet operation sequence exhausted"))?;
        let token = state.next_operation;
        state.busy = Some(token);
        Ok(RowSetExecutionGuard {
            owner: Arc::clone(&self.owner),
            token,
        })
    }
    fn timeout_error() -> RdbcError {
        RdbcError::SqlException(Box::new(
            SqlException::new(
                0,
                Some("HYT00".into()),
                Some("RowSet query timed out".into()),
            )
            .with_class_name("java.sql.SQLTimeoutException"),
        ))
    }
    async fn execute_reader(
        &mut self,
        connection: Option<&mut dyn Connection>,
    ) -> Result<(), RdbcError> {
        let guard = self.begin()?;
        let (reader, seconds, max_rows, max_field_size) = {
            let state = self.lock();
            (
                state
                    .reader
                    .clone()
                    .ok_or_else(|| RowSetCursor::error("RowSet has no reader provider"))?,
                state.properties.query_timeout(),
                state.properties.max_rows(),
                state.properties.max_field_size(),
            )
        };
        let staged = Arc::new(Mutex::new(RowSetCursor::new()));
        let source = Self {
            owner: Arc::clone(&self.owner),
            staged: Some(Arc::clone(&staged)),
        };
        let mut context = RowSetProviderContext { source, connection };
        // provider 锁仅保护可变 provider，绝不在 source 同步锁内等待它。
        let action = async { reader.lock().await.read_data(&mut context).await };
        if seconds > 0 {
            tokio::time::timeout(Duration::from_secs(seconds as u64), action)
                .await
                .map_err(|_| Self::timeout_error())??;
        } else {
            action.await?;
        }
        drop(context);
        {
            let mut cursor = staged
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cursor.rows.changes()?;
            let mut rows = cursor.rows.visible_rows();
            if max_rows > 0 {
                rows.truncate(max_rows as usize);
            }
            if max_field_size > 0 {
                for row in &mut rows {
                    for value in row {
                        Self::limit_value(value, max_field_size as usize);
                    }
                }
            }
            let mut store = RowSetRowStore::new(cursor.metadata.column_count());
            store.replace_rows(rows)?;
            cursor.rows = store;
            cursor.cursor = -1;
            cursor.insert = None;
        }
        {
            let mut state = self.lock();
            if state.closed || state.busy != Some(guard.token) {
                return Err(RowSetCursor::error("RowSet execution was invalidated"));
            }
            state.cursor = staged;
        }
        drop(guard);
        self.notify('S');
        Ok(())
    }
    async fn write_changes(
        &mut self,
        connection: Option<&mut dyn Connection>,
    ) -> Result<bool, RdbcError> {
        let guard = self.begin()?;
        let (writer, seconds) = {
            let state = self.lock();
            (
                state
                    .writer
                    .clone()
                    .ok_or_else(|| RowSetCursor::error("RowSet has no writer provider"))?,
                state.properties.query_timeout(),
            )
        };
        let cursor = self.cursor_handle();
        let snapshot = cursor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        snapshot.rows.changes()?;
        let staged = Arc::new(Mutex::new(snapshot));
        let source = Self {
            owner: Arc::clone(&self.owner),
            staged: Some(Arc::clone(&staged)),
        };
        let mut context = RowSetProviderContext { source, connection };
        let action = async { writer.lock().await.write_data(&mut context).await };
        let accepted = if seconds > 0 {
            tokio::time::timeout(Duration::from_secs(seconds as u64), action)
                .await
                .map_err(|_| Self::timeout_error())??
        } else {
            action.await?
        };
        drop(context);
        if accepted {
            staged
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .rows
                .accept_changes()?;
            let mut state = self.lock();
            if state.closed || state.busy != Some(guard.token) {
                return Err(RowSetCursor::error("RowSet writeback was invalidated"));
            }
            state.cursor = staged;
        }
        drop(guard);
        if accepted {
            self.notify('S');
        }
        Ok(accepted)
    }
    fn limit_value(value: &mut RdbcObject, limit: usize) {
        match value {
            RdbcObject::Bytes(bytes) | RdbcObject::Scalar(Value::Bytes(bytes)) => {
                bytes.truncate(limit);
            }
            RdbcObject::String(text)
            | RdbcObject::NString(text)
            | RdbcObject::Scalar(Value::String(text)) => {
                let mut end = limit.min(text.len());
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                text.truncate(end);
            }
            _ => {}
        }
    }
    fn notify(&self, kind: char) {
        if self.staged.is_some() {
            return;
        }
        let listeners = self.lock().listeners.clone();
        let event = RowSetEvent {
            source: self.clone(),
        };
        for listener in listeners {
            match kind {
                'S' => listener.row_set_changed(&event),
                'R' => listener.row_changed(&event),
                _ => listener.cursor_moved(&event),
            }
        }
    }
    fn original_result(
        &self,
        rows: Vec<Vec<RdbcObject>>,
        metadata: SharedRowSetMetaData,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        let result = Self::new();
        {
            let cursor = result.cursor_handle();
            let mut cursor = cursor
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cursor.rows = RowSetRowStore::new(metadata.column_count());
            cursor.rows.replace_rows(rows)?;
            cursor.metadata = metadata;
        }
        Ok(Box::new(result))
    }
    fn update_object(update: ResultSetUpdate) -> Result<RdbcObject, RdbcError> {
        let null = || RdbcObject::Scalar(Value::Null);
        Ok(match update {
            ResultSetUpdate::Null => null(),
            ResultSetUpdate::Boolean(value) => RdbcObject::Boolean(value),
            ResultSetUpdate::Byte(value) => RdbcObject::Byte(value),
            ResultSetUpdate::Short(value) => RdbcObject::Short(value),
            ResultSetUpdate::Int(value) => RdbcObject::Integer(value),
            ResultSetUpdate::Long(value) => RdbcObject::Long(value),
            ResultSetUpdate::Float(value) => RdbcObject::Float(value),
            ResultSetUpdate::Double(value) => RdbcObject::Double(value),
            ResultSetUpdate::String(value) => value.map(RdbcObject::String).unwrap_or_else(null),
            ResultSetUpdate::NString(value) => value.map(RdbcObject::NString).unwrap_or_else(null),
            ResultSetUpdate::Bytes(value) => value.map(RdbcObject::Bytes).unwrap_or_else(null),
            ResultSetUpdate::BigDecimal(value) => {
                value.map(RdbcObject::BigDecimal).unwrap_or_else(null)
            }
            ResultSetUpdate::Date(value) => value.map(RdbcObject::Date).unwrap_or_else(null),
            ResultSetUpdate::Time(value) => value.map(RdbcObject::Time).unwrap_or_else(null),
            ResultSetUpdate::Timestamp(value) => {
                value.map(RdbcObject::Timestamp).unwrap_or_else(null)
            }
            ResultSetUpdate::Object(value)
            | ResultSetUpdate::ObjectWithScaleOrLength { value, .. } => value,
            ResultSetUpdate::Ref(value) => value.map(RdbcObject::Ref).unwrap_or_else(null),
            ResultSetUpdate::Blob(value) => value.map(RdbcObject::Blob).unwrap_or_else(null),
            ResultSetUpdate::Clob(value) => value.map(RdbcObject::Clob).unwrap_or_else(null),
            ResultSetUpdate::NClob(value) => value.map(RdbcObject::NClob).unwrap_or_else(null),
            ResultSetUpdate::Array(value) => value.map(RdbcObject::Array).unwrap_or_else(null),
            ResultSetUpdate::RowId(value) => value.map(RdbcObject::RowId).unwrap_or_else(null),
            ResultSetUpdate::SqlXml(value) => value.map(RdbcObject::SqlXml).unwrap_or_else(null),
            ResultSetUpdate::CharacterStream { reader, .. } => {
                reader.map(RdbcObject::CharacterStream).unwrap_or_else(null)
            }
            ResultSetUpdate::NCharacterStream { reader, .. } => reader
                .map(RdbcObject::NCharacterStream)
                .unwrap_or_else(null),
            _ => {
                return Err(RdbcError::feature_not_supported(
                    "row_set_lob_stream_update",
                ))
            }
        })
    }
}
