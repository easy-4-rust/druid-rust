//! RDBC `ResultSetMetaData` 公共句柄。
//!
//! 对应 Java：`java.sql.ResultSetMetaData`。eager Adapter 使用列 descriptor，
//! 真实 driver 使用 `PhysicalResultSetMetaData` 逐方法委托并保留 Wrapper 身份。

use super::{
    DruidError, PhysicalResultSetMetaData, ResultSetColumnMeta, ResultSetColumnType,
    ResultSetNullability, Unwrapped, Wrapper,
};
use std::any::{Any, TypeId};
use std::fmt;
use std::sync::Arc;

enum ResultSetMetaDataBackend {
    Columns(Vec<ResultSetColumnMeta>),
    Physical(Arc<dyn PhysicalResultSetMetaData>),
}

/// 查询结果的列 metadata。
pub struct ResultSetMetaData {
    backend: ResultSetMetaDataBackend,
    mysql8_datetime_compatibility: bool,
}

impl ResultSetMetaData {
    /// 使用稳定列 descriptor 创建 eager metadata。
    pub fn new(columns: Vec<ResultSetColumnMeta>) -> Self {
        Self {
            backend: ResultSetMetaDataBackend::Columns(columns),
            mysql8_datetime_compatibility: false,
        }
    }

    /// 包装真实 driver metadata。
    pub fn from_physical(physical: Arc<dyn PhysicalResultSetMetaData>) -> Self {
        Self {
            backend: ResultSetMetaDataBackend::Physical(physical),
            mysql8_datetime_compatibility: false,
        }
    }

    /// 启用 `MySQL` Connector/J 8.0.24 的 DATETIME 类名兼容。
    ///
    /// 仅由 canonical `MySQL8DateTimeResultSetMetaData` 使用；其余 metadata
    /// 委托、物理 Wrapper 身份和列属性保持不变。
    pub(crate) fn with_mysql8_datetime_compatibility(mut self) -> Self {
        self.mysql8_datetime_compatibility = true;
        self
    }

