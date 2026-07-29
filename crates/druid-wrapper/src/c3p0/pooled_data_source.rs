use druid::core::Pool;

/// c3p0 池化数据源兼容契约。
///
/// 对应 Java: `com.mchange.v2.c3p0.PooledDataSource`。原接口只扩展
/// `javax.sql.DataSource`，因此 Rust 以统一 [`Pool`] 契约表达，不增加伪方法。
pub trait PooledDataSource: Pool {}

impl<T> PooledDataSource for T where T: Pool + ?Sized {}
