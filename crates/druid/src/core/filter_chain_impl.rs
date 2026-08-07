//! 对应 Java 类：com.alibaba.druid.filter.FilterChainImpl

use super::connection::ExecResult;
use super::error::DruidError;
use super::filter::{
    AfterFilter, BatchExecContext, BeforeFilter, ConnectionEvent, ConnectionEventContext,
    ExecContext, StatementEvent, StatementEventContext,
};
use super::{
    ClobProxy, DruidPooledConnection, JavaString, PhysicalConnection, PhysicalConnectionFactory,
    PhysicalDatabaseMetaData, PhysicalPreparedStatement, PhysicalResultSet, PhysicalStatement,
    PoolState, RdbcArray, RdbcBlob, RdbcCalendarArgument, RdbcClob, RdbcInputStream, RdbcNClob,
    RdbcObject, RdbcOutputStream, RdbcReader, RdbcRef, RdbcRowId, RdbcSqlXml, RdbcTargetType,
    RdbcTypeMap, RdbcUrl, RdbcWriter, ResultSetFilter, ResultSetFilterChain,
    ResultSetFilterContext, ResultSetMetaData, ResultSetOpenContext, ResultSetStatement,
    SqlWarning, Value,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

macro_rules! scalar_result_set_filter_chain_methods {
    ($(($index:ident, $label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int)` around-chain。")]
            pub fn $index(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
            ) -> Result<$ty, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index(column_index)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String)` around-chain。")]
            pub fn $label(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
            ) -> Result<$ty, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label(column_label)
            }
        )+
    };
}

macro_rules! temporal_result_set_filter_chain_methods {
    ($(($index:ident, $label:ident, $index_calendar:ident, $label_calendar:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int)` around-chain。")]
            pub fn $index(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
            ) -> Result<Option<$ty>, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index(column_index)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String)` around-chain。")]
            pub fn $label(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
            ) -> Result<Option<$ty>, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label(column_label)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int, Calendar)` around-chain。")]
            pub fn $index_calendar(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
                calendar: &RdbcCalendarArgument,
            ) -> Result<Option<$ty>, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index_calendar(column_index, calendar)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String, Calendar)` around-chain。")]
            pub fn $label_calendar(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
                calendar: &RdbcCalendarArgument,
            ) -> Result<Option<$ty>, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label_calendar(column_label, calendar)
            }
        )+
    };
}

macro_rules! resource_result_set_filter_chain_methods {
    ($(($index:ident, $label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int)` around-chain。")]
            pub fn $index(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
            ) -> Result<Option<$ty>, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index(column_index)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String)` around-chain。")]
            pub fn $label(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
            ) -> Result<Option<$ty>, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label(column_label)
            }
        )+
    };
}

macro_rules! no_arg_result_set_filter_chain_methods {
    ($(($method:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "()` around-chain。")]
            pub fn $method(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
            ) -> Result<$ty, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context).$method()
            }
        )+
    };
}

macro_rules! i32_arg_result_set_filter_chain_methods {
    ($(($method:ident, $argument:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int)` around-chain。")]
            pub fn $method(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                $argument: i32,
            ) -> Result<$ty, DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context).$method($argument)
            }
        )+
    };
}

macro_rules! scalar_update_result_set_filter_chain_methods {
    ($(($index:ident, $label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int, ..)` around-chain。")]
            pub fn $index(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
                value: $ty,
            ) -> Result<(), DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index(column_index, value)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String, ..)` around-chain。")]
            pub fn $label(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
                value: $ty,
            ) -> Result<(), DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label(column_label, value)
            }
        )+
    };
}

macro_rules! resource_update_result_set_filter_chain_methods {
    ($(($index:ident, $label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int, ..)` 资源 around-chain。")]
            pub fn $index(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index(column_index, value)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String, ..)` 资源 around-chain。")]
            pub fn $label(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label(column_label, value)
            }
        )+
    };
}

macro_rules! lob_stream_update_result_set_filter_chain_methods {
    ($((
        $index:ident,
        $label:ident,
        $index_with_length:ident,
        $label_with_length:ident,
        $ty:ty,
        $java:literal
    )),+ $(,)?) => {
        $(
            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int, stream/reader)` around-chain。")]
            pub fn $index(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index(column_index, value)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String, stream/reader)` around-chain。")]
            pub fn $label(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label(column_label, value)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int, stream/reader, long)` around-chain。")]
            pub fn $index_with_length(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
                value: Option<$ty>,
                length: i64,
            ) -> Result<(), DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index_with_length(column_index, value, length)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String, stream/reader, long)` around-chain。")]
            pub fn $label_with_length(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
                value: Option<$ty>,
                length: i64,
            ) -> Result<(), DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label_with_length(column_label, value, length)
            }
        )+
    };
}

macro_rules! stream_update_result_set_filter_chain_methods {
    ($((
        $index:ident,
        $label:ident,
        $index_with_int_length:ident,
        $label_with_int_length:ident,
        $index_with_length:ident,
        $label_with_length:ident,
        $ty:ty,
        $java:literal
    )),+ $(,)?) => {
        $(
            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int, stream/reader)` around-chain。")]
            pub fn $index(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index(column_index, value)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String, stream/reader)` around-chain。")]
            pub fn $label(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label(column_label, value)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int, stream/reader, int)` around-chain。")]
            pub fn $index_with_int_length(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
                value: Option<$ty>,
                length: i32,
            ) -> Result<(), DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index_with_int_length(column_index, value, length)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String, stream/reader, int)` around-chain。")]
            pub fn $label_with_int_length(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
                value: Option<$ty>,
                length: i32,
            ) -> Result<(), DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label_with_int_length(column_label, value, length)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(int, stream/reader, long)` around-chain。")]
            pub fn $index_with_length(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_index: usize,
                value: Option<$ty>,
                length: i64,
            ) -> Result<(), DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$index_with_length(column_index, value, length)
            }

            #[doc = concat!("从位置 0 执行 `ResultSet#", $java, "(String, stream/reader, long)` around-chain。")]
            pub fn $label_with_length(
                &self,
                physical: &dyn PhysicalResultSet,
                context: &ResultSetFilterContext,
                column_label: &str,
                value: Option<$ty>,
                length: i64,
            ) -> Result<(), DruidError> {
                ResultSetFilterChain::new(&self.result_set, physical, context)
                    .$label_with_length(column_label, value, length)
            }
        )+
    };
}

macro_rules! long_stream_update_result_set_filter_chain_methods {
    ($((
        $index:ident,
        $label:ident,
        $index_with_length:ident,
        $label_with_length:ident,
        $ty:ty,
        $java:literal
    )),+ $(,)?) => {
        $(
            lob_stream_update_result_set_filter_chain_methods!((
                $index,
                $label,
                $index_with_length,
                $label_with_length,
                $ty,
                $java
            ));
        )+
    };
}

/// Filter 链。
#[derive(Clone)]
pub struct FilterChainImpl {
    before: Vec<Arc<dyn BeforeFilter>>,
    after: Vec<Arc<dyn AfterFilter>>,
    result_set: Vec<Arc<dyn ResultSetFilter>>,
    filter_class_names: Vec<String>,
}

/// 数据源获取链末端协议。
///
/// 这是 Rust 为避免 core 反向依赖具体 `DruidPool` 引入的支撑协议，对应 Java
/// `FilterChainImpl` 持有的 `DruidDataSource` 引用，不形成额外迁移对象。
#[async_trait::async_trait]
pub trait DataSourceConnectionProvider: Send + Sync {
    /// 返回数据源名称，供 Filter 观察宿主身份。
    fn data_source_name(&self) -> &str;

