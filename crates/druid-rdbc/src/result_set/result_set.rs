use crate::error::{RdbcError, SqlWarning};
use crate::metadata::ResultSetMetaData;
use crate::result_set::ResultSetUpdate;
use crate::statement::{FetchDirection, ResultSetConcurrency, ResultSetHoldability, ResultSetType};
use crate::types::{RdbcInputStream, RdbcObject, RdbcReader, RdbcTargetType, RdbcTypeMap, Value};
use crate::wrapper::Wrapper;
use std::sync::Arc;

/// JDBC 1-based column index。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColumnIndex(usize);

impl ColumnIndex {
    /// 创建 1-based column index；`0` 返回 [`RdbcError::InvalidArgument`]。
    pub fn new(index: usize) -> Result<Self, RdbcError> {
        if index == 0 {
            return Err(RdbcError::InvalidArgument(
                "column index is 1-based".to_string(),
            ));
        }
        Ok(Self(index))
    }

    /// 返回原始 1-based 数值。
    pub const fn get(self) -> usize {
        self.0
    }
}

impl TryFrom<i64> for ColumnIndex {
    type Error = RdbcError;

    fn try_from(index: i64) -> Result<Self, Self::Error> {
        Self::try_from(i128::from(index))
    }
}

impl TryFrom<i128> for ColumnIndex {
    type Error = RdbcError;

    fn try_from(index: i128) -> Result<Self, Self::Error> {
        if index <= 0 {
            return Err(RdbcError::InvalidArgument(
                "column index is 1-based".to_string(),
            ));
        }
        let index = usize::try_from(index).map_err(|_| {
            RdbcError::InvalidArgument("column index exceeds platform usize".to_string())
        })?;
        Self::new(index)
    }
}

/// ResultSet cursor navigation request。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResultSetNavigation {
    /// 下一行。
    Next,
    /// 前一行。
    Previous,
    /// 第一行。
    First,
    /// 最后一行。
    Last,
    /// before-first position。
    BeforeFirst,
    /// after-last position。
    AfterLast,
    /// absolute 1-based/negative row position。
    Absolute(i32),
    /// 相对当前位置移动。
    Relative(i32),
}

/// ResultSet byte-stream getter family。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResultSetStreamKind {
    /// ASCII bytes。
    Ascii,
    /// Legacy Unicode stream bytes。
    Unicode,
    /// Raw binary bytes。
    Binary,
}

