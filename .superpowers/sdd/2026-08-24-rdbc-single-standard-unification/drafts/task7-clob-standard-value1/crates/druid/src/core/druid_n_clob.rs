use super::druid_clob::DruidClob;
use super::resource_context::ResourceContext;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_registry::StandardResourceRegistry;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{
    Clob, ClobPattern, NClob, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcWriter, ResourceOperations,
};
use druid_rdbc::types::RdbcString;
use std::sync::Arc;

/// 保留国家字符资源身份，复用同一 Clob 操作和监管状态。
/// 对应 Java: com.alibaba.druid.proxy.jdbc.NClobProxyImpl。
#[derive(Debug)]
pub(crate) struct DruidNClob {
    inner: DruidClob,
}
impl DruidNClob {
    /// 接管国家字符驱动资源、上下文和 registry；登记成功返回独占代理，失败返回清理错误。
    pub(crate) async fn register(
        payload: Box<dyn NClob>,
        context: Arc<ResourceContext>,
        registry: Arc<StandardResourceRegistry>,
        admission: &StandardResourceAdmission,
    ) -> Result<Self, RdbcError> {
        // NClob 不增加 JDBC 方法；上转保留真实驱动对象，不复制内容或重建资源。
        let payload: Box<dyn Clob> = payload;
        Ok(Self {
            inner: DruidClob::register(payload, context, registry, admission).await?,
        })
    }
}
#[async_trait::async_trait]
impl Clob for DruidNClob {
    fn capabilities(&self) -> ResourceOperations {
        self.inner.capabilities()
    }
    async fn length(&mut self) -> Result<i64, RdbcError> {
        self.inner.length().await
    }
    async fn get_sub_string(
        &mut self,
        position: i64,
        length: i32,
    ) -> Result<RdbcString, RdbcError> {
        self.inner.get_sub_string(position, length).await
    }
    async fn get_character_stream(&mut self) -> Result<RdbcReader, RdbcError> {
        self.inner.get_character_stream().await
    }
    async fn get_ascii_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        self.inner.get_ascii_stream().await
    }
    async fn position_string(
        &mut self,
        pattern: &RdbcString,
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        self.inner.position_string(pattern, start).await
    }
    async fn position_clob(
        &mut self,
        pattern: ClobPattern<'_>,
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        self.inner.position_clob(pattern, start).await
    }
    async fn set_string(&mut self, position: i64, value: &RdbcString) -> Result<i32, RdbcError> {
        self.inner.set_string(position, value).await
    }
    async fn set_string_range(
        &mut self,
        position: i64,
        value: &RdbcString,
        offset: i32,
        length: i32,
    ) -> Result<i32, RdbcError> {
        self.inner
            .set_string_range(position, value, offset, length)
            .await
    }
    async fn set_ascii_stream(&mut self, position: i64) -> Result<RdbcOutputStream, RdbcError> {
        self.inner.set_ascii_stream(position).await
    }
    async fn set_character_stream(&mut self, position: i64) -> Result<RdbcWriter, RdbcError> {
        self.inner.set_character_stream(position).await
    }
    async fn truncate(&mut self, length: i64) -> Result<(), RdbcError> {
        self.inner.truncate(length).await
    }
    async fn get_character_stream_range(
        &mut self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, RdbcError> {
        self.inner
            .get_character_stream_range(position, length)
            .await
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        self.inner.free().await
    }
}
impl NClob for DruidNClob {}
