use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::ResourceOperations;
use druid_rdbc::types::{RdbcObject,RdbcTypeMap};

/// 独占可变SQL资源标准接口；对应 Java: java.sql.Ref。
/// 对象独立拥有，仅要求Send，不要求Sync。各方法返回实际值或数据库错误。
#[async_trait::async_trait]
pub trait Ref: std::fmt::Debug + Send + 'static {
    /// 返回真实支持的操作集合（Rust扩展）。
    fn capabilities(&self) -> ResourceOperations;
    /// 对应 Java: Ref#getBaseTypeName()；返回引用目标的完整SQL结构类型名。
    async fn base_type_name(&mut self) -> Result<String, RdbcError>;
    /// 对应 Java: Ref#getObject()；按默认映射读取目标对象。
    async fn object(&mut self) -> Result<RdbcObject, RdbcError>;
    /// 对应 Java: Ref#getObject(Map)；显式映射读取目标对象。
    async fn object_with_type_map(&mut self, type_map: &RdbcTypeMap) -> Result<RdbcObject, RdbcError>;
    /// 对应 Java: Ref#setObject(Object)；替换引用目标。
    async fn set_object(&mut self, value: RdbcObject) -> Result<(), RdbcError>;
}
