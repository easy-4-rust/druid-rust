//! 对应 Java：`com.alibaba.druid.proxy.jdbc.ClobProxyImpl`。

use super::{
    ClobProxy, DruidError, FilterChain, JavaString, JdbcClob, JdbcInputStream, JdbcOutputStream,
    JdbcReader, JdbcWriter,
};
use std::fmt;
use std::sync::Arc;

/// 经 Druid FilterChain 访问的 Clob Proxy。
pub struct ClobProxyImpl {
    connection_id: u64,
    clob: JdbcClob,
    filter_chain: Arc<FilterChain>,
}

impl ClobProxyImpl {
    /// 创建 Clob Proxy；Rust 类型系统保证 raw Clob 非空。
    #[must_use]
    pub fn new(connection_id: u64, clob: JdbcClob, filter_chain: Arc<FilterChain>) -> Self {
        Self {
            connection_id,
            clob,
            filter_chain,
        }
    }

    /// 对应 Java：`Clob#free()`。
    pub fn free(&self) -> Result<(), DruidError> {
        self.filter_chain.clob_free(self)
    }

    /// 对应 Java：`Clob#getAsciiStream()`。
    pub fn get_ascii_stream(&self) -> Result<JdbcInputStream, DruidError> {
        self.filter_chain.clob_get_ascii_stream(self)
    }

    /// 对应 Java：`Clob#getCharacterStream()`。
    pub fn get_character_stream(&self) -> Result<JdbcReader, DruidError> {
        self.filter_chain.clob_get_character_stream(self)
    }

    /// 对应 Java：`Clob#getCharacterStream(long,long)`。
    pub fn get_character_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<JdbcReader, DruidError> {
        self.filter_chain
            .clob_get_character_stream_range(self, position, length)
    }

    /// 对应 Java：`Clob#getSubString(long,int)`。
    pub fn get_sub_string(&self, position: i64, length: i32) -> Result<JavaString, DruidError> {
        self.filter_chain
            .clob_get_sub_string(self, position, length)
    }

    /// 对应 Java：`Clob#length()`。
    pub fn length(&self) -> Result<i64, DruidError> {
        self.filter_chain.clob_length(self)
    }

    /// 对应 Java：`Clob#position(String,long)`。
    pub fn position_string(
        &self,
        pattern: &JavaString,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        self.filter_chain.clob_position_string(self, pattern, start)
    }

    /// 对应 Java：`Clob#position(Clob,long)`。
    pub fn position_clob(&self, pattern: &JdbcClob, start: i64) -> Result<Option<i64>, DruidError> {
        self.filter_chain.clob_position_clob(self, pattern, start)
    }

    /// 对应 Java：`Clob#setAsciiStream(long)`。
    pub fn set_ascii_stream(&self, position: i64) -> Result<JdbcOutputStream, DruidError> {
        self.filter_chain.clob_set_ascii_stream(self, position)
    }

    /// 对应 Java：`Clob#setCharacterStream(long)`。
    pub fn set_character_stream(&self, position: i64) -> Result<JdbcWriter, DruidError> {
        self.filter_chain.clob_set_character_stream(self, position)
    }

    /// 对应 Java：`Clob#setString(long,String)`。
    pub fn set_string(&self, position: i64, value: &JavaString) -> Result<i32, DruidError> {
        self.filter_chain.clob_set_string(self, position, value)
    }

    /// 对应 Java：`Clob#setString(long,String,int,int)`。
    pub fn set_string_range(
        &self,
        position: i64,
        value: &JavaString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        self.filter_chain
            .clob_set_string_range(self, position, value, offset, length)
    }

    /// 对应 Java：`Clob#truncate(long)`。
    pub fn truncate(&self, length: i64) -> Result<(), DruidError> {
        self.filter_chain.clob_truncate(self, length)
    }
}

impl ClobProxy for ClobProxyImpl {
    fn connection_id(&self) -> u64 {
        self.connection_id
    }

    fn raw_clob(&self) -> &JdbcClob {
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
