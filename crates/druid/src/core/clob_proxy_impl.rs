//! 对应 Java：`com.alibaba.druid.proxy.rdbc.ClobProxyImpl`。

use super::{
    ClobProxy, DruidError, FilterChain, RdbcClob, RdbcInputStream, RdbcOutputStream, RdbcReader,
    RdbcString, RdbcWriter,
};
use std::fmt;
use std::sync::Arc;

/// 经 Druid `FilterChain` 访问的 Clob Proxy。
pub struct ClobProxyImpl {
    connection_id: u64,
    clob: RdbcClob,
    filter_chain: Arc<FilterChain>,
}

impl ClobProxyImpl {
    /// 创建 Clob Proxy；Rust 类型系统保证 raw Clob 非空。
    #[must_use]
    pub fn new(connection_id: u64, clob: RdbcClob, filter_chain: Arc<FilterChain>) -> Self {
        Self {
            connection_id,
            clob,
            filter_chain,
        }
    }

    /// 对应 Java：`Clob#free()`。
    pub async fn free(&self) -> Result<(), DruidError> {
        self.filter_chain.clob_free(self).await
    }

    /// 对应 Java：`Clob#getAsciiStream()`。
    pub async fn get_ascii_stream(&self) -> Result<RdbcInputStream, DruidError> {
        self.filter_chain.clob_get_ascii_stream(self).await
    }

    /// 对应 Java：`Clob#getCharacterStream()`。
    pub async fn get_character_stream(&self) -> Result<RdbcReader, DruidError> {
        self.filter_chain.clob_get_character_stream(self).await
    }

    /// 对应 Java：`Clob#getCharacterStream(long,long)`。
    pub async fn get_character_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, DruidError> {
        self.filter_chain
            .clob_get_character_stream_range(self, position, length)
            .await
    }

    /// 对应 Java：`Clob#getSubString(long,int)`。
    pub async fn get_sub_string(
        &self,
        position: i64,
        length: i32,
    ) -> Result<RdbcString, DruidError> {
        self.filter_chain
            .clob_get_sub_string(self, position, length)
            .await
    }

    /// 对应 Java：`Clob#length()`。
    pub async fn length(&self) -> Result<i64, DruidError> {
        self.filter_chain.clob_length(self).await
    }

    /// 对应 Java：`Clob#position(String,long)`。
    pub async fn position_string(
        &self,
        pattern: &RdbcString,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        self.filter_chain
            .clob_position_string(self, pattern, start)
            .await
    }

    /// 对应 Java：`Clob#position(Clob,long)`。
    pub async fn position_clob(
        &self,
        pattern: &RdbcClob,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        self.filter_chain
            .clob_position_clob(self, pattern, start)
            .await
    }

    /// 对应 Java：`Clob#setAsciiStream(long)`。
    pub async fn set_ascii_stream(&self, position: i64) -> Result<RdbcOutputStream, DruidError> {
        self.filter_chain
            .clob_set_ascii_stream(self, position)
            .await
    }

    /// 对应 Java：`Clob#setCharacterStream(long)`。
    pub async fn set_character_stream(&self, position: i64) -> Result<RdbcWriter, DruidError> {
        self.filter_chain
            .clob_set_character_stream(self, position)
            .await
    }

    /// 对应 Java：`Clob#setString(long,String)`。
    pub async fn set_string(&self, position: i64, value: &RdbcString) -> Result<i32, DruidError> {
        self.filter_chain
            .clob_set_string(self, position, value)
            .await
    }

    /// 对应 Java：`Clob#setString(long,String,int,int)`。
    pub async fn set_string_range(
        &self,
        position: i64,
        value: &RdbcString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        self.filter_chain
            .clob_set_string_range(self, position, value, offset, length)
            .await
    }

    /// 对应 Java：`Clob#truncate(long)`。
    pub async fn truncate(&self, length: i64) -> Result<(), DruidError> {
        self.filter_chain.clob_truncate(self, length).await
    }
}

impl ClobProxy for ClobProxyImpl {
    fn connection_id(&self) -> u64 {
        self.connection_id
    }

    fn raw_clob(&self) -> &RdbcClob {
        &self.clob
    }
}

impl fmt::Debug for ClobProxyImpl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClobProxyImpl")
            .field("connection_id", &self.connection_id)
            .field("clob", &self.clob)
            .finish_non_exhaustive()
    }
}
