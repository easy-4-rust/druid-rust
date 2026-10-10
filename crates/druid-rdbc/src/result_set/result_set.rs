use crate::error::{RdbcError, SqlWarning};
use crate::metadata::ResultSetMetaData;
use crate::result_set::{ResultSetRead, ResultSetUpdate};
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
    /// 返回创建此结果的标准语句；无创建语句时返回 None。
    /// 对应 Java: java.sql.ResultSet#getStatement。默认实现明确报告不支持。
    fn get_statement(&self) -> Result<Option<Box<dyn crate::statement::Statement>>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_get_statement",
        })
    }

    /// 返回已知的完整结果行数；未知或关闭时返回 `None`。
    ///
    /// Rust 扩展：支持执行摘要保留缓冲查询行数。不得请求数据库、消费后续行或
    /// 改变游标位置；此值不是 JDBC update count。默认流式实现不预先计算总行数。
    fn known_row_count(&self) -> Option<u64> {
        None
    }

    /// 推进到下一行；存在当前行时返回 `true`。
    async fn next(&mut self) -> Result<bool, RdbcError>;

    /// 执行 typed cursor navigation；默认拒绝保留原操作名和 Java 默认异常类别。
    /// 对应 Java: java.sql.ResultSet 导航方法；驱动可覆盖并报告自身 SQL 能力错误。
    async fn navigate(&mut self, navigation: ResultSetNavigation) -> Result<bool, RdbcError> {
        match navigation {
            ResultSetNavigation::Next => self.next().await,
            ResultSetNavigation::Previous => Err(RdbcError::UnsupportedOperation {
                operation: "result_set_previous",
            }),
            ResultSetNavigation::First => Err(RdbcError::UnsupportedOperation {
                operation: "result_set_first",
            }),
            ResultSetNavigation::Last => Err(RdbcError::UnsupportedOperation {
                operation: "result_set_last",
            }),
            ResultSetNavigation::BeforeFirst => Err(RdbcError::UnsupportedOperation {
                operation: "result_set_before_first",
            }),
            ResultSetNavigation::AfterLast => Err(RdbcError::UnsupportedOperation {
                operation: "result_set_after_last",
            }),
            ResultSetNavigation::Absolute(_) => Err(RdbcError::UnsupportedOperation {
                operation: "result_set_absolute",
            }),
            ResultSetNavigation::Relative(_) => Err(RdbcError::UnsupportedOperation {
                operation: "result_set_relative",
            }),
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

    /// 读取 getString 对应的原始标量；参数为 1-based 列，返回 String 或 SQL NULL。
    /// 默认实现复用值读取并拒绝其他类型；适配器可覆盖驱动原生字符串转换。
    /// 对应 Java: java.sql.ResultSet#getString(int)。
    fn string_value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        match self.value(column)? {
            value @ (Value::Null | Value::String(_)) => Ok(value),
            other => Err(super::result_set_ext::result_set_type_error(
                "String",
                &RdbcObject::Scalar(other),
            )),
        }
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

    /// 按列和读取描述符分派强类型 getter，返回保留资源身份的对象。
    /// 默认实现保留 BASE 标量转换；已解码日期时间不再重新解释时区。
    /// 原生驱动覆盖此入口后可使用完整 Calendar 和 scale 参数。
    /// 对应 Java: java.sql.ResultSet#getBigDecimal/getDate/getTime/getTimestamp/getBlob 等。
    fn read_object(
        &self,
        column: ColumnIndex,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        default_read_object(self, column, request)
    }

    /// 按原列名和描述符分派读取；参数为列名和重载请求，返回对象或错误。
    /// 默认实现解析列名，原生驱动可覆盖以保留列名入口身份。
    /// 对应 Java: java.sql.ResultSet#getXxx(String, ...)。
    fn read_object_by_label(
        &self,
        label: &str,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        self.read_object(self.find_column(label)?, request)
    }

    /// 使用可空 type map 读取 object。
    fn object_with_type_map(
        &self,
        _column: ColumnIndex,
        _type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_get_object_with_type_map",
        })
    }

    /// 按原列名使用可空类型映射读取对象；参数为列名和映射，返回驱动对象或错误。
    /// 默认不查列而明确报告不支持。对应 Java: java.sql.ResultSet#getObject(String, Map)。
    fn object_with_type_map_by_label(
        &self,
        _label: &str,
        _type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_get_object_by_label_with_type_map",
        })
    }

    /// 按原列名和目标类型读取对象；参数为列名和目标，返回驱动对象或错误。
    /// 默认仅解析一次列，驱动可覆盖列名入口。对应 Java: java.sql.ResultSet#getObject(String, Class)。
    fn object_as_by_label(
        &self,
        label: &str,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        self.object_as(self.find_column(label)?, target_type)
    }

    /// 使用目标类型读取当前列；参数为列和目标类型，返回转换对象或驱动错误。
    /// 资源分派到对应 getter；自定义目标先读取列一次，保留驱动错误优先级。
    /// 对应 Java: java.sql.ResultSet#getObject(int, Class)。
    fn object_as(
        &self,
        column: ColumnIndex,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        default_object_as(self, column, target_type)
    }

    /// 默认拒绝对应字节流能力，不预读列值或改变原错误优先级。
    ///
    /// 对应 Java: ResultSet#getAsciiStream/getUnicodeStream/getBinaryStream。
    /// 参数为列下标和流类型；支持流的驱动覆盖并返回自身资源。
    fn input_stream(
        &self,
        _column: ColumnIndex,
        kind: ResultSetStreamKind,
    ) -> Result<Option<RdbcInputStream>, RdbcError> {
        let operation = match kind {
            ResultSetStreamKind::Ascii => "result_set_ascii_stream",
            ResultSetStreamKind::Unicode => "result_set_unicode_stream",
            ResultSetStreamKind::Binary => "result_set_binary_stream",
        };
        Err(RdbcError::UnsupportedOperation { operation })
    }

    /// 按原列名读取字节流；参数为标签及流类型，返回驱动流或定位错误。
    /// 默认只定位一次并委托同一标准下标入口，原生驱动可覆盖标签重载。
    /// 对应 Java: ResultSet#getAsciiStream/getUnicodeStream/getBinaryStream(String)。
    fn input_stream_by_label(
        &self,
        label: &str,
        kind: ResultSetStreamKind,
    ) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.input_stream(self.find_column(label)?, kind)
    }

    /// 默认拒绝对应字符流能力，不预读列值。
    ///
    /// 对应 Java: ResultSet#getCharacterStream/getNCharacterStream。
    /// 参数为列下标及国家字符标志；支持 Reader 的驱动覆盖并返回自身资源。
    fn reader(
        &self,
        _column: ColumnIndex,
        national: bool,
    ) -> Result<Option<RdbcReader>, RdbcError> {
        let operation = if national {
            "result_set_n_character_stream"
        } else {
            "result_set_character_stream"
        };
        Err(RdbcError::UnsupportedOperation { operation })
    }

    /// 按原列名读取字符流；参数保留标签及国家字符标志，返回原Reader或错误。
    /// 默认只定位一次，不预读Reader。对应 Java: ResultSet#getCharacterStream/getNCharacterStream(String)。
    fn reader_by_label(
        &self,
        label: &str,
        national: bool,
    ) -> Result<Option<RdbcReader>, RdbcError> {
        self.reader(self.find_column(label)?, national)
    }

    /// 默认拒绝单列更新并保留操作名；驱动可覆盖并返回自身 SQL 能力错误。
    fn update_value(
        &mut self,
        _column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: update
                .unsupported_lob_stream_operation(false)
                .unwrap_or("result_set_update_value"),
        })
    }

    /// 默认按原列名直接拒绝更新，不查列或读取调用者资源。
    /// 参数为列名和更新描述符，返回保留标签重载名的精确不支持错误。
    /// 对应 Java: java.sql.ResultSet#updateXxx(String, ...)；有更新能力的驱动自行覆盖。
    fn update_value_by_label(
        &mut self,
        _column_label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: update
                .unsupported_lob_stream_operation(true)
                .unwrap_or("result_set_update_value_by_label"),
        })
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
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_type",
        })
    }

    /// 返回 cursor concurrency。
    fn concurrency(&self) -> Result<ResultSetConcurrency, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_concurrency",
        })
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
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_row_updated",
        })
    }

    /// 返回当前 row 是否为插入行。
    fn row_inserted(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_row_inserted",
        })
    }

    /// 返回当前 row 是否已删除。
    fn row_deleted(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_row_deleted",
        })
    }

    /// 插入 staged row。
    fn insert_row(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_insert_row",
        })
    }

    /// 更新当前 row。
    fn update_row(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_update_row",
        })
    }

    /// 删除当前 row。
    fn delete_row(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_delete_row",
        })
    }

    /// 刷新当前 row。
    fn refresh_row(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_refresh_row",
        })
    }

    /// 取消当前 row updates。
    fn cancel_row_updates(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_cancel_row_updates",
        })
    }

    /// 移动到 insert-row staging area。
    fn move_to_insert_row(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_move_to_insert_row",
        })
    }

    /// 从 insert-row staging area 返回 current row。
    fn move_to_current_row(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_move_to_current_row",
        })
    }

    /// 交接唯一拥有的游标以执行非阻塞释放；无参数、无返回值。
    /// 默认直接丢弃驱动对象，沿用驱动自身的 Drop；池化代理应把异步关闭
    /// 提交给已有所有者监管队列，保留失败与重试，不在 Drop 中阻塞等待。
    /// 仅 owning Stream 的所有权适配调用，不改变普通共享视图的生命周期。
    /// 对应 Java: java.sql.ResultSet#close 的 Rust 拥有型消费扩展。
    fn release_owned(self: Box<Self>) {
        drop(self);
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

/// 复用标准类型化对象转换；参数为结果集、1-based 列和目标，返回对象或准确驱动错误。
/// 对应 Java: java.sql.ResultSet#getObject(int, Class)。适配器可先执行自身状态校验，
/// 再复用此默认分派；不会回调 object_as，也不改变原生 getter 的资源身份。
pub fn default_object_as<T: ResultSet + ?Sized>(
    result_set: &T,
    column: ColumnIndex,
    target_type: &RdbcTargetType,
) -> Result<RdbcObject, RdbcError> {
    use super::result_set_ext::{typed_scalar_object, ResultSetExt};
    let null = RdbcObject::Scalar(Value::Null);
    match target_type {
        RdbcTargetType::Blob => result_set
            .blob(column)
            .map(|value| value.map_or(null, RdbcObject::Blob)),
        RdbcTargetType::Clob => result_set
            .clob(column)
            .map(|value| value.map_or(null, RdbcObject::Clob)),
        RdbcTargetType::NClob => result_set
            .n_clob(column)
            .map(|value| value.map_or(null, RdbcObject::NClob)),
        RdbcTargetType::Array => result_set
            .array(column)
            .map(|value| value.map_or(null, RdbcObject::Array)),
        RdbcTargetType::Ref => result_set
            .reference(column)
            .map(|value| value.map_or(null, RdbcObject::Ref)),
        RdbcTargetType::RowId => result_set
            .row_id(column)
            .map(|value| value.map_or(null, RdbcObject::RowId)),
        RdbcTargetType::SqlXml => result_set
            .sql_xml(column)
            .map(|value| value.map_or(null, RdbcObject::SqlXml)),
        RdbcTargetType::Url => result_set
            .url(column)
            .map(|value| value.map_or(null, RdbcObject::Url)),
        RdbcTargetType::Custom(_) => {
            // 只读取一次，关闭、列或驱动错误优先于自定义目标不支持。
            let _ = result_set.value(column)?;
            Err(RdbcError::UnsupportedOperation {
                operation: "result_set_get_object_typed_custom",
            })
        }
        _ => typed_scalar_object(result_set.value(column)?, target_type),
    }
}

/// 复用标准默认读取分派，保持严格标量类型与日期、scale、资源合同。
/// 参数为结果集、1-based 列与读取描述符；返回对象或原始错误。
/// 对应 Java: java.sql.ResultSet#getXxx；缓冲实现可只覆盖原生标量转换。
pub(crate) fn default_read_object<T: ResultSet + ?Sized>(
    result_set: &T,
    column: ColumnIndex,
    request: &ResultSetRead,
) -> Result<RdbcObject, RdbcError> {
    use super::result_set_ext::{
        date_object, decimal_object, n_string_object, resource_object, time_object,
        timestamp_object,
    };
    // 标量默认读取保持既有 value/string_value 路径，避免转调 object 改变错误优先级。
    if std::matches!(request, ResultSetRead::String) {
        return result_set.string_value(column).map(RdbcObject::Scalar);
    }
    if std::matches!(
        request,
        ResultSetRead::Boolean
            | ResultSetRead::Long
            | ResultSetRead::Int
            | ResultSetRead::Short
            | ResultSetRead::Byte
            | ResultSetRead::Double
            | ResultSetRead::Float
            | ResultSetRead::Bytes
    ) {
        return result_set.value(column).map(RdbcObject::Scalar);
    }
    let object = result_set.object(column)?;
    match request {
        // national 字符串继续使用 object 默认合同，不能借用 getString 的标量转换。
        ResultSetRead::NString => n_string_object(object)
            .map(|value| value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::NString)),
        ResultSetRead::BigDecimal(scale) => decimal_object(object).map(|value| {
            value.map_or(RdbcObject::Scalar(Value::Null), |value| {
                RdbcObject::BigDecimal(match scale {
                    Some(scale) => value.with_scale(i64::from(*scale)),
                    None => value,
                })
            })
        }),
        ResultSetRead::Date(_) => date_object(object)
            .map(|value| value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::Date)),
        ResultSetRead::Time(_) => time_object(object)
            .map(|value| value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::Time)),
        ResultSetRead::Timestamp(_) => timestamp_object(object)
            .map(|value| value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::Timestamp)),
        _ => resource_object(object, request),
    }
}
