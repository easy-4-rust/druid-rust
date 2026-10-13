use std::fmt;

use crate::error::RdbcError;
use crate::resource::{
    RdbcArray, RdbcBlob, RdbcClob, RdbcNClob, RdbcRef, RdbcSqlXml, ResourceOperations,
};

/// 标准资源句柄的共同身份、能力与释放合同；对应 Java: java.sql.Array/Blob/Clob/NClob/Ref/SQLXML。
/// Ref 没有 Java `free` 操作，保留无副作用的默认释放；实际可释放资源委托唯一标准对象。
#[async_trait::async_trait]
pub trait Resource: fmt::Debug + Send + Sync {
    /// 返回稳定的资源身份；克隆句柄应返回相同值。
    fn resource_id(&self) -> &str;

    /// 返回该实例实际支持的标准操作集合。
    fn capabilities(&self) -> ResourceOperations;

    /// 释放资源；无 Java `free` 操作的资源默认成功且保持可用。
    async fn free(&self) -> Result<(), RdbcError> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl Resource for RdbcArray {
    fn resource_id(&self) -> &str {
        RdbcArray::resource_id(self)
    }

    fn capabilities(&self) -> ResourceOperations {
        RdbcArray::capabilities(self)
    }

    async fn free(&self) -> Result<(), RdbcError> {
        RdbcArray::free(self).await
    }
}

#[async_trait::async_trait]
impl Resource for RdbcBlob {
    fn resource_id(&self) -> &str {
        RdbcBlob::resource_id(self)
    }

    fn capabilities(&self) -> ResourceOperations {
        RdbcBlob::capabilities(self)
    }

    async fn free(&self) -> Result<(), RdbcError> {
        RdbcBlob::free(self).await
    }
}

#[async_trait::async_trait]
impl Resource for RdbcClob {
    fn resource_id(&self) -> &str {
        RdbcClob::resource_id(self)
    }

    fn capabilities(&self) -> ResourceOperations {
        RdbcClob::capabilities(self)
    }

    async fn free(&self) -> Result<(), RdbcError> {
        RdbcClob::free(self).await
    }
}

#[async_trait::async_trait]
impl Resource for RdbcNClob {
    fn resource_id(&self) -> &str {
        RdbcNClob::resource_id(self)
    }

    fn capabilities(&self) -> ResourceOperations {
        RdbcNClob::capabilities(self)
    }

    async fn free(&self) -> Result<(), RdbcError> {
        RdbcNClob::free(self).await
    }
}

#[async_trait::async_trait]
impl Resource for RdbcRef {
    fn resource_id(&self) -> &str {
        RdbcRef::resource_id(self)
    }

    fn capabilities(&self) -> ResourceOperations {
        RdbcRef::capabilities(self)
    }
}

#[async_trait::async_trait]
impl Resource for RdbcSqlXml {
    fn resource_id(&self) -> &str {
        RdbcSqlXml::resource_id(self)
    }

    fn capabilities(&self) -> ResourceOperations {
        RdbcSqlXml::capabilities(self)
    }

    async fn free(&self) -> Result<(), RdbcError> {
        RdbcSqlXml::free(self).await
    }
}
