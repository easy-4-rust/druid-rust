//! 对应 Java：`com.alibaba.druid.proxy.rdbc.DataSourceProxy`。
//! 来源文件：`core/src/main/java/com/alibaba/druid/proxy/rdbc/DataSourceProxy.java`。

use crate::core::PhysicalConnectionFactory;
use crate::stats::StatsCollector;
use std::collections::HashMap;
use std::sync::Arc;

/// Druid 数据源代理的可观察合同。
///
/// Rust 由 canonical `DruidDataSource` 实现该接口，不创建第二套连接池或
/// `DataSourceProxyImpl` 状态机。
pub trait DataSourceProxy {
    /// 返回共享数据源统计对象。
    fn data_source_stat(&self) -> &Arc<StatsCollector>;

    /// 返回管理注册 ID；尚未注册时为 0。
    fn data_source_id(&self) -> u64;

    /// 返回数据源名称。
    fn name(&self) -> &str;

    /// 返回数据库类型；未推断时为 `None`。
    fn db_type(&self) -> Option<&str>;

    /// 返回未池化物理驱动工厂。
    fn raw_driver(&self) -> &dyn PhysicalConnectionFactory;

    /// 返回对外配置 URL。
    fn url(&self) -> Option<&str>;

    /// 返回底层驱动 URL。
    fn raw_rdbc_url(&self) -> Option<&str>;

    /// 返回已装配 Filter 类型名称。
    fn proxy_filter_names(&self) -> Vec<String>;

    /// 分配连接 ID。
    fn create_connection_id(&self) -> u64;

    /// 分配 Statement ID。
    fn create_statement_id(&self) -> u64;

    /// 分配 ResultSet ID。
    fn create_result_set_id(&self) -> u64;

    /// 分配 metadata ID。
    fn create_metadata_id(&self) -> u64;

    /// 分配事务 ID。
    fn create_transaction_id(&self) -> u64;

    /// 返回逻辑驱动连接属性。
    fn connect_properties(&self) -> &HashMap<String, String>;
}
