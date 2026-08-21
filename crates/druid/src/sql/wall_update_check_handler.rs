//! 对应 Java：`com.alibaba.druid.wall.WallUpdateCheckHandler`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/wall/WallUpdateCheckHandler.java`。

use crate::core::Value;

/// UPDATE 赋值与过滤值的业务一致性检查器。
///
/// Java 在 SQL 中全部是字面量时于 Wall visitor 阶段调用；存在占位符时，
/// 在 `PreparedStatement` 真正执行前以绑定参数求值后调用。Rust 保留相同的
/// 两阶段语义，并要求实现可在线程间安全共享。
pub trait WallUpdateCheckHandler: Send + Sync {
    /// 检查 UPDATE 的目标值是否允许覆盖过滤条件命中的旧值。
    ///
    /// 对应 Java：
    /// `WallUpdateCheckHandler#check(String,String,Object,List<Object>)`。
    ///
    /// # 参数
    /// - `table`：Java 参数 `table`，已按 `WallConfig` 规则规范化。
    /// - `column`：Java 参数 `column`，为配置中首个检查列。
    /// - `set_value`：Java 参数 `setValue`；SQL NULL 或未绑定参数为
    ///   [`Value::Null`]。
    /// - `filter_values`：Java 参数 `filterValues`，已去除重复字面量。
    ///
    /// # 返回
    /// 返回 `true` 表示允许执行，`false` 表示 UPDATE 检查失败。
    fn check(&self, table: &str, column: &str, set_value: &Value, filter_values: &[Value]) -> bool;
}