    /// 返回当前数据源状态快照。
    fn data_source_state(&self) -> PoolState;

    /// 末端直接获取，不再次进入 dataSource_getConnection 链。
    async fn get_connection_direct_for_filter(
        &self,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError>;
}

/// 单次 `dataSource_getConnection` 使用的有位置 around-chain。
///
/// Filter 可以继续、修改 maxWait、短路返回连接或抛错；只有遍历完所有 Filter
/// 才调用 provider 的 direct 入口。
pub struct DataSourceGetConnectionFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    provider: &'a dyn DataSourceConnectionProvider,
}

/// 单次 `dataSource_releaseConnection` 使用的有位置 around-chain。
///
/// 末端才调用池化连接的 canonical recycle 状态机。Filter 成功短路时不会
/// 隐式归还 holder，对齐 Java 自定义 Filter 对连接所有权的责任。
pub struct DataSourceReleaseConnectionFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
}

/// 物理连接关闭时提供给 Filter 的不可变身份与寿命。
///
/// 这是 Java `ConnectionProxy` 在关闭路径上被 Druid Filter 消费的最小语义
/// 投影，不模拟 RDBC 对象，也不形成对外驱动标准。
#[derive(Debug, Clone, Copy)]
pub struct PhysicalConnectionCloseContext {
    /// Druid 分配的物理连接 ID。
    pub connection_id: u64,
    /// 物理连接从建立成功到本次关闭的存活时间。
    pub physical_age: Duration,
}

/// 单次物理连接关闭使用的有位置 around-chain。
///
/// Filter 按注册顺序进入；遍历完成后才调用 `PhysicalConnectionFactory#close`。
/// Filter 可以观察、短路或返回错误，与 Java FilterChain 的所有权规则一致。
pub struct PhysicalConnectionCloseFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    factory: &'a dyn PhysicalConnectionFactory,
    connection: &'a mut Box<dyn PhysicalConnection>,
    context: PhysicalConnectionCloseContext,
}

/// 物理建连 around-chain 的成功结果。
///
/// Java `connection_connect` 返回已经分配 ID 的 `ConnectionProxy`。Rust 在
/// 初始化、校验和入池前暂以阶段对象持有 raw connection，同时保留相同 ID
/// 分配时点。
pub struct PhysicalConnectionConnectResult {
    connection_info: super::PhysicalConnectionInfo,
    connection_id: u64,
}

/// 单次 Clob 操作使用的有位置 Filter around-chain。
///
/// Java 每次 `ClobProxyImpl` 调用都创建新的 `FilterChainImpl`，因此 Rust 也为
/// 每个调用从 position=0 开始，并在末端委托同一 raw Clob。
pub struct ClobFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    wrapper: &'a dyn ClobProxy,
}

impl<'a> ClobFilterChain<'a> {
    fn new(filters: &'a [Arc<dyn BeforeFilter>], wrapper: &'a dyn ClobProxy) -> Self {
        Self {
            filters,
            position: 0,
            wrapper,
        }
    }

    /// 返回当前 Druid Clob Proxy。
    #[must_use]
    pub fn wrapper(&self) -> &dyn ClobProxy {
        self.wrapper
    }

    /// 继续 `Clob#length` 链。
    pub fn clob_length(&mut self) -> Result<i64, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_length(self)
        } else {
            self.wrapper.raw_clob().length()
        }
    }

    /// 继续 `Clob#getSubString` 链。
    pub fn clob_get_sub_string(
        &mut self,
        position: i64,
        length: i32,
    ) -> Result<JavaString, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_get_sub_string(self, position, length)
        } else {
            self.wrapper.raw_clob().get_sub_string(position, length)
        }
    }

    /// 继续无范围 `Clob#getCharacterStream` 链。
    pub fn clob_get_character_stream(&mut self) -> Result<RdbcReader, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_get_character_stream(self)
        } else {
            self.wrapper.raw_clob().get_character_stream()
        }
    }

    /// 继续 `Clob#getAsciiStream` 链。
    pub fn clob_get_ascii_stream(&mut self) -> Result<RdbcInputStream, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_get_ascii_stream(self)
        } else {
            self.wrapper.raw_clob().get_ascii_stream()
        }
    }

    /// 继续 `Clob#position(String,long)` 链。
    pub fn clob_position_string(
        &mut self,
        pattern: &JavaString,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_position_string(self, pattern, start)
        } else {
            self.wrapper.raw_clob().position_string(pattern, start)
        }
    }

    /// 继续 `Clob#position(Clob,long)` 链。
    pub fn clob_position_clob(
        &mut self,
        pattern: &RdbcClob,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_position_clob(self, pattern, start)
        } else {
            self.wrapper.raw_clob().position_clob(pattern, start)
        }
    }

    /// 继续 `Clob#setString(long,String)` 链。
    pub fn clob_set_string(
        &mut self,
        position: i64,
        value: &JavaString,
    ) -> Result<i32, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_set_string(self, position, value)
        } else {
            self.wrapper.raw_clob().set_string(position, value)
        }
    }

    /// 继续 `Clob#setString(long,String,int,int)` 链。
    pub fn clob_set_string_range(
        &mut self,
        position: i64,
        value: &JavaString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_set_string_range(self, position, value, offset, length)
        } else {
            self.wrapper
                .raw_clob()
                .set_string_range(position, value, offset, length)
        }
    }

    /// 继续 `Clob#setAsciiStream` 链。
    pub fn clob_set_ascii_stream(&mut self, position: i64) -> Result<RdbcOutputStream, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_set_ascii_stream(self, position)
        } else {
            self.wrapper.raw_clob().set_ascii_stream(position)
        }
    }

    /// 继续 `Clob#setCharacterStream` 链。
    pub fn clob_set_character_stream(&mut self, position: i64) -> Result<RdbcWriter, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_set_character_stream(self, position)
        } else {
            self.wrapper.raw_clob().set_character_stream(position)
        }
    }

    /// 继续 `Clob#truncate` 链。
    pub fn clob_truncate(&mut self, length: i64) -> Result<(), DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_truncate(self, length)
        } else {
            self.wrapper.raw_clob().truncate(length)
        }
    }

    /// 继续 `Clob#free` 链。
    pub fn clob_free(&mut self) -> Result<(), DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_free(self)
        } else {
            self.wrapper.raw_clob().free()
        }
    }

    /// 继续范围 `Clob#getCharacterStream` 链。
    pub fn clob_get_character_stream_range(
        &mut self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, DruidError> {
        if let Some(filter) = self.next_filter() {
            filter.clob_get_character_stream_range(self, position, length)
        } else {
            self.wrapper
                .raw_clob()
                .get_character_stream_range(position, length)
        }
    }

    fn next_filter(&mut self) -> Option<Arc<dyn BeforeFilter>> {
        let filter = self.filters.get(self.position).cloned();
        if filter.is_some() {
            self.position += 1;
        }
        filter
    }
}

impl PhysicalConnectionConnectResult {
    /// 由无 Filter 的 canonical 池末端构造建连结果。
    pub(crate) const fn new(
        connection_info: super::PhysicalConnectionInfo,
        connection_id: u64,
    ) -> Self {
        Self {
            connection_info,
            connection_id,
        }
    }

    /// 返回本次 raw connection 的 Druid ID。
    #[must_use]
    pub const fn connection_id(&self) -> u64 {
        self.connection_id
    }

    /// 拆分连接阶段对象与 ID。
    #[must_use]
    pub fn into_parts(self) -> (super::PhysicalConnectionInfo, u64) {
        (self.connection_info, self.connection_id)
    }
}

