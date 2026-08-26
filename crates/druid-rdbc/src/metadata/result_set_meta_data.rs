use crate::error::RdbcError;
use crate::result_set::ColumnIndex;
use crate::wrapper::Wrapper;

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
    /// RowId 生命周期不属于 transaction/session/forever 三类。
    ValidOther,
    /// RowId 仅在 transaction 内有效。
    Transaction,
    /// RowId 仅在 session 内有效。
    Session,
    /// RowId 永久有效。
    Forever,
}

impl RowIdLifetime {
    /// 返回 JDBC 4.2 enum constant name。
    #[must_use]
    pub const fn jdbc_name(self) -> &'static str {
        match self {
            Self::Unsupported => "ROWID_UNSUPPORTED",
            Self::ValidOther => "ROWID_VALID_OTHER",
            Self::Transaction => "ROWID_VALID_TRANSACTION",
            Self::Session => "ROWID_VALID_SESSION",
            Self::Forever => "ROWID_VALID_FOREVER",
        }
    }
}

impl TryFrom<&str> for RowIdLifetime {
    type Error = RdbcError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "ROWID_UNSUPPORTED" => Ok(Self::Unsupported),
            "ROWID_VALID_OTHER" => Ok(Self::ValidOther),
            "ROWID_VALID_TRANSACTION" => Ok(Self::Transaction),
            "ROWID_VALID_SESSION" => Ok(Self::Session),
            "ROWID_VALID_FOREVER" => Ok(Self::Forever),
            _ => Err(RdbcError::InvalidArgument(format!(
                "unknown JDBC RowIdLifetime: {value}"
            ))),
        }
    }
}

/// 一个 immutable result-set column snapshot。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDescriptor {
    label: String,
    name: String,
    column_type: ResultSetColumnType,
    type_name: String,
    nullable: ColumnNullability,
    auto_increment: bool,
    case_sensitive: bool,
    searchable: bool,
    currency: bool,
    signed: bool,
    display_size: usize,
    schema_name: String,
    precision: usize,
    scale: usize,
    table_name: String,
    catalog_name: String,
    read_only: bool,
    writable: bool,
    definitely_writable: bool,
    class_name: String,
}

/// Result-set column type descriptor。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResultSetColumnType {
    /// 驱动无法确定。
    Unknown,
    /// Boolean。
    Boolean,
    /// Integral numeric。
    Integer,
    /// Floating-point numeric。
    Float,
    /// Exact decimal numeric。
    Decimal,
    /// Date。
    Date,
    /// Time。
    Time,
    /// Timestamp。
    Timestamp,
    /// Character data。
    Text,
    /// Binary data。
    Binary,
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

    fn column(&self, column_index: ColumnIndex) -> Result<ColumnDescriptor, RdbcError> {
        let offset = column_index
            .get()
            .checked_sub(1)
            .ok_or_else(|| RdbcError::InvalidArgument("column index is 1-based".to_string()))?;
        self.columns.get(offset).cloned().ok_or_else(|| {
            RdbcError::InvalidArgument(format!(
                "column index {} exceeds metadata width",
                column_index.get()
            ))
        })
    }
}

crate::impl_standard_wrapper!(ResultSetMetaDataSnapshot => ResultSetMetaData);

impl ColumnDescriptor {
    /// 创建 column snapshot。
    pub fn new(
        label: impl Into<String>,
        type_name: impl Into<String>,
        nullable: ColumnNullability,
    ) -> Self {
        let label = label.into();
        let type_name = type_name.into();
        let column_type = column_type_from_name(&type_name);
        Self {
            name: label.clone(),
            label,
            column_type,
            type_name,
            nullable,
            auto_increment: false,
            case_sensitive: false,
            searchable: true,
            currency: false,
            signed: false,
            display_size: 0,
            schema_name: String::new(),
            precision: 0,
            scale: 0,
            table_name: String::new(),
            catalog_name: String::new(),
            read_only: true,
            writable: false,
            definitely_writable: false,
            class_name: String::new(),
        }
    }

    /// 设置 driver column name 与 type。
    pub fn with_driver_type(
        mut self,
        name: impl Into<String>,
        column_type: ResultSetColumnType,
    ) -> Self {
        self.name = name.into();
        self.column_type = column_type;
        self
    }

    /// 设置 precision、scale 与 display size。
    pub const fn with_dimensions(
        mut self,
        precision: usize,
        scale: usize,
        display_size: usize,
    ) -> Self {
        self.precision = precision;
        self.scale = scale;
        self.display_size = display_size;
        self
    }

    /// 设置 catalog/schema/table origin。
    pub fn with_origin(
        mut self,
        catalog_name: impl Into<String>,
        schema_name: impl Into<String>,
        table_name: impl Into<String>,
    ) -> Self {
        self.catalog_name = catalog_name.into();
        self.schema_name = schema_name.into();
        self.table_name = table_name.into();
        self
    }

