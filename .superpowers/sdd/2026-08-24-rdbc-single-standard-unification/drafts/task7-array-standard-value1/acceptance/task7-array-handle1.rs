use crate::error::RdbcError;
use crate::resource::{Array, ResourceOperations};
use crate::result_set::ResultSet;
use crate::types::{RdbcObject, RdbcTypeMap};
use super::standard_resource_value::StandardResourceValue;
use std::fmt;
use std::sync::Arc;

/// SQL数组的共享值句柄；对应 Java: java.sql.Array。
/// 只承载唯一标准Array对象，克隆共享身份与释放确认状态，驱动仅要求Send。
/// 范围、映射与索引/值游标原样委托，不物化或替换驱动返回的ResultSet。
#[derive(Clone)]
pub struct RdbcArray {value:Arc<StandardResourceValue<dyn Array>>}
impl RdbcArray {
    pub(crate) fn from_standard(value:Box<dyn Array>)->Self {
        let capabilities=value.capabilities() & ResourceOperations::array();
        Self {value:Arc::new(StandardResourceValue::new(value,capabilities,"Array"))}
    }
    /// 返回完整SQL元素类型名，无参数；返回原驱动错误。对应 Java: Array#getBaseTypeName。
    pub async fn base_type_name(&self)->Result<String,RdbcError> {self.value.lock(ResourceOperations::READ,"Array#getBaseTypeName").await?.base_type_name().await}
    /// 返回完整SQL元素类型名，无参数；返回原驱动错误。对应 Java: Array#getBaseTypeName。
    pub async fn get_base_type_name(&self)->Result<String,RdbcError> {self.base_type_name().await}
    /// 返回元素Types编号，无参数；返回原驱动错误。对应 Java: Array#getBaseType。
    pub async fn base_type(&self)->Result<i32,RdbcError> {self.value.lock(ResourceOperations::READ,"Array#getBaseType").await?.base_type().await}
    /// 返回元素Types编号，无参数；返回原驱动错误。对应 Java: Array#getBaseType。
    pub async fn get_base_type(&self)->Result<i32,RdbcError> {self.base_type().await}
    /// 按默认映射读取全部元素，无参数；保留对象及SQL NULL。对应 Java: Array#getArray()。
    pub async fn values(&self)->Result<Vec<RdbcObject>,RdbcError> {self.value.lock(ResourceOperations::READ,"Array#getArray").await?.values().await}
    /// 按默认映射读取全部元素，无参数；返回对象数组或原错误。对应 Java: Array#getArray()。
    pub async fn get_array(&self)->Result<Vec<RdbcObject>,RdbcError> {self.values().await}
    /// 使用参数type_map显式映射元素，返回对象数组或原错误。对应 Java: Array#getArray(Map)。
    pub async fn values_with_type_map(&self,type_map:&RdbcTypeMap)->Result<Vec<RdbcObject>,RdbcError> {self.value.lock(ResourceOperations::READ|ResourceOperations::TYPE_MAP,"Array#getArray(Map)").await?.values_with_type_map(type_map).await}
    /// 按1-based参数index与count读取范围，返回对象数组或原错误。对应 Java: Array#getArray(long,int)。
    pub async fn values_range(&self,index:i64,count:i32)->Result<Vec<RdbcObject>,RdbcError> {self.value.lock(ResourceOperations::READ|ResourceOperations::RANGE,"Array#getArray(long,int)").await?.values_range(index,count).await}
    /// 按参数index/count/type_map读取范围并显式映射，返回对象数组或原错误。对应 Java: Array#getArray(long,int,Map)。
    pub async fn values_range_with_type_map(&self,index:i64,count:i32,type_map:&RdbcTypeMap)->Result<Vec<RdbcObject>,RdbcError> {self.value.lock(ResourceOperations::READ|ResourceOperations::RANGE|ResourceOperations::TYPE_MAP,"Array#getArray(long,int,Map)").await?.values_range_with_type_map(index,count,type_map).await}
    /// 返回独立拥有的索引/值游标，无参数；原样返回驱动对象或错误。对应 Java: Array#getResultSet()。
    pub async fn result_set(&self)->Result<Box<dyn ResultSet>,RdbcError> {self.value.lock(ResourceOperations::READ|ResourceOperations::RESULT_SET,"Array#getResultSet").await?.result_set().await}
    /// 按参数type_map返回显式映射的索引/值游标或原错误。对应 Java: Array#getResultSet(Map)。
    pub async fn result_set_with_type_map(&self,type_map:&RdbcTypeMap)->Result<Box<dyn ResultSet>,RdbcError> {self.value.lock(ResourceOperations::READ|ResourceOperations::RESULT_SET|ResourceOperations::TYPE_MAP,"Array#getResultSet(Map)").await?.result_set_with_type_map(type_map).await}
    /// 按1-based参数index与count返回范围游标或原错误。对应 Java: Array#getResultSet(long,int)。
    pub async fn result_set_range(&self,index:i64,count:i32)->Result<Box<dyn ResultSet>,RdbcError> {self.value.lock(ResourceOperations::READ|ResourceOperations::RANGE|ResourceOperations::RESULT_SET,"Array#getResultSet(long,int)").await?.result_set_range(index,count).await}
    /// 按参数index/count/type_map返回范围映射游标或原错误。对应 Java: Array#getResultSet(long,int,Map)。
    pub async fn result_set_range_with_type_map(&self,index:i64,count:i32,type_map:&RdbcTypeMap)->Result<Box<dyn ResultSet>,RdbcError> {self.value.lock(ResourceOperations::READ|ResourceOperations::RANGE|ResourceOperations::RESULT_SET|ResourceOperations::TYPE_MAP,"Array#getResultSet(long,int,Map)").await?.result_set_range_with_type_map(index,count,type_map).await}
    /// 幂等释放数组，无参数；失败保留原错误并允许重试，取消后拒绝读取直到重试确认。对应 Java: Array#free。
    pub async fn free(&self)->Result<(),RdbcError> {self.value.free(|value|value.free()).await}
    /// 返回本共享句柄是否已确认成功释放，无参数（Rust扩展）。
    #[must_use]
    pub fn is_freed(&self)->bool {self.value.is_freed()}
    /// 返回共享句柄稳定身份，无参数（Rust扩展）。
    #[must_use]
    pub fn resource_id(&self)->&str {self.value.resource_id()}
    /// 返回驱动声明与Array标准操作范围交集，无参数（Rust扩展）。
    #[must_use]
    pub fn capabilities(&self)->ResourceOperations {self.value.capabilities()}
}
impl fmt::Debug for RdbcArray {fn fmt(&self,formatter:&mut fmt::Formatter<'_>)->fmt::Result {formatter.debug_struct("RdbcArray").field("value",&self.value).finish_non_exhaustive()}}
impl PartialEq for RdbcArray {fn eq(&self,other:&Self)->bool {Arc::ptr_eq(&self.value,&other.value)}}
impl Eq for RdbcArray {}