/// 单次真实物理建连使用的有位置 around-chain。
///
/// Filter 可原地修改 Properties、短路返回或传播错误。遍历完成后才执行 driver
/// factory；login timeout 只包围末端 driver future，不包含 Filter 自身耗时。
pub struct PhysicalConnectionConnectFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    factory: &'a dyn PhysicalConnectionFactory,
    login_timeout_seconds: i32,
    next_connection_id: &'a mut (dyn FnMut() -> u64 + Send),
}

impl<'a> PhysicalConnectionConnectFilterChain<'a> {
    fn new(
        filters: &'a [Arc<dyn BeforeFilter>],
        factory: &'a dyn PhysicalConnectionFactory,
        login_timeout_seconds: i32,
        next_connection_id: &'a mut (dyn FnMut() -> u64 + Send),
    ) -> Self {
        Self {
            filters,
            position: 0,
            factory,
            login_timeout_seconds,
            next_connection_id,
        }
    }

    /// 继续执行 `connection_connect` around-chain。
    pub async fn connection_connect(
        &mut self,
        properties: &mut HashMap<String, String>,
    ) -> Result<PhysicalConnectionConnectResult, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.connection_connect(self, properties).await
        } else {
            let connection_info = if self.login_timeout_seconds > 0 {
                match tokio::time::timeout(
                    Duration::from_secs(self.login_timeout_seconds as u64),
                    self.factory.create_info_with_properties(properties),
                )
                .await
                {
                    Ok(result) => result?,
                    Err(_) => return Err(DruidError::LoginTimeout),
                }
            } else {
                self.factory.create_info_with_properties(properties).await?
            };
            let connection_id = (self.next_connection_id)();
            Ok(PhysicalConnectionConnectResult::new(
                connection_info,
                connection_id,
            ))
        }
    }
}

impl<'a> PhysicalConnectionCloseFilterChain<'a> {
    fn new(
        filters: &'a [Arc<dyn BeforeFilter>],
        factory: &'a dyn PhysicalConnectionFactory,
        connection: &'a mut Box<dyn PhysicalConnection>,
        context: PhysicalConnectionCloseContext,
    ) -> Self {
        Self {
            filters,
            position: 0,
            factory,
            connection,
            context,
        }
    }

    /// 返回当前关闭的物理连接身份。
    #[must_use]
    pub fn context(&self) -> PhysicalConnectionCloseContext {
        self.context
    }

    /// 继续执行 `connection_close` around-chain。
    pub async fn connection_close(&mut self) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.connection_close(self).await
        } else {
            self.factory.close(self.connection).await
        }
    }
}

impl<'a> DataSourceReleaseConnectionFilterChain<'a> {
    fn new(filters: &'a [Arc<dyn BeforeFilter>]) -> Self {
        Self {
            filters,
            position: 0,
        }
    }

    /// 继续执行 `dataSource_releaseConnection` around-chain。
    pub async fn data_source_recycle(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter
                .data_source_release_connection(self, connection)
                .await
        } else {
            connection.recycle_from_data_source_filter().await
        }
    }
}

impl<'a> DataSourceGetConnectionFilterChain<'a> {
    fn new(
        filters: &'a [Arc<dyn BeforeFilter>],
        provider: &'a dyn DataSourceConnectionProvider,
    ) -> Self {
        Self {
            filters,
            position: 0,
            provider,
        }
    }

    /// 返回当前数据源名称。
    #[must_use]
    pub fn data_source_name(&self) -> &str {
        self.provider.data_source_name()
    }

    /// 返回当前数据源状态快照。
    #[must_use]
    pub fn data_source_state(&self) -> PoolState {
        self.provider.data_source_state()
    }

    /// 继续执行 `dataSource_getConnection` around-chain。
    pub async fn data_source_get_connection(
        &mut self,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.data_source_get_connection(self, max_wait).await
        } else {
            self.provider
                .get_connection_direct_for_filter(max_wait)
                .await
        }
    }
}

/// 单次 Connection warning 操作使用的有位置 Filter 调用链。
///
/// 对应 Java：`FilterChainImpl#connection_getWarnings` 与
/// `connection_clearWarnings`。每次调用都从位置 0 开始，末端才进入物理连接。
pub struct ConnectionWarningFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    physical: &'a mut dyn PhysicalConnection,
}

/// 单次 Connection LOB 创建使用的有位置 Filter 调用链。
///
/// 对应 Java `FilterChainImpl#connection_createBlob/createClob/createNClob`。
/// 每个公开调用都创建新链并从位置 0 开始；Filter 可以替换返回句柄、短路或
/// 返回错误，遍历完成后才进入同一物理连接。
pub struct ConnectionLobFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    physical: &'a mut dyn PhysicalConnection,
}

impl<'a> ConnectionLobFilterChain<'a> {
    fn new(filters: &'a [Arc<dyn BeforeFilter>], physical: &'a mut dyn PhysicalConnection) -> Self {
        Self {
            filters,
            position: 0,
            physical,
        }
    }

    /// 继续分派 `Connection#createBlob()`。
    pub async fn connection_create_blob(&mut self) -> Result<RdbcBlob, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.connection_create_blob(self).await
        } else {
            self.physical.create_blob().await
        }
    }

    /// 继续分派 `Connection#createClob()`。
    pub async fn connection_create_clob(&mut self) -> Result<RdbcClob, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.connection_create_clob(self).await
        } else {
            self.physical.create_clob().await
        }
    }

    /// 继续分派 `Connection#createNClob()`。
    pub async fn connection_create_n_clob(&mut self) -> Result<RdbcNClob, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.connection_create_n_clob(self).await
        } else {
            self.physical.create_n_clob().await
        }
    }
}

/// 单次 `Connection#getMetaData()` 使用的有位置 Filter 调用链。
///
/// 两个生命周期分别约束 Filter 注册表和物理连接借用；返回 metadata 只绑定
/// 物理连接，不能因 FilterChain 临时对象结束而失效或被提升为 `'static`。
pub struct ConnectionDatabaseMetaDataFilterChain<'filters, 'connection> {
    filters: &'filters [Arc<dyn BeforeFilter>],
    position: usize,
    physical: &'connection mut dyn PhysicalConnection,
    connection_id: u64,
}

impl<'filters, 'connection> ConnectionDatabaseMetaDataFilterChain<'filters, 'connection> {
    fn new(
        filters: &'filters [Arc<dyn BeforeFilter>],
        physical: &'connection mut dyn PhysicalConnection,
        connection_id: u64,
    ) -> Self {
        Self {
            filters,
            position: 0,
            physical,
            connection_id,
        }
    }

    /// 返回当前 Druid 连接 ID，供 Wall/Stat 等 Filter 判定。
    #[must_use]
    pub const fn connection_id(&self) -> u64 {
        self.connection_id
    }

    /// 继续分派 `Connection#getMetaData()`，末端借用物理 metadata。
    pub fn connection_get_meta_data(
        mut self,
    ) -> Result<Box<dyn PhysicalDatabaseMetaData + 'connection>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.connection_get_meta_data(self)
        } else {
            self.physical.database_meta_data()
        }
    }
}

impl<'a> ConnectionWarningFilterChain<'a> {
    fn new(filters: &'a [Arc<dyn BeforeFilter>], physical: &'a mut dyn PhysicalConnection) -> Self {
        Self {
            filters,
            position: 0,
            physical,
        }
    }

