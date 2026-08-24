use crate::error::RdbcError;
use crate::metadata::ResultSetMetaData;
use crate::types::Value;
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

/// Result-set update operation descriptor。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ResultSetUpdate {
    /// 更新一个 column value。
    Column {
        /// 1-based column index。
        index: ColumnIndex,
        /// 新 SQL value。
        value: Value,
    },
    /// 插入当前 staged row。
    InsertRow,
    /// 删除当前 row。
    DeleteRow,
}

/// JDBC `ResultSet` 的唯一 owned mutable cursor 标准。
///
/// 对应 Java：`java.sql.ResultSet`。对象由 `Box<dyn ResultSet>` 唯一持有；
/// [`ResultSet::next`] 通过 `&mut self` 推进，metadata 通过 owned [`Arc`] snapshot 返回。
#[async_trait::async_trait]
pub trait ResultSet: Wrapper + Send + 'static {
    /// 推进到下一行；存在当前行时返回 `true`。
    async fn next(&mut self) -> Result<bool, RdbcError>;

    /// 读取当前行的 SQL 值。
    fn value(&self, _column: ColumnIndex) -> Result<Value, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_value",
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

    /// 关闭游标并释放其 owned actor/native handle。
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    /// 返回游标是否已经关闭。
    fn is_closed(&self) -> bool {
        false
    }
}
