use crate::error::RdbcError;
use crate::resource::ResourceOperations;
use crate::types::{RdbcObject, RdbcTypeMap};

/// 独占可变SQL资源标准接口；对应 Java: java.sql.Ref。
/// 对象独立拥有，仅要求Send，不要求Sync。SQL REF是结构值的逻辑指针，取得引用不物化目标属性。
/// 支持该数据类型的驱动必须实现全部四个JDBC方法；不继承Wrapper，没有free或Connection#createRef。
/// 引用可作为列值或结构属性，也可通过PreparedStatement#setRef写入数据库。各方法返回实际值或数据库错误。
#[async_trait::async_trait]
pub trait Ref: std::fmt::Debug + Send + 'static {
    /// 返回真实支持的操作集合（Rust扩展）。
    fn capabilities(&self) -> ResourceOperations;
    /// 对应 Java: Ref#getBaseTypeName()；返回引用目标的完整SQL结构类型名。
    async fn base_type_name(&mut self) -> Result<String, RdbcError>;
    /// 对应 Java: Ref#getObject()；按连接关联映射读取目标，无匹配映射时返回Struct对象。
    async fn object(&mut self) -> Result<RdbcObject, RdbcError>;
    /// 对应 Java: Ref#getObject(Map)；使用参数type_map的完整SQL结构类型名与SQLData目标映射读取对象。
    async fn object_with_type_map(
        &mut self,
        type_map: &RdbcTypeMap,
    ) -> Result<RdbcObject, RdbcError>;
    /// 对应 Java: Ref#setObject(Object)；替换引用目标。
    async fn set_object(&mut self, value: RdbcObject) -> Result<(), RdbcError>;
}
