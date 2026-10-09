use crate::error::RdbcError;
use crate::resource::ResourceOperations;
use crate::types::{RdbcObject, RdbcTypeMap};

/// SQL结构类型的独占标准对象；对应 Java: java.sql.Struct。
/// 属性按声明顺序保留，包括SQL NULL及嵌套对象；支持此类型的驱动必须实现全部三个方法。
/// 默认有效期持续到应用不再持有引用，不附加连接关闭失效或free方法；只要求Send，不要求Sync。
#[async_trait::async_trait]
pub trait Struct: std::fmt::Debug + Send + 'static {
    /// 返回当前对象真实支持的操作集合（Rust扩展），无参数。
    fn capabilities(&self) -> ResourceOperations;
    /// 返回完整限定SQL类型名，无参数；数据库访问或不支持时返回错误。
    /// 对应 Java: Struct#getSQLTypeName。
    async fn sql_type_name(&mut self) -> Result<String, RdbcError>;
    /// 按声明顺序返回属性对象，无参数；使用连接关联的类型映射，无匹配项时使用标准映射。
    /// 概念上逐属性执行getObject；数据库访问或不支持时返回错误。对应 Java: Struct#getAttributes()。
    async fn attributes(&mut self) -> Result<Vec<RdbcObject>, RdbcError>;
    /// 使用参数type_map显式映射属性，返回声明顺序的对象；无匹配项时使用标准映射。
    /// 此重载绝不使用连接的类型映射；数据库访问或不支持时返回错误。
    /// 对应 Java: Struct#getAttributes(Map)。
    async fn attributes_with_type_map(&mut self, type_map: &RdbcTypeMap) -> Result<Vec<RdbcObject>, RdbcError>;
}
