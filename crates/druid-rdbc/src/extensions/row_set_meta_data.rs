use crate::error::RdbcError;
use crate::metadata::ResultSetMetaData;
use crate::result_set::ColumnIndex;

/// 行集的可设置列元数据合同；继承同一标准结果集元数据及Wrapper身份。
/// 对应 Java: javax.sql.RowSetMetaData。由RowSetReader内部创建、设置并交给行集；应用通常不直接调用。
/// 所有setter都是必需方法，驱动不得以默认空实现声明支持。
pub trait RowSetMetaData: ResultSetMetaData {
    /// 设置参数column_count表示的列数；返回成功或数据库/参数错误。对应 Java: RowSetMetaData#setColumnCount。
    fn set_column_count(&mut self, column_count: i32) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的是否自动编号为property；返回成功或数据库错误。默认false。对应 Java: RowSetMetaData#setAutoIncrement。
    fn set_auto_increment(
        &mut self,
        column_index: ColumnIndex,
        property: bool,
    ) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的是否区分大小写为property；返回成功或数据库错误。默认false。对应 Java: RowSetMetaData#setCaseSensitive。
    fn set_case_sensitive(
        &mut self,
        column_index: ColumnIndex,
        property: bool,
    ) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的是否可用于WHERE查询为property；返回成功或数据库错误。默认false。对应 Java: RowSetMetaData#setSearchable。
    fn set_searchable(
        &mut self,
        column_index: ColumnIndex,
        property: bool,
    ) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的是否货币值为property；返回成功或数据库错误。默认false。对应 Java: RowSetMetaData#setCurrency。
    fn set_currency(&mut self, column_index: ColumnIndex, property: bool) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的是否有符号为property；返回成功或数据库错误。默认false。对应 Java: RowSetMetaData#setSigned。
    fn set_signed(&mut self, column_index: ColumnIndex, property: bool) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的可空性property（0不可空、1可空、2未知，默认未知）；返回成功或数据库错误。对应 Java: RowSetMetaData#setNullable。
    fn set_nullable(&mut self, column_index: ColumnIndex, property: i32) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的显示宽度为value；返回成功或数据库错误。对应 Java: RowSetMetaData#setColumnDisplaySize。
    fn set_column_display_size(
        &mut self,
        column_index: ColumnIndex,
        value: i32,
    ) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的十进制精度为value；返回成功或数据库错误。对应 Java: RowSetMetaData#setPrecision。
    fn set_precision(&mut self, column_index: ColumnIndex, value: i32) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的小数位数为value；返回成功或数据库错误。对应 Java: RowSetMetaData#setScale。
    fn set_scale(&mut self, column_index: ColumnIndex, value: i32) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的列标题为label；None表示Java null，返回成功或数据库错误。对应 Java: RowSetMetaData#setColumnLabel。
    fn set_column_label(
        &mut self,
        column_index: ColumnIndex,
        label: Option<String>,
    ) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的列名称为column_name；None表示Java null，返回成功或数据库错误。对应 Java: RowSetMetaData#setColumnName。
    fn set_column_name(
        &mut self,
        column_index: ColumnIndex,
        column_name: Option<String>,
    ) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的模式名称为schema_name；None表示Java null，返回成功或数据库错误。对应 Java: RowSetMetaData#setSchemaName。
    fn set_schema_name(
        &mut self,
        column_index: ColumnIndex,
        schema_name: Option<String>,
    ) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的表名称为table_name；None表示Java null，返回成功或数据库错误。对应 Java: RowSetMetaData#setTableName。
    fn set_table_name(
        &mut self,
        column_index: ColumnIndex,
        table_name: Option<String>,
    ) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的目录名称为catalog_name；None表示Java null，返回成功或数据库错误。对应 Java: RowSetMetaData#setCatalogName。
    fn set_catalog_name(
        &mut self,
        column_index: ColumnIndex,
        catalog_name: Option<String>,
    ) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的数据源专属类型名为type_name；None表示Java null，返回成功或数据库错误。对应 Java: RowSetMetaData#setColumnTypeName。
    fn set_column_type_name(
        &mut self,
        column_index: ColumnIndex,
        type_name: Option<String>,
    ) -> Result<(), RdbcError>;
    /// 设置1-based参数column_index的SQL类型代码sql_type；返回成功或数据库错误。对应 Java: RowSetMetaData#setColumnType，代码属于java.sql.Types。
    fn set_column_type(
        &mut self,
        column_index: ColumnIndex,
        sql_type: i32,
    ) -> Result<(), RdbcError>;
}