    /// 继续分派 `Connection#getWarnings()`。
    pub async fn connection_get_warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.connection_get_warnings(self).await
        } else {
            self.physical.warnings().await
        }
    }

    /// 继续分派 `Connection#clearWarnings()`。
    pub async fn connection_clear_warnings(&mut self) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.connection_clear_warnings(self).await
        } else {
            self.physical.clear_warnings().await
        }
    }
}

/// 单次 Statement warning 操作使用的有位置 Filter 调用链。
///
/// 对应 Java：`FilterChainImpl#statement_getWarnings` 与
/// `statement_clearWarnings`。PreparedStatement 使用同一 Java 继承语义。
pub struct StatementWarningFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    physical: StatementWarningTarget<'a>,
}

enum StatementWarningTarget<'a> {
    Statement(&'a dyn PhysicalStatement),
    PreparedStatement(&'a dyn PhysicalPreparedStatement),
}

impl<'a> StatementWarningFilterChain<'a> {
    fn new_statement(
        filters: &'a [Arc<dyn BeforeFilter>],
        physical: &'a dyn PhysicalStatement,
    ) -> Self {
        Self {
            filters,
            position: 0,
            physical: StatementWarningTarget::Statement(physical),
        }
    }

    fn new_prepared_statement(
        filters: &'a [Arc<dyn BeforeFilter>],
        physical: &'a dyn PhysicalPreparedStatement,
    ) -> Self {
        Self {
            filters,
            position: 0,
            physical: StatementWarningTarget::PreparedStatement(physical),
        }
    }

    /// 继续分派 `Statement#getWarnings()`。
    pub async fn statement_get_warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.statement_get_warnings(self).await
        } else {
            match self.physical {
                StatementWarningTarget::Statement(physical) => physical.warnings(),
                StatementWarningTarget::PreparedStatement(physical) => physical.warnings(),
            }
        }
    }

    /// 继续分派 `Statement#clearWarnings()`。
    pub async fn statement_clear_warnings(&mut self) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.statement_clear_warnings(self).await
        } else {
            match self.physical {
                StatementWarningTarget::Statement(physical) => physical.clear_warnings(),
                StatementWarningTarget::PreparedStatement(physical) => physical.clear_warnings(),
            }
        }
    }
}

impl FilterChainImpl {
    pub fn new() -> Self {
        Self {
            before: Vec::new(),
            after: Vec::new(),
            result_set: Vec::new(),
            filter_class_names: Vec::new(),
        }
    }

    /// 从位置 0 执行一次数据源连接获取 around-chain。
    pub async fn data_source_get_connection(
        &self,
        provider: &dyn DataSourceConnectionProvider,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        DataSourceGetConnectionFilterChain::new(&self.before, provider)
            .data_source_get_connection(max_wait)
            .await
    }

    /// 从位置 0 执行一次数据源连接归还 around-chain。
    pub async fn data_source_release_connection(
        &self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        DataSourceReleaseConnectionFilterChain::new(&self.before)
            .data_source_recycle(connection)
            .await
    }

    /// 从位置 0 执行一次真实物理连接关闭 around-chain。
    pub async fn physical_connection_close(
        &self,
        factory: &dyn PhysicalConnectionFactory,
        connection: &mut Box<dyn PhysicalConnection>,
        connection_id: u64,
        physical_age: Duration,
    ) -> Result<(), DruidError> {
        let context = PhysicalConnectionCloseContext {
            connection_id,
            physical_age,
        };
        PhysicalConnectionCloseFilterChain::new(&self.before, factory, connection, context)
            .connection_close()
            .await
    }

    /// 从位置 0 执行一次真实物理建连 around-chain。
    pub async fn physical_connection_connect(
        &self,
        factory: &dyn PhysicalConnectionFactory,
        properties: &mut HashMap<String, String>,
        login_timeout_seconds: i32,
        next_connection_id: &mut (dyn FnMut() -> u64 + Send),
    ) -> Result<PhysicalConnectionConnectResult, DruidError> {
        PhysicalConnectionConnectFilterChain::new(
            &self.before,
            factory,
            login_timeout_seconds,
            next_connection_id,
        )
        .connection_connect(properties)
        .await
    }

    /// 从位置 0 执行 `Clob#length()`。
    pub fn clob_length(&self, wrapper: &dyn ClobProxy) -> Result<i64, DruidError> {
        ClobFilterChain::new(&self.before, wrapper).clob_length()
    }

    /// 从位置 0 执行 `Clob#getSubString(long,int)`。
    pub fn clob_get_sub_string(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
        length: i32,
    ) -> Result<JavaString, DruidError> {
        ClobFilterChain::new(&self.before, wrapper).clob_get_sub_string(position, length)
    }

    /// 从位置 0 执行 `Clob#getCharacterStream()`。
    pub fn clob_get_character_stream(
        &self,
        wrapper: &dyn ClobProxy,
    ) -> Result<RdbcReader, DruidError> {
        ClobFilterChain::new(&self.before, wrapper).clob_get_character_stream()
    }

    /// 从位置 0 执行 `Clob#getAsciiStream()`。
    pub fn clob_get_ascii_stream(
        &self,
        wrapper: &dyn ClobProxy,
    ) -> Result<RdbcInputStream, DruidError> {
        ClobFilterChain::new(&self.before, wrapper).clob_get_ascii_stream()
    }

    /// 从位置 0 执行 `Clob#position(String,long)`。
    pub fn clob_position_string(
        &self,
        wrapper: &dyn ClobProxy,
        pattern: &JavaString,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        ClobFilterChain::new(&self.before, wrapper).clob_position_string(pattern, start)
    }

    /// 从位置 0 执行 `Clob#position(Clob,long)`。
    pub fn clob_position_clob(
        &self,
        wrapper: &dyn ClobProxy,
        pattern: &RdbcClob,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        ClobFilterChain::new(&self.before, wrapper).clob_position_clob(pattern, start)
    }

    /// 从位置 0 执行 `Clob#setString(long,String)`。
    pub fn clob_set_string(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
        value: &JavaString,
    ) -> Result<i32, DruidError> {
        ClobFilterChain::new(&self.before, wrapper).clob_set_string(position, value)
    }

    /// 从位置 0 执行 `Clob#setString(long,String,int,int)`。
    pub fn clob_set_string_range(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
        value: &JavaString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_set_string_range(position, value, offset, length)
    }

    /// 从位置 0 执行 `Clob#setAsciiStream(long)`。
    pub fn clob_set_ascii_stream(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
    ) -> Result<RdbcOutputStream, DruidError> {
        ClobFilterChain::new(&self.before, wrapper).clob_set_ascii_stream(position)
    }

    /// 从位置 0 执行 `Clob#setCharacterStream(long)`。
    pub fn clob_set_character_stream(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
    ) -> Result<RdbcWriter, DruidError> {
        ClobFilterChain::new(&self.before, wrapper).clob_set_character_stream(position)
    }

    /// 从位置 0 执行 `Clob#truncate(long)`。
    pub fn clob_truncate(&self, wrapper: &dyn ClobProxy, length: i64) -> Result<(), DruidError> {
        ClobFilterChain::new(&self.before, wrapper).clob_truncate(length)
    }

    /// 从位置 0 执行 `Clob#free()`。
    pub fn clob_free(&self, wrapper: &dyn ClobProxy) -> Result<(), DruidError> {
        ClobFilterChain::new(&self.before, wrapper).clob_free()
    }

