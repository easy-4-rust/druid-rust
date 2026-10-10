use super::RowSetMetaData;
use crate::error::RdbcError;
use crate::metadata::{
    ColumnDescriptor, ColumnNullability, ResultSetColumnType, ResultSetMetaData,
};
use crate::result_set::ColumnIndex;
use std::sync::{Arc, Mutex, MutexGuard};

/// 保留同一安装对象身份的可变行集元数据共享代理。
/// 对应 Java: RowSetInternal#setMetaData 传入的 javax.sql.RowSetMetaData 引用。
#[derive(Clone)]
pub struct SharedRowSetMetaData {
    inner: Arc<Mutex<Box<dyn RowSetMetaData>>>,
}
impl SharedRowSetMetaData {
    /// 接管参数 metadata 的对象引用；返回共享代理，不读取或复制列定义。
    pub fn new(metadata: Box<dyn RowSetMetaData>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(metadata)),
        }
    }
    fn lock(&self) -> MutexGuard<'_, Box<dyn RowSetMetaData>> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
impl PartialEq for SharedRowSetMetaData {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}
impl Eq for SharedRowSetMetaData {}
impl std::fmt::Debug for SharedRowSetMetaData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedRowSetMetaData")
            .finish_non_exhaustive()
    }
}
crate::impl_standard_wrapper!(SharedRowSetMetaData => RowSetMetaData);
impl ResultSetMetaData for SharedRowSetMetaData {
    /// 委托同一底层对象的 column_count，保持参数、返回值及原错误；不创建元数据副本。
    fn column_count(&self) -> usize {
        self.lock().column_count()
    }
    /// 委托同一底层对象的 column，保持参数、返回值及原错误；不创建元数据副本。
    fn column(&self, column_index: ColumnIndex) -> Result<ColumnDescriptor, RdbcError> {
        self.lock().column(column_index)
    }
    /// 委托同一底层对象的 is_auto_increment，保持参数、返回值及原错误；不创建元数据副本。
    fn is_auto_increment(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.lock().is_auto_increment(column)
    }
    /// 委托同一底层对象的 is_case_sensitive，保持参数、返回值及原错误；不创建元数据副本。
    fn is_case_sensitive(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.lock().is_case_sensitive(column)
    }
    /// 委托同一底层对象的 is_searchable，保持参数、返回值及原错误；不创建元数据副本。
    fn is_searchable(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.lock().is_searchable(column)
    }
    /// 委托同一底层对象的 is_currency，保持参数、返回值及原错误；不创建元数据副本。
    fn is_currency(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.lock().is_currency(column)
    }
    /// 委托同一底层对象的 nullability，保持参数、返回值及原错误；不创建元数据副本。
    fn nullability(&self, column: ColumnIndex) -> Result<ColumnNullability, RdbcError> {
        self.lock().nullability(column)
    }
    /// 委托同一底层对象的 is_signed，保持参数、返回值及原错误；不创建元数据副本。
    fn is_signed(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.lock().is_signed(column)
    }
    /// 委托同一底层对象的 column_display_size，保持参数、返回值及原错误；不创建元数据副本。
    fn column_display_size(&self, column: ColumnIndex) -> Result<usize, RdbcError> {
        self.lock().column_display_size(column)
    }
    /// 委托同一底层对象的 column_label，保持参数、返回值及原错误；不创建元数据副本。
    fn column_label(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.lock().column_label(column)
    }
    /// 委托同一底层对象的 column_name，保持参数、返回值及原错误；不创建元数据副本。
    fn column_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.lock().column_name(column)
    }
    /// 委托同一底层对象的 schema_name，保持参数、返回值及原错误；不创建元数据副本。
    fn schema_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.lock().schema_name(column)
    }
    /// 委托同一底层对象的 precision，保持参数、返回值及原错误；不创建元数据副本。
    fn precision(&self, column: ColumnIndex) -> Result<usize, RdbcError> {
        self.lock().precision(column)
    }
    /// 委托同一底层对象的 scale，保持参数、返回值及原错误；不创建元数据副本。
    fn scale(&self, column: ColumnIndex) -> Result<usize, RdbcError> {
        self.lock().scale(column)
    }
    /// 委托同一底层对象的 table_name，保持参数、返回值及原错误；不创建元数据副本。
    fn table_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.lock().table_name(column)
    }
    /// 委托同一底层对象的 catalog_name，保持参数、返回值及原错误；不创建元数据副本。
    fn catalog_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.lock().catalog_name(column)
    }
    /// 委托同一底层对象的 column_type，保持参数、返回值及原错误；不创建元数据副本。
    fn column_type(&self, column: ColumnIndex) -> Result<ResultSetColumnType, RdbcError> {
        self.lock().column_type(column)
    }
    /// 委托同一底层对象的 column_type_name，保持参数、返回值及原错误；不创建元数据副本。
    fn column_type_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.lock().column_type_name(column)
    }
    /// 委托同一底层对象的 is_read_only，保持参数、返回值及原错误；不创建元数据副本。
    fn is_read_only(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.lock().is_read_only(column)
    }
    /// 委托同一底层对象的 is_writable，保持参数、返回值及原错误；不创建元数据副本。
    fn is_writable(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.lock().is_writable(column)
    }
    /// 委托同一底层对象的 is_definitely_writable，保持参数、返回值及原错误；不创建元数据副本。
    fn is_definitely_writable(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.lock().is_definitely_writable(column)
    }
    /// 委托同一底层对象的 column_class_name，保持参数、返回值及原错误；不创建元数据副本。
    fn column_class_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.lock().column_class_name(column)
    }
}
impl RowSetMetaData for SharedRowSetMetaData {
    /// 委托同一底层对象的 set_column_count，保持参数、返回值及原错误；不创建元数据副本。
    fn set_column_count(&mut self, column_count: i32) -> Result<(), RdbcError> {
        self.lock().set_column_count(column_count)
    }
    /// 委托同一底层对象的 set_auto_increment，保持参数、返回值及原错误；不创建元数据副本。
    fn set_auto_increment(
        &mut self,
        column_index: ColumnIndex,
        property: bool,
    ) -> Result<(), RdbcError> {
        self.lock().set_auto_increment(column_index, property)
    }
    /// 委托同一底层对象的 set_case_sensitive，保持参数、返回值及原错误；不创建元数据副本。
    fn set_case_sensitive(
        &mut self,
        column_index: ColumnIndex,
        property: bool,
    ) -> Result<(), RdbcError> {
        self.lock().set_case_sensitive(column_index, property)
    }
    /// 委托同一底层对象的 set_searchable，保持参数、返回值及原错误；不创建元数据副本。
    fn set_searchable(
        &mut self,
        column_index: ColumnIndex,
        property: bool,
    ) -> Result<(), RdbcError> {
        self.lock().set_searchable(column_index, property)
    }
    /// 委托同一底层对象的 set_currency，保持参数、返回值及原错误；不创建元数据副本。
    fn set_currency(&mut self, column_index: ColumnIndex, property: bool) -> Result<(), RdbcError> {
        self.lock().set_currency(column_index, property)
    }
    /// 委托同一底层对象的 set_signed，保持参数、返回值及原错误；不创建元数据副本。
    fn set_signed(&mut self, column_index: ColumnIndex, property: bool) -> Result<(), RdbcError> {
        self.lock().set_signed(column_index, property)
    }
    /// 委托同一底层对象的 set_nullable，保持参数、返回值及原错误；不创建元数据副本。
    fn set_nullable(&mut self, column_index: ColumnIndex, property: i32) -> Result<(), RdbcError> {
        self.lock().set_nullable(column_index, property)
    }
    /// 委托同一底层对象的 set_column_display_size，保持参数、返回值及原错误；不创建元数据副本。
    fn set_column_display_size(
        &mut self,
        column_index: ColumnIndex,
        value: i32,
    ) -> Result<(), RdbcError> {
        self.lock().set_column_display_size(column_index, value)
    }
    /// 委托同一底层对象的 set_precision，保持参数、返回值及原错误；不创建元数据副本。
    fn set_precision(&mut self, column_index: ColumnIndex, value: i32) -> Result<(), RdbcError> {
        self.lock().set_precision(column_index, value)
    }
    /// 委托同一底层对象的 set_scale，保持参数、返回值及原错误；不创建元数据副本。
    fn set_scale(&mut self, column_index: ColumnIndex, value: i32) -> Result<(), RdbcError> {
        self.lock().set_scale(column_index, value)
    }
    /// 委托同一底层对象的 set_column_label，保持参数、返回值及原错误；不创建元数据副本。
    fn set_column_label(
        &mut self,
        column_index: ColumnIndex,
        label: Option<String>,
    ) -> Result<(), RdbcError> {
        self.lock().set_column_label(column_index, label)
    }
    /// 委托同一底层对象的 set_column_name，保持参数、返回值及原错误；不创建元数据副本。
    fn set_column_name(
        &mut self,
        column_index: ColumnIndex,
        column_name: Option<String>,
    ) -> Result<(), RdbcError> {
        self.lock().set_column_name(column_index, column_name)
    }
    /// 委托同一底层对象的 set_schema_name，保持参数、返回值及原错误；不创建元数据副本。
    fn set_schema_name(
        &mut self,
        column_index: ColumnIndex,
        schema_name: Option<String>,
    ) -> Result<(), RdbcError> {
        self.lock().set_schema_name(column_index, schema_name)
    }
    /// 委托同一底层对象的 set_table_name，保持参数、返回值及原错误；不创建元数据副本。
    fn set_table_name(
        &mut self,
        column_index: ColumnIndex,
        table_name: Option<String>,
    ) -> Result<(), RdbcError> {
        self.lock().set_table_name(column_index, table_name)
    }
    /// 委托同一底层对象的 set_catalog_name，保持参数、返回值及原错误；不创建元数据副本。
    fn set_catalog_name(
        &mut self,
        column_index: ColumnIndex,
        catalog_name: Option<String>,
    ) -> Result<(), RdbcError> {
        self.lock().set_catalog_name(column_index, catalog_name)
    }
    /// 委托同一底层对象的 set_column_type_name，保持参数、返回值及原错误；不创建元数据副本。
    fn set_column_type_name(
        &mut self,
        column_index: ColumnIndex,
        type_name: Option<String>,
    ) -> Result<(), RdbcError> {
        self.lock().set_column_type_name(column_index, type_name)
    }
    /// 委托同一底层对象的 set_column_type，保持参数、返回值及原错误；不创建元数据副本。
    fn set_column_type(
        &mut self,
        column_index: ColumnIndex,
        sql_type: i32,
    ) -> Result<(), RdbcError> {
        self.lock().set_column_type(column_index, sql_type)
    }
}
