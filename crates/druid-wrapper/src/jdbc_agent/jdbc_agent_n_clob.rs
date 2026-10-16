use std::sync::Arc;

use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{
    Clob, ClobPattern, NClob, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcWriter,
    ResourceOperations,
};
use druid_rdbc::types::RdbcString;

use super::jdbc_agent_clob::JdbcAgentClob;
use crate::standard_adapter_executor::StandardExecutor;

/// 保留真实 JDBC NClob 身份，并复用其继承的 Clob 操作。对应 Java: java.sql.NClob。
pub(super) struct JdbcAgentNClob {
    inherited: JdbcAgentClob,
}

impl JdbcAgentNClob {
    /// 接收已由 Agent 注册的 NClob 句柄，返回独占资源适配器。
    pub(super) fn new(executor: Arc<dyn StandardExecutor>, n_clob_id: String) -> Self {
        Self {
            inherited: JdbcAgentClob::national(executor, n_clob_id),
        }
    }
}

impl std::fmt::Debug for JdbcAgentNClob {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JdbcAgentNClob")
            .field("inherited", &self.inherited)
            .finish()
    }
}

#[async_trait::async_trait]
impl Clob for JdbcAgentNClob {
    fn capabilities(&self) -> ResourceOperations {
        self.inherited.capabilities()
    }

    async fn length(&mut self) -> Result<i64, RdbcError> {
        self.inherited.length().await
    }

    async fn get_sub_string(
        &mut self,
        position: i64,
        length: i32,
    ) -> Result<RdbcString, RdbcError> {
        self.inherited.get_sub_string(position, length).await
    }

    async fn get_character_stream(&mut self) -> Result<RdbcReader, RdbcError> {
        self.inherited.get_character_stream().await
    }

    async fn get_ascii_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        self.inherited.get_ascii_stream().await
    }

    async fn position_string(
        &mut self,
        pattern: &RdbcString,
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        self.inherited.position_string(pattern, start).await
    }

    async fn position_clob(
        &mut self,
        pattern: ClobPattern<'_>,
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        self.inherited.position_clob(pattern, start).await
    }

    async fn set_string(&mut self, position: i64, value: &RdbcString) -> Result<i32, RdbcError> {
        self.inherited.set_string(position, value).await
    }

    async fn set_string_range(
        &mut self,
        position: i64,
        value: &RdbcString,
        offset: i32,
        length: i32,
    ) -> Result<i32, RdbcError> {
        self.inherited
            .set_string_range(position, value, offset, length)
            .await
    }

    async fn set_ascii_stream(&mut self, position: i64) -> Result<RdbcOutputStream, RdbcError> {
        self.inherited.set_ascii_stream(position).await
    }

    async fn set_character_stream(&mut self, position: i64) -> Result<RdbcWriter, RdbcError> {
        self.inherited.set_character_stream(position).await
    }

    async fn truncate(&mut self, length: i64) -> Result<(), RdbcError> {
        self.inherited.truncate(length).await
    }

    async fn free(&mut self) -> Result<(), RdbcError> {
        self.inherited.free().await
    }

    async fn get_character_stream_range(
        &mut self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, RdbcError> {
        self.inherited
            .get_character_stream_range(position, length)
            .await
    }
}

impl NClob for JdbcAgentNClob {}
