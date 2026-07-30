//! 对应 Java：`com.alibaba.druid.filter.FilterAdapter`。
//! 来源文件：`core/src/main/java/com/alibaba/druid/filter/FilterAdapter.java`。

use super::{
    AfterFilter, BeforeFilter, DruidError, ExecContext, ExecResult, ExtendedFilter,
    ResultSetFilter, Wrapper,
};
use std::any::Any;
use std::time::Duration;

/// Druid Filter 的默认适配对象。
///
/// Java `FilterAdapter` 为抽象基类：生命周期与属性配置默认不做处理，Wrapper
/// 只识别运行时自身类型，所有 JDBC hook 默认继续调用 `FilterChain`。Rust
/// 不使用继承，因此以一个可组合、可直接注册的对象承载相同默认行为：
///
/// - SQL before/after 默认放行；
/// - 已迁移的 ResultSet hook 通过 [`ResultSetFilter`] 默认方法继续调用链；
/// - Extended hook、生命周期和属性配置使用对应 trait 的默认空语义；
/// - [`Wrapper`] 只识别并返回当前 `FilterAdapter` 对象。
///
/// Java 尚未迁移的 CallableStatement、其他 Connection/Statement、Clob 与
/// DataSource 精确 hook 仍由各自迁移账目跟踪；`Connection#getMetaData` 和
/// ResultSet metadata 已进入真实 around-chain，但不能据此把全部 384 hook
/// 视为完成。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FilterAdapter;

impl FilterAdapter {
    /// 创建默认 Filter 适配对象。
    ///
    /// 对应 Java：`FilterAdapter` 的隐式无参构造语义。
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl BeforeFilter for FilterAdapter {
    fn name(&self) -> &str {
        "FilterAdapter"
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for FilterAdapter {
    fn name(&self) -> &str {
        "FilterAdapter"
    }

    async fn after(
        &self,
        _context: &ExecContext<'_>,
        _result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl ExtendedFilter for FilterAdapter {
    fn is_wrapper_for(&self, type_name: &str) -> bool {
        type_name == std::any::type_name::<Self>()
    }
}

// Java 的 185 个已迁移 ResultSet 方法均是 `return/call chain.resultSet_*`。
// `ResultSetFilter` 的逐方法默认实现正是该继续链语义，不用生成第二份委托代码。
impl ResultSetFilter for FilterAdapter {}

impl Wrapper for FilterAdapter {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
