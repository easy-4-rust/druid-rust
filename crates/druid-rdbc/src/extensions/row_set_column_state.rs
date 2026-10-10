use crate::metadata::{ColumnDescriptor, ColumnNullability, ResultSetColumnType};

/// 行集一列的可变状态，向标准metadata输出独立快照。
/// 对应 Java: javax.sql.RowSetMetaData的列属性（Rust实现的私有状态）。
#[derive(Clone, Debug)]
pub(crate) struct RowSetColumnState {
    pub(crate) auto_increment: bool,
    pub(crate) case_sensitive: bool,
    pub(crate) searchable: bool,
    pub(crate) currency: bool,
    pub(crate) signed: bool,
    pub(crate) nullable: ColumnNullability,
    pub(crate) column_display_size: usize,
    pub(crate) precision: usize,
    pub(crate) scale: usize,
    pub(crate) column_label: String,
    pub(crate) column_name: String,
    pub(crate) schema_name: String,
    pub(crate) table_name: String,
    pub(crate) catalog_name: String,
    pub(crate) column_type_name: String,
    pub(crate) sql_type: i32,
}
impl Default for RowSetColumnState {
    fn default() -> Self {
        Self {
            auto_increment: false,
            case_sensitive: false,
            searchable: false,
            currency: false,
            signed: false,
            nullable: ColumnNullability::Unknown,
            column_display_size: 0,
            precision: 0,
            scale: 0,
            column_label: String::new(),
            column_name: String::new(),
            schema_name: String::new(),
            table_name: String::new(),
            catalog_name: String::new(),
            column_type_name: String::new(),
            sql_type: 0,
        }
    }
}
impl RowSetColumnState {
    /// 复制当前列属性并返回owned标准快照，不使后续setter影响已返回数据。
    pub(crate) fn snapshot(&self) -> ColumnDescriptor {
        ColumnDescriptor::new(
            self.column_label.clone(),
            self.column_type_name.clone(),
            self.nullable,
        )
        .with_driver_type(
            self.column_name.clone(),
            ResultSetColumnType::Jdbc(self.sql_type),
        )
        .with_dimensions(self.precision, self.scale, self.column_display_size)
        .with_origin(
            self.catalog_name.clone(),
            self.schema_name.clone(),
            self.table_name.clone(),
        )
        .with_flags(
            self.auto_increment,
            self.case_sensitive,
            self.searchable,
            self.currency,
            self.signed,
        )
        .with_writability(false, true, true)
        .with_class_name("java.lang.Object")
    }
}