    /// 返回物理 metadata SPI；eager descriptor 返回 `None`。
    pub fn physical(&self) -> Option<&dyn PhysicalResultSetMetaData> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => None,
            ResultSetMetaDataBackend::Physical(physical) => Some(physical.as_ref()),
        }
    }

    /// 返回列数。对应 Java：`ResultSetMetaData#getColumnCount()`。
    pub fn column_count(&self) -> Result<usize, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(columns) => Ok(columns.len()),
            ResultSetMetaDataBackend::Physical(physical) => physical.column_count(),
        }
    }

    /// 返回 1-based 列标签。
    pub fn column_label(&self, column_index: usize) -> Result<String, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => Ok(self.column(column_index)?.label.clone()),
            ResultSetMetaDataBackend::Physical(physical) => physical.column_label(column_index),
        }
    }

    /// 返回 1-based 列类型。
    pub fn column_type(&self, column_index: usize) -> Result<ResultSetColumnType, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => Ok(self.column(column_index)?.column_type),
            ResultSetMetaDataBackend::Physical(physical) => physical.column_type(column_index),
        }
    }

    /// 返回列是否允许 SQL NULL。
    pub fn is_nullable(&self, column_index: usize) -> Result<bool, DruidError> {
        Ok(matches!(
            self.nullability(column_index)?,
            ResultSetNullability::Nullable
        ))
    }

    /// 返回 Java 三态可空性。
    pub fn nullability(&self, column_index: usize) -> Result<ResultSetNullability, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => Ok(self.column(column_index)?.nullability),
            ResultSetMetaDataBackend::Physical(physical) => physical.nullability(column_index),
        }
    }

    /// 返回 Java `columnNoNulls/columnNullable/columnNullableUnknown` 数值。
    pub fn nullable_code(&self, column_index: usize) -> Result<i32, DruidError> {
        Ok(self.nullability(column_index)?.rdbc_code())
    }

    /// 返回列是否自动递增。
    pub fn is_auto_increment(&self, column_index: usize) -> Result<bool, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => Ok(self.column(column_index)?.auto_increment),
            ResultSetMetaDataBackend::Physical(physical) => {
                physical.is_auto_increment(column_index)
            }
        }
    }

    /// 返回列是否区分大小写。
    pub fn is_case_sensitive(&self, column_index: usize) -> Result<bool, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => Ok(self.column(column_index)?.case_sensitive),
            ResultSetMetaDataBackend::Physical(physical) => {
                physical.is_case_sensitive(column_index)
            }
        }
    }

    /// 返回列是否可用于 WHERE 搜索。
    pub fn is_searchable(&self, column_index: usize) -> Result<bool, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => Ok(self.column(column_index)?.searchable),
            ResultSetMetaDataBackend::Physical(physical) => physical.is_searchable(column_index),
        }
    }

    /// 返回列是否为货币值。
    pub fn is_currency(&self, column_index: usize) -> Result<bool, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => Ok(self.column(column_index)?.currency),
            ResultSetMetaDataBackend::Physical(physical) => physical.is_currency(column_index),
        }
    }

    /// 返回数值列是否有符号。
    pub fn is_signed(&self, column_index: usize) -> Result<bool, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => Ok(self.column(column_index)?.signed),
            ResultSetMetaDataBackend::Physical(physical) => physical.is_signed(column_index),
        }
    }

    /// 返回最大显示字符数；驱动未知时为零。
    pub fn column_display_size(&self, column_index: usize) -> Result<usize, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => Ok(self.column(column_index)?.display_size),
            ResultSetMetaDataBackend::Physical(physical) => {
                physical.column_display_size(column_index)
            }
        }
    }

    /// 返回底层列名。
    pub fn column_name(&self, column_index: usize) -> Result<String, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => Ok(self.column(column_index)?.name.clone()),
            ResultSetMetaDataBackend::Physical(physical) => physical.column_name(column_index),
        }
    }

    /// 返回 schema 名；驱动未知时为空字符串。
    pub fn schema_name(&self, column_index: usize) -> Result<String, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => {
                Ok(self.column(column_index)?.schema_name.clone())
            }
            ResultSetMetaDataBackend::Physical(physical) => physical.schema_name(column_index),
        }
    }

    /// 返回数值精度；驱动未知时为零。
    pub fn precision(&self, column_index: usize) -> Result<usize, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => Ok(self.column(column_index)?.precision),
            ResultSetMetaDataBackend::Physical(physical) => physical.precision(column_index),
        }
    }

    /// 返回小数位数；驱动未知时为零。
    pub fn scale(&self, column_index: usize) -> Result<usize, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => Ok(self.column(column_index)?.scale),
            ResultSetMetaDataBackend::Physical(physical) => physical.scale(column_index),
        }
    }

    /// 返回表名；驱动未知时为空字符串。
    pub fn table_name(&self, column_index: usize) -> Result<String, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => {
                Ok(self.column(column_index)?.table_name.clone())
            }
            ResultSetMetaDataBackend::Physical(physical) => physical.table_name(column_index),
        }
    }

    /// 返回 catalog 名；驱动未知时为空字符串。
    pub fn catalog_name(&self, column_index: usize) -> Result<String, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => {
                Ok(self.column(column_index)?.catalog_name.clone())
            }
            ResultSetMetaDataBackend::Physical(physical) => physical.catalog_name(column_index),
        }
    }

    /// 返回 `java.sql.Types` 数值。
    pub fn rdbc_type(&self, column_index: usize) -> Result<i32, DruidError> {
        Ok(self.column_type(column_index)?.rdbc_type())
    }

    /// 返回数据库类型名。
    pub fn column_type_name(&self, column_index: usize) -> Result<String, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => {
                Ok(self.column(column_index)?.type_name.clone())
            }
            ResultSetMetaDataBackend::Physical(physical) => physical.column_type_name(column_index),
        }
    }

    /// 返回列是否只读。
    pub fn is_read_only(&self, column_index: usize) -> Result<bool, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => Ok(self.column(column_index)?.read_only),
            ResultSetMetaDataBackend::Physical(physical) => physical.is_read_only(column_index),
        }
    }

    /// 返回列是否可写。
    pub fn is_writable(&self, column_index: usize) -> Result<bool, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => Ok(self.column(column_index)?.writable),
            ResultSetMetaDataBackend::Physical(physical) => physical.is_writable(column_index),
        }
    }

    /// 返回列是否确定可写。
    pub fn is_definitely_writable(&self, column_index: usize) -> Result<bool, DruidError> {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => {
                Ok(self.column(column_index)?.definitely_writable)
            }
            ResultSetMetaDataBackend::Physical(physical) => {
                physical.is_definitely_writable(column_index)
            }
        }
    }

    /// 返回 Java typed getter 对应类名。
    pub fn column_class_name(&self, column_index: usize) -> Result<String, DruidError> {
        let class_name = match &self.backend {
            ResultSetMetaDataBackend::Columns(_) => self.column(column_index)?.class_name.clone(),
            ResultSetMetaDataBackend::Physical(physical) => {
                physical.column_class_name(column_index)?
            }
        };
        if self.mysql8_datetime_compatibility && class_name == "java.time.LocalDateTime" {
            Ok("java.sql.Timestamp".to_owned())
        } else {
            Ok(class_name)
        }
    }

    /// 对应 Java `getColumnCount`。
    pub fn get_column_count(&self) -> Result<usize, DruidError> {
        self.column_count()
    }
    /// 对应 Java `getColumnLabel`。
    pub fn get_column_label(&self, column_index: usize) -> Result<String, DruidError> {
        self.column_label(column_index)
    }
    /// 对应 Java `getColumnName`。
    pub fn get_column_name(&self, column_index: usize) -> Result<String, DruidError> {
        self.column_name(column_index)
    }
    /// 对应 Java `getColumnType`。
    pub fn get_column_type(&self, column_index: usize) -> Result<i32, DruidError> {
        self.rdbc_type(column_index)
    }
    /// 对应 Java `getColumnTypeName`。
    pub fn get_column_type_name(&self, column_index: usize) -> Result<String, DruidError> {
        self.column_type_name(column_index)
    }
    /// 对应 Java `getColumnClassName`。
    pub fn get_column_class_name(&self, column_index: usize) -> Result<String, DruidError> {
        self.column_class_name(column_index)
    }
    /// 对应 Java `getSchemaName`。
    pub fn get_schema_name(&self, column_index: usize) -> Result<String, DruidError> {
        self.schema_name(column_index)
    }
    /// 对应 Java `getTableName`。
    pub fn get_table_name(&self, column_index: usize) -> Result<String, DruidError> {
        self.table_name(column_index)
    }
    /// 对应 Java `getCatalogName`。
    pub fn get_catalog_name(&self, column_index: usize) -> Result<String, DruidError> {
        self.catalog_name(column_index)
    }
    /// 对应 Java `getPrecision`。
    pub fn get_precision(&self, column_index: usize) -> Result<usize, DruidError> {
        self.precision(column_index)
    }
    /// 对应 Java `getScale`。
    pub fn get_scale(&self, column_index: usize) -> Result<usize, DruidError> {
        self.scale(column_index)
    }
    /// 对应 Java `getColumnDisplaySize`。
    pub fn get_column_display_size(&self, column_index: usize) -> Result<usize, DruidError> {
        self.column_display_size(column_index)
    }

    fn column(&self, column_index: usize) -> Result<&ResultSetColumnMeta, DruidError> {
        let ResultSetMetaDataBackend::Columns(columns) = &self.backend else {
            return Err(DruidError::Other(
                "physical metadata does not expose an eager column descriptor".to_string(),
            ));
        };
        let index = column_index
            .checked_sub(1)
            .ok_or_else(|| DruidError::InvalidArgument("column_index is 1-based".to_string()))?;
        columns.get(index).ok_or_else(|| {
            DruidError::InvalidArgument(format!(
                "column_index {column_index} exceeds metadata width"
            ))
        })
    }
}