    /// 从位置 0 执行 `Clob#getCharacterStream(long,long)`。
    pub fn clob_get_character_stream_range(
        &self,
        wrapper: &dyn ClobProxy,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, DruidError> {
        ClobFilterChain::new(&self.before, wrapper)
            .clob_get_character_stream_range(position, length)
    }

    /// 初始化链中每个 Java Filter 实例。
    ///
    /// 对应 Java：`DruidDataSource#init()` 按注册顺序调用
    /// `Filter#init(DataSourceProxy)`。三个 Rust trait 视图共享同一实例，
    /// 因此生命周期只通过 `BeforeFilter` 视图调用一次。
    ///
    /// # Errors
    ///
    /// Rust Filter 把 Java 无返回值生命周期扩展为可失败操作；任一初始化失败时
    /// 原样返回该错误，并停止初始化后续 Filter。
    pub(crate) async fn init_filters(&self) -> Result<(), DruidError> {
        for filter in &self.before {
            filter.init().await?;
        }
        Ok(())
    }

    /// 按注册顺序向当前显式 Filter 应用连接属性。
    ///
    /// 对应 Java：`DruidDataSource#setConnectProperties` 中逐项调用
    /// `Filter#configFromProperties`。自动 provider 在 Java 中晚于该阶段追加，
    /// 因而不会被本方法回溯配置。
    pub(crate) fn configure_filters(
        &self,
        properties: &std::collections::HashMap<String, String>,
        system_properties: &std::collections::HashMap<String, String>,
    ) -> Result<(), DruidError> {
        for filter in &self.before {
            filter.config_from_properties(properties)?;
            filter.config_from_system_properties(system_properties)?;
        }
        Ok(())
    }

    /// 销毁链中每个 Java Filter 实例。
    ///
    /// 对应 Java：`DruidDataSource#close()` 按注册顺序调用
    /// `Filter#destroy()`。Java `destroy` 不抛受检异常；Rust 扩展错误只记录，
    /// 不能中断其余 Filter 的清理。
    pub(crate) async fn destroy_filters(&self) {
        for filter in &self.before {
            if let Err(error) = filter.destroy().await {
                tracing::warn!(%error, filter = filter.name(), "destroy filter error");
            }
        }
    }

    /// 返回链中是否没有任何 Java Filter。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.before.is_empty()
            && self.after.is_empty()
            && self.result_set.is_empty()
            && self.filter_class_names.is_empty()
    }
    pub fn add_before(&mut self, filter: Arc<dyn BeforeFilter>) {
        self.before.push(filter);
    }
    pub fn add_after(&mut self, filter: Arc<dyn AfterFilter>) {
        self.after.push(filter);
    }
    /// 添加同步 `ResultSet Filter`，保持 Java `Filter` 注册顺序。
    pub fn add_result_set(&mut self, filter: Arc<dyn ResultSetFilter>) {
        self.result_set.push(filter);
    }

    /// 把同一个 Java 风格 Filter 实例注册到全部已迁移的调用族。
    ///
    /// Java 一个 `Filter` 对象同时接收 before、after 与 ResultSet 方法；该入口
    /// 保证三个 Rust trait 视图共享同一实例与注册位置，避免调用方漏接其中一族。
    pub fn add_filter<T>(&mut self, filter: Arc<T>)
    where
        T: BeforeFilter + AfterFilter + ResultSetFilter + 'static,
    {
        self.filter_class_names
            .push(std::any::type_name::<T>().to_string());
        self.before
            .push(Arc::clone(&filter) as Arc<dyn BeforeFilter>);
        self.after.push(Arc::clone(&filter) as Arc<dyn AfterFilter>);
        self.result_set.push(filter);
    }

    /// 添加由 `FilterManager` 构造的同一 Filter 三个动态 trait 视图。
    ///
    /// `filter_class_name` 保存 Java 完整类名，供 `existsFilter` 语义去重。
    pub(crate) fn add_registered_filter(
        &mut self,
        filter_class_name: String,
        before: Arc<dyn BeforeFilter>,
        after: Arc<dyn AfterFilter>,
        result_set: Arc<dyn ResultSetFilter>,
    ) {
        self.filter_class_names.push(filter_class_name);
        self.before.push(before);
        self.after.push(after);
        self.result_set.push(result_set);
    }

