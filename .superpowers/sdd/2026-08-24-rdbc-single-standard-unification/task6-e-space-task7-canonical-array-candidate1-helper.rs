use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::ResourceOperations;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::types::{RdbcObject, RdbcTypeMap};

/// 独占可变SQL资源标准接口；对应 Java: java.sql.Array。
/// 对象独立拥有，仅要求Send，不要求Sync。各方法返回实际值或数据库错误。
#[async_trait::async_trait]
pub trait Array: std::fmt::Debug + Send + 'static {
    /// 返回真实支持的操作集合（Rust扩展）。
    fn capabilities(&self) -> ResourceOperations;
    /// 对应 Java: Array#getBaseTypeName()；返回完整SQL元素类型名。
    async fn base_type_name(&mut self) -> Result<String, RdbcError>;
    /// 对应 Java: Array#getBaseType()；返回Types编号。
    async fn base_type(&mut self) -> Result<i32, RdbcError>;
    /// 对应 Java: Array#getArray()；按默认类型映射返回元素。
    async fn values(&mut self) -> Result<Vec<RdbcObject>, RdbcError>;
    /// 对应 Java: Array#getArray(Map)；保留显式类型映射。
    async fn values_with_type_map(&mut self, type_map: &RdbcTypeMap) -> Result<Vec<RdbcObject>, RdbcError>;
    /// 对应 Java: Array#getArray(long,int)；元素位置从1开始。
    async fn values_range(&mut self, index: i64, count: i32) -> Result<Vec<RdbcObject>, RdbcError>;
    /// 对应 Java: Array#getArray(long,int,Map)；保留范围与显式映射。
    async fn values_range_with_type_map(&mut self, index: i64, count: i32, type_map: &RdbcTypeMap) -> Result<Vec<RdbcObject>, RdbcError>;
    /// 对应 Java: Array#getResultSet()；返回独立拥有的索引/值结果游标。
    async fn result_set(&mut self) -> Result<Box<dyn ResultSet>, RdbcError>;
    /// 对应 Java: Array#getResultSet(Map)；保留结果游标和类型映射。
    async fn result_set_with_type_map(&mut self, type_map: &RdbcTypeMap) -> Result<Box<dyn ResultSet>, RdbcError>;
    /// 对应 Java: Array#getResultSet(long,int)；返回1-based范围游标。
    async fn result_set_range(&mut self, index: i64, count: i32) -> Result<Box<dyn ResultSet>, RdbcError>;
    /// 对应 Java: Array#getResultSet(long,int,Map)；保留范围及映射。
    async fn result_set_range_with_type_map(&mut self, index: i64, count: i32, type_map: &RdbcTypeMap) -> Result<Box<dyn ResultSet>, RdbcError>;
    /// 对应 Java: Array#free()；幂等释放后其他JDBC操作拒绝。
    async fn free(&mut self) -> Result<(), RdbcError>;
}