    /// 设置常用 driver flags。
    pub const fn with_flags(
        mut self,
        auto_increment: bool,
        case_sensitive: bool,
        searchable: bool,
        currency: bool,
        signed: bool,
    ) -> Self {
        self.auto_increment = auto_increment;
        self.case_sensitive = case_sensitive;
        self.searchable = searchable;
        self.currency = currency;
        self.signed = signed;
        self
    }

    /// 设置 read/write flags。
    pub const fn with_writability(
        mut self,
        read_only: bool,
        writable: bool,
        definitely_writable: bool,
    ) -> Self {
        self.read_only = read_only;
        self.writable = writable;
        self.definitely_writable = definitely_writable;
        self
    }

    /// 设置 driver-reported column class name。
    pub fn with_class_name(mut self, class_name: impl Into<String>) -> Self {
        self.class_name = class_name.into();
        self
    }

    /// 返回 column label。
    pub fn label(&self) -> &str {
        &self.label
    }

    /// 返回驱动 SQL type name。
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// 返回 driver column name。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 返回 typed column kind。
    pub fn column_type(&self) -> &ResultSetColumnType {
        &self.column_type
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
    fn column(&self, _column_index: ColumnIndex) -> Result<ColumnDescriptor, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_meta_data_column",
        })
    }

    /// 返回 column 是否 auto-increment。
    fn is_auto_increment(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.column(column).map(|value| value.auto_increment)
    }

    /// 返回 column 是否区分大小写。
    fn is_case_sensitive(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.column(column).map(|value| value.case_sensitive)
    }

    /// 返回 column 是否 searchable。
    fn is_searchable(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.column(column).map(|value| value.searchable)
    }

    /// 返回 column 是否 currency。
    fn is_currency(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.column(column).map(|value| value.currency)
    }

    /// 返回 nullability。
    fn nullability(&self, column: ColumnIndex) -> Result<ColumnNullability, RdbcError> {
        self.column(column).map(|value| value.nullable)
    }

    /// 返回 numeric column 是否 signed。
    fn is_signed(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.column(column).map(|value| value.signed)
    }

    /// 返回 display size。
    fn column_display_size(&self, column: ColumnIndex) -> Result<usize, RdbcError> {
        self.column(column).map(|value| value.display_size)
    }

    /// 返回 column label。
    fn column_label(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.column(column).map(|value| value.label)
    }

    /// 返回 driver column name。
    fn column_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.column(column).map(|value| value.name)
    }

    /// 返回 schema name。
    fn schema_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.column(column).map(|value| value.schema_name)
    }

    /// 返回 precision。
    fn precision(&self, column: ColumnIndex) -> Result<usize, RdbcError> {
        self.column(column).map(|value| value.precision)
    }

    /// 返回 scale。
    fn scale(&self, column: ColumnIndex) -> Result<usize, RdbcError> {
        self.column(column).map(|value| value.scale)
    }

    /// 返回 table name。
    fn table_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.column(column).map(|value| value.table_name)
    }

    /// 返回 catalog name。
    fn catalog_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.column(column).map(|value| value.catalog_name)
    }

    /// 返回 typed column kind。
    fn column_type(&self, column: ColumnIndex) -> Result<ResultSetColumnType, RdbcError> {
        self.column(column).map(|value| value.column_type)
    }

    /// 返回 driver type name。
    fn column_type_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.column(column).map(|value| value.type_name)
    }

    /// 返回 column 是否 read-only。
    fn is_read_only(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.column(column).map(|value| value.read_only)
    }

    /// 返回 column 是否 writable。
    fn is_writable(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.column(column).map(|value| value.writable)
    }

    /// 返回 column 是否 definitely writable。
    fn is_definitely_writable(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.column(column).map(|value| value.definitely_writable)
    }

    /// 返回 driver column class name。
    fn column_class_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.column(column).map(|value| value.class_name)
    }
}

fn column_type_from_name(type_name: &str) -> ResultSetColumnType {
    match type_name.to_ascii_uppercase().as_str() {
        "BOOLEAN" | "BOOL" => ResultSetColumnType::Boolean,
        "TINYINT" | "SMALLINT" | "INTEGER" | "INT" | "BIGINT" => ResultSetColumnType::Integer,
        "REAL" | "FLOAT" | "DOUBLE" | "DOUBLE PRECISION" => ResultSetColumnType::Float,
        "NUMERIC" | "DECIMAL" => ResultSetColumnType::Decimal,
        "DATE" => ResultSetColumnType::Date,
        "TIME" => ResultSetColumnType::Time,
        "TIMESTAMP" | "DATETIME" => ResultSetColumnType::Timestamp,
        "CHAR" | "VARCHAR" | "TEXT" => ResultSetColumnType::Text,
        "BINARY" | "VARBINARY" | "BLOB" | "BYTEA" => ResultSetColumnType::Binary,
        "NULL" | "UNKNOWN" | "" => ResultSetColumnType::Unknown,
        other => ResultSetColumnType::Named(other.to_owned()),
    }
}
