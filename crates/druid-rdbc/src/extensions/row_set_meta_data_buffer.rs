use crate::error::{RdbcError, SqlException};
use crate::extensions::row_set_column_state::RowSetColumnState;
use crate::extensions::RowSetMetaData;
use crate::metadata::{ColumnDescriptor, ColumnNullability, ResultSetMetaData};
use crate::result_set::ColumnIndex;
use crate::types::JdbcType;
use std::collections::BTreeMap;

/// RowSetMetaData的稀疏内存实现；设置count重置列属性，快照不共享可变状态。
/// 对应 Java: javax.sql.RowSetMetaData（Rust缓冲扩展，不宣称RowSetMetaDataImpl具体类完全等价）。
/// 初始flags为false、可空性Unknown、名称为空；类型代码默认NULL，未知应用类为Object。
/// 稀疏存储只为实际修改的列分配状态，支持完整正i32列数；不是结果数据的可写性认证。
#[derive(Debug, Default)]
pub struct RowSetMetaDataBuffer {
    column_count: usize,
    columns: BTreeMap<usize, RowSetColumnState>,
}
impl RowSetMetaDataBuffer {
    /// 创建无列的元数据，无参数；返回可通过set_column_count配置的对象。
    pub fn new() -> Self {
        Self::default()
    }
    /// 创建列元数据参数错误，不泄漏业务列值。
    fn sql_error(message: &'static str) -> RdbcError {
        RdbcError::SqlException(Box::new(SqlException::new(0, None, Some(message.into()))))
    }
    /// 检查1-based列范围；越界返回SQL错误，不建立新列。
    fn check_column(&self, column_index: ColumnIndex) -> Result<usize, RdbcError> {
        let index = column_index.get();
        if index > self.column_count {
            return Err(Self::sql_error("Invalid column index"));
        }
        Ok(index)
    }
    /// 获取有效列的独占可变状态；仅在范围校验后分配默认列。
    fn column_mut(
        &mut self,
        column_index: ColumnIndex,
    ) -> Result<&mut RowSetColumnState, RdbcError> {
        let index = self.check_column(column_index)?;
        Ok(self.columns.entry(index).or_default())
    }
}
impl ResultSetMetaData for RowSetMetaDataBuffer {
    fn column_count(&self) -> usize {
        self.column_count
    }
    fn column(&self, column_index: ColumnIndex) -> Result<ColumnDescriptor, RdbcError> {
        let index = self.check_column(column_index)?;
        Ok(self
            .columns
            .get(&index)
            .cloned()
            .unwrap_or_default()
            .snapshot())
    }
}
crate::impl_standard_wrapper!(RowSetMetaDataBuffer => RowSetMetaData);
impl RowSetMetaData for RowSetMetaDataBuffer {
    fn set_column_count(&mut self, column_count: i32) -> Result<(), RdbcError> {
        if column_count <= 0 {
            return Err(Self::sql_error("Invalid column count"));
        }
        self.column_count = column_count as usize;
        self.columns.clear();
        Ok(())
    }
    fn set_auto_increment(
        &mut self,
        column_index: ColumnIndex,
        property: bool,
    ) -> Result<(), RdbcError> {
        self.column_mut(column_index)?.auto_increment = property;
        Ok(())
    }
    fn set_case_sensitive(
        &mut self,
        column_index: ColumnIndex,
        property: bool,
    ) -> Result<(), RdbcError> {
        self.column_mut(column_index)?.case_sensitive = property;
        Ok(())
    }
    fn set_searchable(
        &mut self,
        column_index: ColumnIndex,
        property: bool,
    ) -> Result<(), RdbcError> {
        self.column_mut(column_index)?.searchable = property;
        Ok(())
    }
    fn set_currency(&mut self, column_index: ColumnIndex, property: bool) -> Result<(), RdbcError> {
        self.column_mut(column_index)?.currency = property;
        Ok(())
    }
    fn set_signed(&mut self, column_index: ColumnIndex, property: bool) -> Result<(), RdbcError> {
        self.column_mut(column_index)?.signed = property;
        Ok(())
    }
    fn set_nullable(&mut self, column_index: ColumnIndex, property: i32) -> Result<(), RdbcError> {
        let nullable = match property {
            0 => ColumnNullability::NoNulls,
            1 => ColumnNullability::Nullable,
            2 => ColumnNullability::Unknown,
            _ => return Err(Self::sql_error("Invalid nullable constant")),
        };
        self.column_mut(column_index)?.nullable = nullable;
        Ok(())
    }
    fn set_column_display_size(
        &mut self,
        column_index: ColumnIndex,
        value: i32,
    ) -> Result<(), RdbcError> {
        let value =
            usize::try_from(value).map_err(|_| Self::sql_error("Negative column dimension"))?;
        self.column_mut(column_index)?.column_display_size = value;
        Ok(())
    }
    fn set_precision(&mut self, column_index: ColumnIndex, value: i32) -> Result<(), RdbcError> {
        let value =
            usize::try_from(value).map_err(|_| Self::sql_error("Negative column dimension"))?;
        self.column_mut(column_index)?.precision = value;
        Ok(())
    }
    fn set_scale(&mut self, column_index: ColumnIndex, value: i32) -> Result<(), RdbcError> {
        let value =
            usize::try_from(value).map_err(|_| Self::sql_error("Negative column dimension"))?;
        self.column_mut(column_index)?.scale = value;
        Ok(())
    }
    fn set_column_label(
        &mut self,
        column_index: ColumnIndex,
        label: Option<String>,
    ) -> Result<(), RdbcError> {
        self.column_mut(column_index)?.column_label = label.unwrap_or_default();
        Ok(())
    }
    fn set_column_name(
        &mut self,
        column_index: ColumnIndex,
        column_name: Option<String>,
    ) -> Result<(), RdbcError> {
        self.column_mut(column_index)?.column_name = column_name.unwrap_or_default();
        Ok(())
    }
    fn set_schema_name(
        &mut self,
        column_index: ColumnIndex,
        schema_name: Option<String>,
    ) -> Result<(), RdbcError> {
        self.column_mut(column_index)?.schema_name = schema_name.unwrap_or_default();
        Ok(())
    }
    fn set_table_name(
        &mut self,
        column_index: ColumnIndex,
        table_name: Option<String>,
    ) -> Result<(), RdbcError> {
        self.column_mut(column_index)?.table_name = table_name.unwrap_or_default();
        Ok(())
    }
    fn set_catalog_name(
        &mut self,
        column_index: ColumnIndex,
        catalog_name: Option<String>,
    ) -> Result<(), RdbcError> {
        self.column_mut(column_index)?.catalog_name = catalog_name.unwrap_or_default();
        Ok(())
    }
    fn set_column_type_name(
        &mut self,
        column_index: ColumnIndex,
        type_name: Option<String>,
    ) -> Result<(), RdbcError> {
        self.column_mut(column_index)?.column_type_name = type_name.unwrap_or_default();
        Ok(())
    }
    fn set_column_type(
        &mut self,
        column_index: ColumnIndex,
        sql_type: i32,
    ) -> Result<(), RdbcError> {
        if JdbcType::value_of(sql_type).is_err() {
            return Err(Self::sql_error("Invalid SQL type for column"));
        }
        self.column_mut(column_index)?.sql_type = sql_type;
        Ok(())
    }
}
