use crate::error::RdbcError;
use crate::resource::{ResourceOperations, Struct};
use crate::types::{RdbcObject, RdbcTypeMap};

/// 已序列化SQLData的单个结构属性，保留UDT名称及原对象身份。对应 Java: java.sql.Struct（SQLData缓冲结果）。
#[derive(Debug)]
pub(crate) struct BufferedSqlStruct {
    pub(crate) type_name: String,
    pub(crate) attributes: Vec<RdbcObject>,
}
#[async_trait::async_trait]
impl Struct for BufferedSqlStruct {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ
    }
    async fn sql_type_name(&mut self) -> Result<String, RdbcError> {
        Ok(self.type_name.clone())
    }
    async fn attributes(&mut self) -> Result<Vec<RdbcObject>, RdbcError> {
        Ok(self.attributes.clone())
    }
    async fn attributes_with_type_map(
        &mut self,
        map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        // 该缓冲层没有应用类构造器；拒绝非空映射，不能把忽略映射伪称TYPE_MAP支持。
        if !map.is_empty() {
            return Err(RdbcError::feature_not_supported(
                "BufferedSqlStruct#getAttributes(Map)",
            ));
        }
        Ok(self.attributes.clone())
    }
}