/// JDBC `ResultSet` 的唯一 owned mutable cursor 标准。
///
/// 对应 Java：`java.sql.ResultSet`。对象由 `Box<dyn ResultSet>` 唯一持有；
/// [`ResultSet::next`] 通过 `&mut self` 推进，metadata 通过 owned [`Arc`] snapshot 返回。
#[async_trait::async_trait]
pub trait ResultSet: Wrapper + Send + 'static {
    /// 推进到下一行；存在当前行时返回 `true`。
    async fn next(&mut self) -> Result<bool, RdbcError>;

    /// 执行 typed cursor navigation；forward-only adapter 对其他 variant fail closed。
    async fn navigate(&mut self, navigation: ResultSetNavigation) -> Result<bool, RdbcError> {
        match navigation {
            ResultSetNavigation::Next => self.next().await,
            _ => Err(RdbcError::feature_not_supported("result_set_navigation")),
        }
    }

    /// 移动到前一行。
    async fn previous(&mut self) -> Result<bool, RdbcError> {
        self.navigate(ResultSetNavigation::Previous).await
    }

    /// 移动到第一行。
    async fn first(&mut self) -> Result<bool, RdbcError> {
        self.navigate(ResultSetNavigation::First).await
    }

    /// 移动到最后一行。
    async fn last(&mut self) -> Result<bool, RdbcError> {
        self.navigate(ResultSetNavigation::Last).await
    }

    /// 移动到 before-first position。
    async fn before_first(&mut self) -> Result<(), RdbcError> {
        self.navigate(ResultSetNavigation::BeforeFirst)
            .await
            .map(|_| ())
    }

    /// 移动到 after-last position。
    async fn after_last(&mut self) -> Result<(), RdbcError> {
        self.navigate(ResultSetNavigation::AfterLast)
            .await
            .map(|_| ())
    }

    /// 移动到 absolute row。
    async fn absolute(&mut self, row: i32) -> Result<bool, RdbcError> {
        self.navigate(ResultSetNavigation::Absolute(row)).await
    }

    /// 相对当前位置移动。
    async fn relative(&mut self, rows: i32) -> Result<bool, RdbcError> {
        self.navigate(ResultSetNavigation::Relative(rows)).await
    }

    /// 返回当前 JDBC 1-based row；不在 row 上时为 0。
    fn row(&self) -> Result<i32, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_row",
        })
    }

    /// 读取当前行的 SQL 值。
    fn value(&self, _column: ColumnIndex) -> Result<Value, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_value",
        })
    }

    /// 按 column label 读取 SQL value。
    fn value_by_label(&self, column_label: &str) -> Result<Value, RdbcError> {
        self.value(self.find_column(column_label)?)
    }

    /// 查找 label 的首个 1-based column index。
    fn find_column(&self, _column_label: &str) -> Result<ColumnIndex, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_find_column",
        })
    }

    /// 读取通用 object；标量默认保持无损 [`RdbcObject::Scalar`]。
    fn object(&self, column: ColumnIndex) -> Result<RdbcObject, RdbcError> {
        self.value(column).map(RdbcObject::from)
    }

    /// 按 label 读取通用 object。
    fn object_by_label(&self, column_label: &str) -> Result<RdbcObject, RdbcError> {
        self.object(self.find_column(column_label)?)
    }

    /// 使用可空 type map 读取 object。
    fn object_with_type_map(
        &self,
        _column: ColumnIndex,
        _type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_object_with_type_map",
        })
    }

    /// 使用 target type 读取 object。
    fn object_as(
        &self,
        _column: ColumnIndex,
        _target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_object_as",
        })
    }

    /// 从当前 canonical value 派生 byte stream；SQL NULL 返回 `None`。
    ///
    /// `Bytes` 保持原始字节，其余标量按 JDBC 字符表示编码为 UTF-8。真实 LOB
    /// adapter 可覆盖此方法，以保留 lazy stream 和 native handle 生命周期。
    fn input_stream(
        &self,
        column: ColumnIndex,
        _kind: ResultSetStreamKind,
    ) -> Result<Option<RdbcInputStream>, RdbcError> {
        let bytes = match self.value(column)? {
            Value::Null => return Ok(None),
            Value::Bytes(value) => value,
            Value::String(value) => value.into_bytes(),
            Value::Bool(value) => value.to_string().into_bytes(),
            Value::Int(value) => value.to_string().into_bytes(),
            Value::Float(value) => value.to_string().into_bytes(),
            Value::Decimal(value) => value.to_string().into_bytes(),
            Value::Date(value) => value.to_string().into_bytes(),
            Value::Time(value) => value.to_string().into_bytes(),
            Value::Timestamp(value) => value.to_string().into_bytes(),
        };
        Ok(Some(RdbcInputStream::from_bytes(bytes)))
    }

    /// 从当前 canonical value 派生 character/national-character Reader。
    ///
    /// byte value 必须是有效 UTF-8；SQL NULL 返回 `None`。native CLOB adapter
    /// 可覆盖此方法，以保留自身 reader 生命周期。
    fn reader(
        &self,
        column: ColumnIndex,
        _national: bool,
    ) -> Result<Option<RdbcReader>, RdbcError> {
        let value = match self.value(column)? {
            Value::Null => return Ok(None),
            Value::Bytes(value) => String::from_utf8(value)
                .map_err(|error| RdbcError::DriverError(error.to_string()))?,
            Value::String(value) => value,
            Value::Bool(value) => value.to_string(),
            Value::Int(value) => value.to_string(),
            Value::Float(value) => value.to_string(),
            Value::Decimal(value) => value.to_string(),
            Value::Date(value) => value.to_string(),
            Value::Time(value) => value.to_string(),
            Value::Timestamp(value) => value.to_string(),
        };
        Ok(Some(RdbcReader::from_string(value)))
    }

    /// 暂存单列 update；read-only adapter 返回 `0A000`。
    fn update_value(
        &mut self,
        _column: ColumnIndex,
        _update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_update_value",
        })
    }

    /// 暂存 label-based 单列 update。
    fn update_value_by_label(
        &mut self,
        column_label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        let column = self.find_column(column_label)?;
        self.update_value(column, update)
    }

    /// 返回 immutable result-set metadata snapshot。
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_meta_data",
        })
    }

    /// 返回最近一次 primitive getter 是否读取到 SQL NULL。
    fn was_null(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_was_null",
        })
    }

    /// 返回 cursor 是否处于 before-first。
    fn is_before_first(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_is_before_first",
        })
    }

    /// 返回 cursor 是否处于 after-last。
    fn is_after_last(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_is_after_last",
        })
    }

    /// 返回 cursor 是否处于第一行。
    fn is_first(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_is_first",
        })
    }

    /// 返回 cursor 是否处于最后一行。
    fn is_last(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_is_last",
        })
    }

    /// 设置 fetch direction hint。
    fn set_fetch_direction(&mut self, _direction: FetchDirection) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_set_fetch_direction",
        })
    }

    /// 返回 fetch direction hint。
    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_fetch_direction",
        })
    }

    /// 设置 fetch size hint。
    fn set_fetch_size(&mut self, _rows: i32) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_set_fetch_size",
        })
    }

    /// 返回 fetch size hint。
    fn fetch_size(&self) -> Result<i32, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_fetch_size",
        })
    }

    /// 返回 cursor type。
    fn result_set_type(&self) -> Result<ResultSetType, RdbcError> {
        Ok(ResultSetType::ForwardOnly)
    }

    /// 返回 cursor concurrency。
    fn concurrency(&self) -> Result<ResultSetConcurrency, RdbcError> {
        Ok(ResultSetConcurrency::ReadOnly)
    }

    /// 返回 cursor holdability。
    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_holdability",
        })
    }

    /// 返回 warning chain。
    fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_warnings",
        })
    }

    /// 清理 warning chain。
    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_clear_warnings",
        })
    }

    /// 返回 cursor name。
    fn cursor_name(&self) -> Result<Option<String>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_cursor_name",
        })
    }

    /// 返回当前 row 是否已更新。
    fn row_updated(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    /// 返回当前 row 是否为插入行。
    fn row_inserted(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    /// 返回当前 row 是否已删除。
    fn row_deleted(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    /// 插入 staged row。
    fn insert_row(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("result_set_insert_row"))
    }

    /// 更新当前 row。
    fn update_row(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("result_set_update_row"))
    }

    /// 删除当前 row。
    fn delete_row(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("result_set_delete_row"))
    }

    /// 刷新当前 row。
    fn refresh_row(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("result_set_refresh_row"))
    }

    /// 取消当前 row updates。
    fn cancel_row_updates(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported(
            "result_set_cancel_row_updates",
        ))
    }

    /// 移动到 insert-row staging area。
    fn move_to_insert_row(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported(
            "result_set_move_to_insert_row",
        ))
    }

    /// 从 insert-row staging area 返回 current row。
    fn move_to_current_row(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported(
            "result_set_move_to_current_row",
        ))
    }

    /// 关闭游标并释放其 owned actor/native handle。
    async fn close(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("result_set_close"))
    }

    /// 返回游标是否已经关闭。
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::feature_not_supported("result_set_is_closed"))
    }
}
