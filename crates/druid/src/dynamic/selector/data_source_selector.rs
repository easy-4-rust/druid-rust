//! 对应 Java 接口：`com.alibaba.druid.pool.ha.selector.DataSourceSelector`。

use crate::core::Pool;
use std::sync::Arc;

/// 从 HA 数据源的可用节点中选择一个物理数据源。
pub trait DataSourceSelector: Send + Sync {
    /// 返回当前选择的数据源；无可用节点时返回 `None`。
    fn get(&self) -> Option<Arc<dyn Pool>>;

    /// 设置当前执行上下文的目标数据源名称。
    fn set_target(&self, name: Option<String>);

    /// 返回选择器配置名称。
    fn name(&self) -> &'static str;

    /// 启动选择器维护资源。
    fn init(&self);

    /// 停止选择器维护资源。
    fn destroy(&self);
}
