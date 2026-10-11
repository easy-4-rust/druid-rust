use super::{ConnectionFactory, RdbcError};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// 扩展驱动工厂的异步构造结果；返回未池化的标准连接工厂。
/// 对应 Java: com.alibaba.druid.pool.DruidDataSource#createPhysicalConnection 的异步装配边界。
pub type DriverExtensionFactoryFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Arc<dyn ConnectionFactory>, RdbcError>> + Send + 'a>>;

/// 扩展驱动的异步工厂入口；参数为连接 URL，返回可等待的工厂构造结果。
pub type DriverExtensionFactory = for<'a> fn(&'a str) -> DriverExtensionFactoryFuture<'a>;