impl Wrapper for ResultSetMetaData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_instance_of(&self, iface: TypeId) -> bool {
        iface == TypeId::of::<Self>()
            || match &self.backend {
                ResultSetMetaDataBackend::Columns(_) => false,
                ResultSetMetaDataBackend::Physical(physical) => {
                    iface == TypeId::of::<dyn PhysicalResultSetMetaData>()
                        || physical.is_instance_of(iface)
                }
            }
    }

    fn unwrap(&self, iface: Option<TypeId>) -> Option<Unwrapped<'_>> {
        let iface = iface?;
        if iface == TypeId::of::<Self>() {
            return Some(Unwrapped::Object(self));
        }
        let ResultSetMetaDataBackend::Physical(physical) = &self.backend else {
            return None;
        };
        if iface == TypeId::of::<dyn PhysicalResultSetMetaData>() {
            return Some(Unwrapped::ResultSetMetaData(physical.as_ref()));
        }
        physical.unwrap(Some(iface))
    }
}

impl Clone for ResultSetMetaData {
    fn clone(&self) -> Self {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(columns) => Self::new(columns.clone()),
            ResultSetMetaDataBackend::Physical(physical) => Self::from_physical(physical.clone()),
        }
        .with_mysql8_datetime_compatibility_if(self.mysql8_datetime_compatibility)
    }
}

impl ResultSetMetaData {
    fn with_mysql8_datetime_compatibility_if(mut self, enabled: bool) -> Self {
        self.mysql8_datetime_compatibility = enabled;
        self
    }
}

#[allow(clippy::missing_fields_in_debug)]
impl fmt::Debug for ResultSetMetaData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.backend {
            ResultSetMetaDataBackend::Columns(columns) => formatter
                .debug_struct("ResultSetMetaData")
                .field("columns", columns)
                .finish(),
            ResultSetMetaDataBackend::Physical(physical) => formatter
                .debug_struct("ResultSetMetaData")
                .field("physical", physical)
                .finish(),
        }
    }
}

impl PartialEq for ResultSetMetaData {
    fn eq(&self, other: &Self) -> bool {
        (match (&self.backend, &other.backend) {
            (ResultSetMetaDataBackend::Columns(left), ResultSetMetaDataBackend::Columns(right)) => {
                left == right
            }
            (
                ResultSetMetaDataBackend::Physical(left),
                ResultSetMetaDataBackend::Physical(right),
            ) => Arc::ptr_eq(left, right),
            (ResultSetMetaDataBackend::Columns(_), ResultSetMetaDataBackend::Physical(_))
            | (ResultSetMetaDataBackend::Physical(_), ResultSetMetaDataBackend::Columns(_)) => {
                false
            }
        }) && self.mysql8_datetime_compatibility == other.mysql8_datetime_compatibility
    }
}

impl Eq for ResultSetMetaData {}

impl Default for ResultSetMetaData {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}
