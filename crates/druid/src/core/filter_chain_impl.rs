//! 对应 Java 类：com.alibaba.druid.filter.FilterChainImpl

use super::connection::ExecResult;
use super::error::DruidError;
use super::filter::{
    AfterFilter, BatchExecContext, BeforeFilter, ConnectionEvent, ConnectionEventContext,
    ExecContext, StatementEvent, StatementEventContext,
};
use super::{
    JdbcArray, JdbcBlob, JdbcCalendarArgument, JdbcClob, JdbcInputStream, JdbcNClob, JdbcObject,
    JdbcReader, JdbcRef, JdbcRowId, JdbcSqlXml, JdbcTargetType, JdbcTypeMap, JdbcUrl,
    PhysicalConnection, PhysicalDatabaseMetaData, PhysicalPreparedStatement, PhysicalResultSet,
    PhysicalStatement, ResultSetFilter, ResultSetFilterChain, ResultSetFilterContext,
    ResultSetMetaData, ResultSetOpenContext, ResultSetStatement, SqlWarning, Value,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
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
                calendar: &JdbcCalendarArgument,
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
                calendar: &JdbcCalendarArgument,
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

/// 单次 Connection warning 操作使用的有位置 Filter 调用链。
///
/// 对应 Java：`FilterChainImpl#connection_getWarnings` 与
/// `connection_clearWarnings`。每次调用都从位置 0 开始，末端才进入物理连接。
pub struct ConnectionWarningFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    physical: &'a mut dyn PhysicalConnection,
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
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object_with_type_map(column_index, type_map)
    }

    /// 从位置 0 执行 `ResultSet#getObject(String, Map)` around-chain。
    pub fn result_set_get_object_by_label_with_type_map(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object_by_label_with_type_map(column_label, type_map)
    }

    /// 从位置 0 执行 `ResultSet#getObject(int, Class<T>)` around-chain。
    pub fn result_set_get_object_typed(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_index: usize,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        ResultSetFilterChain::new(&self.result_set, physical, context)
            .result_set_get_object_typed(column_index, target_type)
    }

    /// 从位置 0 执行 `ResultSet#getObject(String, Class<T>)` around-chain。
    pub fn result_set_get_object_typed_by_label(
        &self,
        physical: &dyn PhysicalResultSet,
        context: &ResultSetFilterContext,
        column_label: &str,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
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
            JdbcRef,
            "getRef"
        ),
        (
            result_set_get_blob,
            result_set_get_blob_by_label,
            JdbcBlob,
            "getBlob"
        ),
        (
            result_set_get_clob,
            result_set_get_clob_by_label,
            JdbcClob,
            "getClob"
        ),
        (
            result_set_get_array,
            result_set_get_array_by_label,
            JdbcArray,
            "getArray"
        ),
        (
            result_set_get_url,
            result_set_get_url_by_label,
            JdbcUrl,
            "getURL"
        ),
        (
            result_set_get_row_id,
            result_set_get_row_id_by_label,
            JdbcRowId,
            "getRowId"
        ),
        (
            result_set_get_n_clob,
            result_set_get_n_clob_by_label,
            JdbcNClob,
            "getNClob"
        ),
        (
            result_set_get_sql_xml,
            result_set_get_sql_xml_by_label,
            JdbcSqlXml,
            "getSQLXML"
        ),
        (
            result_set_get_ascii_stream,
            result_set_get_ascii_stream_by_label,
            JdbcInputStream,
            "getAsciiStream"
        ),
        (
            result_set_get_unicode_stream,
            result_set_get_unicode_stream_by_label,
            JdbcInputStream,
            "getUnicodeStream"
        ),
        (
            result_set_get_binary_stream,
            result_set_get_binary_stream_by_label,
            JdbcInputStream,
            "getBinaryStream"
        ),
        (
            result_set_get_character_stream,
            result_set_get_character_stream_by_label,
            JdbcReader,
            "getCharacterStream"
        ),
        (
            result_set_get_n_character_stream,
            result_set_get_n_character_stream_by_label,
            JdbcReader,
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
        value: JdbcObject,
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
        value: JdbcObject,
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
        value: JdbcObject,
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
        value: JdbcObject,
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
            JdbcRef,
            "updateRef"
        ),
        (
            result_set_update_blob,
            result_set_update_blob_by_label,
            JdbcBlob,
            "updateBlob"
        ),
        (
            result_set_update_clob,
            result_set_update_clob_by_label,
            JdbcClob,
            "updateClob"
        ),
        (
            result_set_update_array,
            result_set_update_array_by_label,
            JdbcArray,
            "updateArray"
        ),
        (
            result_set_update_row_id,
            result_set_update_row_id_by_label,
            JdbcRowId,
            "updateRowId"
        ),
        (
            result_set_update_n_clob,
            result_set_update_n_clob_by_label,
            JdbcNClob,
            "updateNClob"
        ),
        (
            result_set_update_sql_xml,
            result_set_update_sql_xml_by_label,
            JdbcSqlXml,
            "updateSQLXML"
        ),
    );

    lob_stream_update_result_set_filter_chain_methods!(
        (
            result_set_update_blob_stream,
            result_set_update_blob_stream_by_label,
            result_set_update_blob_stream_with_length,
            result_set_update_blob_stream_by_label_with_length,
            JdbcInputStream,
            "updateBlob"
        ),
        (
            result_set_update_clob_reader,
            result_set_update_clob_reader_by_label,
            result_set_update_clob_reader_with_length,
            result_set_update_clob_reader_by_label_with_length,
            JdbcReader,
            "updateClob"
        ),
        (
            result_set_update_n_clob_reader,
            result_set_update_n_clob_reader_by_label,
            result_set_update_n_clob_reader_with_length,
            result_set_update_n_clob_reader_by_label_with_length,
            JdbcReader,
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
            JdbcInputStream,
            "updateAsciiStream"
        ),
        (
            result_set_update_binary_stream,
            result_set_update_binary_stream_by_label,
            result_set_update_binary_stream_with_int_length,
            result_set_update_binary_stream_by_label_with_int_length,
            result_set_update_binary_stream_with_length,
            result_set_update_binary_stream_by_label_with_length,
            JdbcInputStream,
            "updateBinaryStream"
        ),
        (
            result_set_update_character_stream,
            result_set_update_character_stream_by_label,
            result_set_update_character_stream_with_int_length,
            result_set_update_character_stream_by_label_with_int_length,
            result_set_update_character_stream_with_length,
            result_set_update_character_stream_by_label_with_length,
            JdbcReader,
            "updateCharacterStream"
        ),
    );

    long_stream_update_result_set_filter_chain_methods!((
        result_set_update_n_character_stream,
        result_set_update_n_character_stream_by_label,
        result_set_update_n_character_stream_with_length,
        result_set_update_n_character_stream_by_label_with_length,
        JdbcReader,
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

    /// 按逆序执行连接关闭后置过滤器。
    pub async fn after_connection_close(&self) -> Result<(), DruidError> {
        self.after_connection_close_with_identity(0).await
    }

    /// 按逆序执行携带真实连接 ID 的连接关闭后置过滤器。
    pub async fn after_connection_close_with_identity(
        &self,
        connection_id: u64,
    ) -> Result<(), DruidError> {
        for filter in self.after.iter().rev() {
            filter.after_connection_close_context(connection_id).await?;
        }
        Ok(())
    }
}

impl Default for FilterChainImpl {
    fn default() -> Self {
        Self::new()
    }
}