    /// 判断链中是否已有指定 Java Filter 类名。
    ///
    /// 对应 Java：`FilterManager#existsFilter`，比较时忽略 ASCII 大小写。
    #[must_use]
    pub fn contains_filter_class_name(&self, filter_class_name: &str) -> bool {
        self.filter_class_names
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(filter_class_name))
    }

    /// 返回按注册顺序保存的 Java Filter 类名。
    #[must_use]
    pub fn filter_class_names(&self) -> &[String] {
        &self.filter_class_names
    }
    pub fn before_count(&self) -> usize {
        self.before.len()
    }
    pub fn after_count(&self) -> usize {
        self.after.len()
    }
    /// 返回 `ResultSet Filter` 数。
    pub fn result_set_count(&self) -> usize {
        self.result_set.len()
    }

    /// 在查询成功并建立结果集后按调用栈逆序执行 open-after hook。
    pub fn result_set_open_after(
        &self,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        for filter in self.result_set.iter().rev() {
            filter.result_set_open_after(context)?;
        }
        Ok(())
    }

    /// 在 ResultSet 代理构造边界执行可变 open-after 链。
    ///
    /// 默认桥接只读 hook，保持第三方 `ResultSetFilter` 源码兼容。
    pub fn result_set_open_after_with_proxy(
        &self,
        context: &mut ResultSetOpenContext<'_>,
    ) -> Result<(), DruidError> {
        for filter in self.result_set.iter().rev() {
            filter.result_set_open_after_with_proxy(context)?;
        }
        Ok(())
    }

    /// 从位置 0 执行一次完整的 ResultSet next around-chain。
    pub fn result_set_next(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context).result_set_next()
    }

    /// 从位置 0 执行一次完整的 ResultSet close around-chain。
    pub fn result_set_close(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context).result_set_close()
    }

    /// 从位置 0 执行一次 `ResultSet#getWarnings()` around-chain。
    pub fn result_set_warnings(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<Option<SqlWarning>, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context).result_set_get_warnings()
    }

    /// 从位置 0 执行一次 `ResultSet#clearWarnings()` around-chain。
    pub fn result_set_clear_warnings(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context).result_set_clear_warnings()
    }

    /// 从位置 0 执行 `ResultSet#getObject(int)` around-chain。
    pub fn result_set_get_object(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_index: usize,
    ) -> Result<Value, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object(column_index)
    }

    /// 从位置 0 执行 `ResultSet#getObject(String)` around-chain。
    pub fn result_set_get_object_by_label(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
    ) -> Result<Value, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object_by_label(column_label)
    }

    /// 从位置 0 执行 `ResultSet#getObject(int, Map)` around-chain。
    pub fn result_set_get_object_with_type_map(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_index: usize,
        type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object_with_type_map(column_index, type_map)
    }

    /// 从位置 0 执行 `ResultSet#getObject(String, Map)` around-chain。
    pub fn result_set_get_object_by_label_with_type_map(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
        type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object_by_label_with_type_map(column_label, type_map)
    }

    /// 从位置 0 执行 `ResultSet#getObject(int, Class<T>)` around-chain。
    pub fn result_set_get_object_typed(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_index: usize,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object_typed(column_index, target_type)
    }

    /// 从位置 0 执行 `ResultSet#getObject(String, Class<T>)` around-chain。
    pub fn result_set_get_object_typed_by_label(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object_typed_by_label(column_label, target_type)
    }

    scalar_result_set_filter_chain_methods!(
        (
            result_set_get_string,
            result_set_get_string_by_label,
            Option<String>,
            "getString"
        ),
        (
            result_set_get_boolean,
            result_set_get_boolean_by_label,
            bool,
            "getBoolean"
        ),
        (
            result_set_get_byte,
            result_set_get_byte_by_label,
            i8,
            "getByte"
        ),
        (
            result_set_get_short,
            result_set_get_short_by_label,
            i16,
            "getShort"
        ),
        (
            result_set_get_int,
            result_set_get_int_by_label,
            i32,
            "getInt"
        ),
        (
            result_set_get_long,
            result_set_get_long_by_label,
            i64,
            "getLong"
        ),
        (
            result_set_get_float,
            result_set_get_float_by_label,
            f32,
            "getFloat"
        ),
        (
            result_set_get_double,
            result_set_get_double_by_label,
            f64,
            "getDouble"
        ),
        (
            result_set_get_bytes,
            result_set_get_bytes_by_label,
            Option<Vec<u8>>,
            "getBytes"
        ),
        (
            result_set_get_n_string,
            result_set_get_n_string_by_label,
            Option<String>,
            "getNString"
        ),
    );

    /// 从位置 0 执行 `ResultSet#getBigDecimal(int)` around-chain。
    pub fn result_set_get_big_decimal(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_index: usize,
    ) -> Result<Option<BigDecimal>, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_big_decimal(column_index)
    }

    /// 从位置 0 执行 `ResultSet#getBigDecimal(String)` around-chain。
    pub fn result_set_get_big_decimal_by_label(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
    ) -> Result<Option<BigDecimal>, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_big_decimal_by_label(column_label)
    }

    /// 从位置 0 执行 `ResultSet#getBigDecimal(int, int)` around-chain。
    pub fn result_set_get_big_decimal_with_scale(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_index: usize,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_big_decimal_with_scale(column_index, scale)
    }

    /// 从位置 0 执行 `ResultSet#getBigDecimal(String, int)` around-chain。
    pub fn result_set_get_big_decimal_by_label_with_scale(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_big_decimal_by_label_with_scale(column_label, scale)
    }

    temporal_result_set_filter_chain_methods!(
        (
            result_set_get_date,
            result_set_get_date_by_label,
            result_set_get_date_with_calendar,
            result_set_get_date_by_label_with_calendar,
            NaiveDate,
            "getDate"
        ),
        (
            result_set_get_time,
            result_set_get_time_by_label,
            result_set_get_time_with_calendar,
            result_set_get_time_by_label_with_calendar,
            NaiveTime,
            "getTime"
        ),
        (
            result_set_get_timestamp,
            result_set_get_timestamp_by_label,
            result_set_get_timestamp_with_calendar,
            result_set_get_timestamp_by_label_with_calendar,
            NaiveDateTime,
            "getTimestamp"
        ),
    );

    resource_result_set_filter_chain_methods!(
        (
            result_set_get_ref,
            result_set_get_ref_by_label,
            RdbcRef,
            "getRef"
        ),
        (
            result_set_get_blob,
            result_set_get_blob_by_label,
            RdbcBlob,
            "getBlob"
        ),
        (
            result_set_get_clob,
            result_set_get_clob_by_label,
            RdbcClob,
            "getClob"
        ),
        (
            result_set_get_array,
            result_set_get_array_by_label,
            RdbcArray,
            "getArray"
        ),
        (
            result_set_get_url,
            result_set_get_url_by_label,
            RdbcUrl,
            "getURL"
        ),
        (
            result_set_get_row_id,
            result_set_get_row_id_by_label,
            RdbcRowId,
            "getRowId"
        ),
        (
            result_set_get_n_clob,
            result_set_get_n_clob_by_label,
            RdbcNClob,
            "getNClob"
        ),
        (
            result_set_get_sql_xml,
            result_set_get_sql_xml_by_label,
            RdbcSqlXml,
            "getSQLXML"
        ),
        (
            result_set_get_ascii_stream,
            result_set_get_ascii_stream_by_label,
            RdbcInputStream,
            "getAsciiStream"
        ),
        (
            result_set_get_unicode_stream,
            result_set_get_unicode_stream_by_label,
            RdbcInputStream,
            "getUnicodeStream"
        ),
        (
            result_set_get_binary_stream,
            result_set_get_binary_stream_by_label,
            RdbcInputStream,
            "getBinaryStream"
        ),
        (
            result_set_get_character_stream,
            result_set_get_character_stream_by_label,
            RdbcReader,
            "getCharacterStream"
        ),
        (
            result_set_get_n_character_stream,
            result_set_get_n_character_stream_by_label,
            RdbcReader,
            "getNCharacterStream"
        ),
    );

    no_arg_result_set_filter_chain_methods!(
        (result_set_was_null, bool, "wasNull"),
        (result_set_previous, bool, "previous"),
        (result_set_is_before_first, bool, "isBeforeFirst"),
        (result_set_is_after_last, bool, "isAfterLast"),
        (result_set_is_first, bool, "isFirst"),
        (result_set_is_last, bool, "isLast"),
        (result_set_before_first, (), "beforeFirst"),
        (result_set_after_last, (), "afterLast"),
        (result_set_first, bool, "first"),
        (result_set_last, bool, "last"),
        (result_set_get_row, i32, "getRow"),
        (result_set_get_fetch_direction, i32, "getFetchDirection"),
        (result_set_get_fetch_size, i32, "getFetchSize"),
        (result_set_get_type, i32, "getType"),
        (result_set_get_concurrency, i32, "getConcurrency"),
        (result_set_get_holdability, i32, "getHoldability"),
        (result_set_get_cursor_name, Option<String>, "getCursorName"),
        (result_set_row_updated, bool, "rowUpdated"),
        (result_set_row_inserted, bool, "rowInserted"),
        (result_set_row_deleted, bool, "rowDeleted"),
        (result_set_insert_row, (), "insertRow"),
        (result_set_update_row, (), "updateRow"),
        (result_set_delete_row, (), "deleteRow"),
        (result_set_refresh_row, (), "refreshRow"),
        (result_set_cancel_row_updates, (), "cancelRowUpdates"),
        (result_set_move_to_insert_row, (), "moveToInsertRow"),
        (result_set_move_to_current_row, (), "moveToCurrentRow"),
        (result_set_is_closed, bool, "isClosed"),
    );

    i32_arg_result_set_filter_chain_methods!(
        (result_set_absolute, row, bool, "absolute"),
        (result_set_relative, rows, bool, "relative"),
        (
            result_set_set_fetch_direction,
            direction,
            (),
            "setFetchDirection"
        ),
        (result_set_set_fetch_size, rows, (), "setFetchSize"),
    );

    /// 从位置 0 执行 `ResultSet#updateNull(int)` around-chain。
    pub fn result_set_update_null(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_index: usize,
    ) -> Result<(), DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_update_null(column_index)
    }

    /// 从位置 0 执行 `ResultSet#updateNull(String)` around-chain。
    pub fn result_set_update_null_by_label(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
    ) -> Result<(), DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_update_null_by_label(column_label)
    }

    scalar_update_result_set_filter_chain_methods!(
        (result_set_update_boolean, result_set_update_boolean_by_label, bool, "updateBoolean"),
        (result_set_update_byte, result_set_update_byte_by_label, i8, "updateByte"),
        (result_set_update_short, result_set_update_short_by_label, i16, "updateShort"),
        (result_set_update_int, result_set_update_int_by_label, i32, "updateInt"),
        (result_set_update_long, result_set_update_long_by_label, i64, "updateLong"),
        (result_set_update_float, result_set_update_float_by_label, f32, "updateFloat"),
        (result_set_update_double, result_set_update_double_by_label, f64, "updateDouble"),
        (result_set_update_big_decimal, result_set_update_big_decimal_by_label, Option<BigDecimal>, "updateBigDecimal"),
        (result_set_update_string, result_set_update_string_by_label, Option<String>, "updateString"),
        (result_set_update_n_string, result_set_update_n_string_by_label, Option<String>, "updateNString"),
        (result_set_update_bytes, result_set_update_bytes_by_label, Option<Vec<u8>>, "updateBytes"),
        (result_set_update_date, result_set_update_date_by_label, Option<NaiveDate>, "updateDate"),
        (result_set_update_time, result_set_update_time_by_label, Option<NaiveTime>, "updateTime"),
        (result_set_update_timestamp, result_set_update_timestamp_by_label, Option<NaiveDateTime>, "updateTimestamp"),
    );

    /// 从位置 0 执行 `ResultSet#updateObject(int, Object)` around-chain。
    pub fn result_set_update_object(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_index: usize,
        value: RdbcObject,
    ) -> Result<(), DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_update_object(column_index, value)
    }

    /// 从位置 0 执行 `ResultSet#updateObject(String, Object)` around-chain。
    pub fn result_set_update_object_by_label(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
        value: RdbcObject,
    ) -> Result<(), DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_update_object_by_label(column_label, value)
    }

    /// 从位置 0 执行 `ResultSet#updateObject(int, Object, int)` around-chain。
    pub fn result_set_update_object_with_scale_or_length(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_index: usize,
        value: RdbcObject,
        scale_or_length: i32,
    ) -> Result<(), DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_update_object_with_scale_or_length(column_index, value, scale_or_length)
    }

    /// 从位置 0 执行 `ResultSet#updateObject(String, Object, int)` around-chain。
    pub fn result_set_update_object_by_label_with_scale_or_length(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
        value: RdbcObject,
        scale_or_length: i32,
    ) -> Result<(), DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_update_object_by_label_with_scale_or_length(
                column_label,
                value,
                scale_or_length,
            )
    }

    resource_update_result_set_filter_chain_methods!(
        (
            result_set_update_reference,
            result_set_update_reference_by_label,
            RdbcRef,
            "updateRef"
        ),
        (
            result_set_update_blob,
            result_set_update_blob_by_label,
            RdbcBlob,
            "updateBlob"
        ),
        (
            result_set_update_clob,
            result_set_update_clob_by_label,
            RdbcClob,
            "updateClob"
        ),
        (
            result_set_update_array,
            result_set_update_array_by_label,
            RdbcArray,
            "updateArray"
        ),
        (
            result_set_update_row_id,
            result_set_update_row_id_by_label,
            RdbcRowId,
            "updateRowId"
        ),
        (
            result_set_update_n_clob,
            result_set_update_n_clob_by_label,
            RdbcNClob,
            "updateNClob"
        ),
        (
            result_set_update_sql_xml,
            result_set_update_sql_xml_by_label,
            RdbcSqlXml,
            "updateSQLXML"
        ),
    );

    lob_stream_update_result_set_filter_chain_methods!(
        (
            result_set_update_blob_stream,
            result_set_update_blob_stream_by_label,
            result_set_update_blob_stream_with_length,
            result_set_update_blob_stream_by_label_with_length,
            RdbcInputStream,
            "updateBlob"
        ),
        (
            result_set_update_clob_reader,
            result_set_update_clob_reader_by_label,
            result_set_update_clob_reader_with_length,
            result_set_update_clob_reader_by_label_with_length,
            RdbcReader,
            "updateClob"
        ),
        (
            result_set_update_n_clob_reader,
            result_set_update_n_clob_reader_by_label,
            result_set_update_n_clob_reader_with_length,
            result_set_update_n_clob_reader_by_label_with_length,
            RdbcReader,
            "updateNClob"
        ),
    );

    stream_update_result_set_filter_chain_methods!(
        (
            result_set_update_ascii_stream,
            result_set_update_ascii_stream_by_label,
            result_set_update_ascii_stream_with_int_length,
            result_set_update_ascii_stream_by_label_with_int_length,
            result_set_update_ascii_stream_with_length,
            result_set_update_ascii_stream_by_label_with_length,
            RdbcInputStream,
            "updateAsciiStream"
        ),
        (
            result_set_update_binary_stream,
            result_set_update_binary_stream_by_label,
            result_set_update_binary_stream_with_int_length,
            result_set_update_binary_stream_by_label_with_int_length,
            result_set_update_binary_stream_with_length,
            result_set_update_binary_stream_by_label_with_length,
            RdbcInputStream,
            "updateBinaryStream"
        ),
        (
            result_set_update_character_stream,
            result_set_update_character_stream_by_label,
            result_set_update_character_stream_with_int_length,
            result_set_update_character_stream_by_label_with_int_length,
            result_set_update_character_stream_with_length,
            result_set_update_character_stream_by_label_with_length,
            RdbcReader,
            "updateCharacterStream"
        ),
    );

    long_stream_update_result_set_filter_chain_methods!((
        result_set_update_n_character_stream,
        result_set_update_n_character_stream_by_label,
        result_set_update_n_character_stream_with_length,
        result_set_update_n_character_stream_by_label_with_length,
        RdbcReader,
        "updateNCharacterStream"
    ));

    /// 从位置 0 执行 `ResultSet#findColumn(String)` around-chain。
    pub fn result_set_find_column(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
    ) -> Result<usize, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_find_column(column_label)
    }

    /// 从位置 0 执行 `ResultSet#getMetaData()` around-chain。
    pub fn result_set_get_meta_data(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<ResultSetMetaData, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context).result_set_get_meta_data()
    }

    /// 从位置 0 执行 `ResultSet#getStatement()` around-chain。
    pub fn result_set_get_statement(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        statement: &ResultSetStatement,
    ) -> Result<ResultSetStatement, DruidError> {
        ResultSetFilterChain::new_with_statement(&self.result_set, physical, context, statement)
            .result_set_get_statement()
    }

    pub async fn before_execute(&self, ctx: &mut ExecContext<'_>) -> Result<(), DruidError> {
        for (index, filter) in self.before.iter().enumerate() {
            if let Err(error) = filter.before(ctx).await {
                for completed in self.before[..index].iter().rev() {
                    if let Err(cleanup_error) = completed.before_execute_error(ctx, &error).await {
                        tracing::warn!(
                            %cleanup_error,
                            "Filter before-error cleanup failed; preserving primary error"
                        );
                    }
                }
                return Err(error);
            }
        }
        Ok(())
    }

    /// 按 Filter 注册顺序展开 PreparedStatement/CallableStatement SQL 改写链。
    ///
    /// 每个 Filter 观察前一个 Filter 的输出；最终文本同时进入物理 prepare 与
    /// `PreparedStatementKey`，避免缓存键和实际驱动语句分裂。
    pub fn prepare_statement_sql(&self, sql: &str) -> Result<String, DruidError> {
        let mut current = sql.to_owned();
        for filter in &self.before {
            current = filter.prepare_statement_sql(&current)?;
        }
        Ok(current)
    }

    /// 按 Filter 注册顺序展开普通 Statement addBatch SQL 改写链。
    pub fn statement_add_batch_sql(&self, sql: &str) -> Result<String, DruidError> {
        let mut current = sql.to_owned();
        for filter in &self.before {
            current = filter.statement_add_batch_sql(&current)?;
        }
        Ok(current)
    }

    /// 在 Statement/PreparedStatement/CallableStatement 创建成功后逆序展开事件。
    ///
    /// 对应 Java `FilterEventAdapter` 的三个 create/prepare after 模板：最先
    /// 注册的 Filter 最外层包围调用，因此下游成功后最后收到 after 事件。
    pub async fn after_statement_event(&self, event: &StatementEvent) -> Result<(), DruidError> {
        self.after_statement_event_with_identity(0, 0, event).await
    }

    /// 在 Statement 创建成功后携带真实 Proxy 身份逆序展开事件。
    pub async fn after_statement_event_with_identity(
        &self,
        connection_id: u64,
        statement_id: u64,
        event: &StatementEvent,
    ) -> Result<(), DruidError> {
        let context = StatementEventContext {
            connection_id,
            statement_id,
            event,
        };
        for filter in self.before.iter().rev() {
            filter.on_statement_event_context(&context).await?;
        }
        Ok(())
    }

    /// 在同步 Statement 关闭成功后携带真实 Proxy 身份逆序展开事件。
    pub fn after_statement_close_with_identity(
        &self,
        connection_id: u64,
        statement_id: u64,
    ) -> Result<(), DruidError> {
        let event = StatementEvent::Close;
        let context = StatementEventContext {
            connection_id,
            statement_id,
            event: &event,
        };
        for filter in self.before.iter().rev() {
            filter.on_statement_close_context(&context)?;
        }
        Ok(())
    }

    /// 按 Java Filter 注册顺序执行一次 batch 前置链。
    pub async fn before_batch(&self, context: &mut BatchExecContext<'_>) -> Result<(), DruidError> {
        for (index, filter) in self.before.iter().enumerate() {
            if let Err(error) = filter.before_batch(context).await {
                for completed in self.before[..index].iter().rev() {
                    if let Err(cleanup_error) = completed.before_batch_error(context, &error).await
                    {
                        tracing::warn!(
                            %cleanup_error,
                            "Filter batch before-error cleanup failed; preserving primary error"
                        );
                    }
                }
                return Err(error);
            }
        }
        Ok(())
    }

    /// 按注册顺序执行连接事件前置过滤器。
    pub async fn before_connection_event(&self, event: &ConnectionEvent) -> Result<(), DruidError> {
        self.before_connection_event_with_identity(0, event).await
    }

    /// 按注册顺序执行携带真实连接 ID 的前置事件。
    pub async fn before_connection_event_with_identity(
        &self,
        connection_id: u64,
        event: &ConnectionEvent,
    ) -> Result<(), DruidError> {
        let context = ConnectionEventContext {
            connection_id,
            event,
        };
        for filter in &self.before {
            filter.on_connection_event_context(&context).await?;
        }
        Ok(())
    }

    /// 从位置 0 执行一次 `Connection#getWarnings()` around-chain。
    pub async fn connection_warnings(
        &self,
        physical: &mut dyn PhysicalConnection,
    ) -> Result<Option<SqlWarning>, DruidError> {
        ConnectionWarningFilterChain::new(&self.before, physical)
            .connection_get_warnings()
            .await
    }

    /// 从位置 0 执行一次 `Connection#clearWarnings()` around-chain。
    pub async fn connection_clear_warnings(
        &self,
        physical: &mut dyn PhysicalConnection,
    ) -> Result<(), DruidError> {
        ConnectionWarningFilterChain::new(&self.before, physical)
            .connection_clear_warnings()
            .await
    }

    /// 从位置 0 执行一次 `Connection#createBlob()` around-chain。
    pub async fn connection_create_blob(
        &self,
        physical: &mut dyn PhysicalConnection,
    ) -> Result<RdbcBlob, DruidError> {
        ConnectionLobFilterChain::new(&self.before, physical)
            .connection_create_blob()
            .await
    }

    /// 从位置 0 执行一次 `Connection#createClob()` around-chain。
    pub async fn connection_create_clob(
        &self,
        physical: &mut dyn PhysicalConnection,
    ) -> Result<RdbcClob, DruidError> {
        ConnectionLobFilterChain::new(&self.before, physical)
            .connection_create_clob()
            .await
    }

    /// 从位置 0 执行一次 `Connection#createNClob()` around-chain。
    pub async fn connection_create_n_clob(
        &self,
        physical: &mut dyn PhysicalConnection,
    ) -> Result<RdbcNClob, DruidError> {
        ConnectionLobFilterChain::new(&self.before, physical)
            .connection_create_n_clob()
            .await
    }

    /// 从位置 0 执行一次 `Connection#getMetaData()` around-chain。
    pub fn connection_database_meta_data<'connection>(
        &self,
        physical: &'connection mut dyn PhysicalConnection,
        connection_id: u64,
    ) -> Result<Box<dyn PhysicalDatabaseMetaData + 'connection>, DruidError> {
        ConnectionDatabaseMetaDataFilterChain::new(&self.before, physical, connection_id)
            .connection_get_meta_data()
    }

    /// 从位置 0 执行一次 `Statement#getWarnings()` around-chain。
    pub async fn statement_warnings(
        &self,
        physical: &dyn PhysicalStatement,
    ) -> Result<Option<SqlWarning>, DruidError> {
        StatementWarningFilterChain::new_statement(&self.before, physical)
            .statement_get_warnings()
            .await
    }

    /// 从位置 0 执行一次 `Statement#clearWarnings()` around-chain。
    pub async fn statement_clear_warnings(
        &self,
        physical: &dyn PhysicalStatement,
    ) -> Result<(), DruidError> {
        StatementWarningFilterChain::new_statement(&self.before, physical)
            .statement_clear_warnings()
            .await
    }

    /// 从位置 0 执行 PreparedStatement 的 `Statement#getWarnings()` around-chain。
    pub async fn prepared_statement_warnings(
        &self,
        physical: &dyn PhysicalPreparedStatement,
    ) -> Result<Option<SqlWarning>, DruidError> {
        StatementWarningFilterChain::new_prepared_statement(&self.before, physical)
            .statement_get_warnings()
            .await
    }

    /// 从位置 0 执行 PreparedStatement 的 `Statement#clearWarnings()` around-chain。
    pub async fn prepared_statement_clear_warnings(
        &self,
        physical: &dyn PhysicalPreparedStatement,
    ) -> Result<(), DruidError> {
        StatementWarningFilterChain::new_prepared_statement(&self.before, physical)
            .statement_clear_warnings()
            .await
    }

    pub async fn after_execute(
        &self,
        ctx: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let mut first_error = None;
        for f in self.after.iter().rev() {
            if let Err(error) = f.after(ctx, result, elapsed).await {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(())
    }

    /// 按调用栈逆序执行一次 batch 后置链。
    pub async fn after_batch(
        &self,
        context: &BatchExecContext<'_>,
        result: &Result<Vec<i32>, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let mut first_error = None;
        for filter in self.after.iter().rev() {
            if let Err(error) = filter.after_batch(context, result, elapsed).await {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(())
    }

    /// 按调用栈逆序执行物理连接事件成功后的 Filter。
    pub async fn after_connection_event(
        &self,
        event: &ConnectionEvent,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        self.after_connection_event_with_identity(0, event, elapsed)
            .await
    }

    /// 按调用栈逆序执行携带真实连接 ID 的物理连接后置事件。
    pub async fn after_connection_event_with_identity(
        &self,
        connection_id: u64,
        event: &ConnectionEvent,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let context = ConnectionEventContext {
            connection_id,
            event,
        };
        for filter in self.after.iter().rev() {
            filter
                .after_connection_event_context(&context, elapsed)
                .await?;
        }
        Ok(())
    }
}

impl Default for FilterChainImpl {
    fn default() -> Self {
        Self::new()
    }
}
