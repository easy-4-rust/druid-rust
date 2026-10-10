use crate::error::RdbcError;
use crate::resource::{SqlInput, SqlOutput};

/// SQL结构类型或DISTINCT类型到应用对象的自定义映射合同。对应 Java: java.sql.SQLData。
/// 连接类型映射关联限定UDT名与实现；ResultSet.getObject读入，PreparedStatement.setObject写回。
/// 通常由驱动而非应用直接调用。字段按SQL声明顺序读取和写入，DISTINCT类型恰好一个元素。
/// 独占异步调用只要求Send，不强迫应用对象Sync；不把可变应用对象包装为不可变共享值。
#[async_trait::async_trait]
pub trait SqlData: Send {
    /// 返回read_sql接收的完全限定SQL类型名，无参数；数据库访问或不支持时返回原SQL错误。
    /// 对应 Java: SQLData#getSQLTypeName。
    async fn sql_type_name(&mut self) -> Result<String, RdbcError>;
    /// 从参数input按声明顺序逐个读取属性并填入本对象；参数type_name是驱动提供的限定SQL类型名。
    /// 驱动预先初始化input的类型映射；返回成功或原数据库/映射/不支持错误。对应 Java: SQLData#readSQL。
    async fn read_sql(
        &mut self,
        input: &mut dyn SqlInput,
        type_name: &str,
    ) -> Result<(), RdbcError>;
    /// 向参数output按声明顺序写入本对象的属性；返回成功或原数据库/序列化/不支持错误。
    /// DISTINCT写一个元素，结构类型写每个属性。对应 Java: SQLData#writeSQL。
    async fn write_sql(&mut self, output: &mut dyn SqlOutput) -> Result<(), RdbcError>;
}
