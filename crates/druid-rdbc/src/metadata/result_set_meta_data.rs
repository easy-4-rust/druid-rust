use crate::error::RdbcError;
use crate::wrapper::Wrapper;
use std::any::Any;

/// JDBC column nullability descriptor。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ColumnNullability {
    /// column 不允许 SQL NULL。
    NoNulls,
    /// column 允许 SQL NULL。
    Nullable,
    /// 驱动无法确定。
    Unknown,
}

/// JDBC RowId 生命周期。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RowIdLifetime {
    /// 驱动不支持 RowId。
    Unsupported,
    /// RowId 仅在 transaction 内有效。
    Transaction,
    /// RowId 仅在 session 内有效。
    Session,
    /// RowId 永久有效。
    Forever,
}

/// 一个 immutable result-set column snapshot。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDescriptor {
    label: String,
    type_name: String,
    nullable: ColumnNullability,
}

/// Result-set column type descriptor。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResultSetColumnType {
    /// JDBC numeric type code。
    Jdbc(i32),
    /// Vendor-specific named type。
    Named(String),
}

/// Canonical name for one immutable result-set column snapshot。
pub type ResultSetColumnMeta = ColumnDescriptor;

/// Canonical name for result-set nullability。
pub type ResultSetNullability = ColumnNullability;

/// Immutable owned result-set metadata snapshot。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResultSetMetaDataSnapshot {
    columns: Vec<ColumnDescriptor>,
}

impl ResultSetMetaDataSnapshot {
    /// 创建 immutable metadata snapshot。
    pub fn new(columns: Vec<ColumnDescriptor>) -> Self {
        Self { columns }
    }

    /// 返回全部 column descriptors。
    pub fn columns(&self) -> &[ColumnDescriptor] {
        &self.columns
    }
}

impl ResultSetMetaData for ResultSetMetaDataSnapshot {
    fn column_count(&self) -> usize {
        self.columns.len()
    }

    fn column(&self, column_index: usize) -> Result<ColumnDescriptor, RdbcError> {
        let offset = column_index
            .checked_sub(1)
            .ok_or_else(|| RdbcError::InvalidArgument("column index is 1-based".to_string()))?;
        self.columns.get(offset).cloned().ok_or_else(|| {
            RdbcError::InvalidArgument(format!(
                "column index {column_index} exceeds metadata width"
            ))
        })
    }
}

impl Wrapper for ResultSetMetaDataSnapshot {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ColumnDescriptor {
    /// 创建 column snapshot。
    pub fn new(
        label: impl Into<String>,
        type_name: impl Into<String>,
        nullable: ColumnNullability,
    ) -> Self {
        Self {
            label: label.into(),
            type_name: type_name.into(),
            nullable,
        }
    }

    /// 返回 column label。
    pub fn label(&self) -> &str {
        &self.label
    }

    /// 返回驱动 SQL type name。
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// 返回 nullability。
    pub const fn nullability(&self) -> ColumnNullability {
        self.nullable
    }
}

/// JDBC `ResultSetMetaData` 的 immutable owned snapshot 标准。
pub trait ResultSetMetaData: Wrapper + Send + Sync + 'static {
    /// 返回 column 数量。
    fn column_count(&self) -> usize;

    /// 返回一个 1-based column descriptor。
    fn column(&self, _column_index: usize) -> Result<ColumnDescriptor, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_meta_data_column",
        })
    }
}
