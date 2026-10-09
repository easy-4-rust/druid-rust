//! 完整对象读取参数候选，尚未接入生产模块或完成 API 验收。
use druid_rdbc::types::{RdbcTargetType, RdbcTypeMap};

/// 保留普通、可空映射和指定目标类型的重载身份。
/// 对应 Java: java.sql.ResultSet#getObject 的普通、Map 和 Class 重载。
pub enum ResultSetObjectRead<'a> {
    /// 普通对象读取，无类型映射参数。
    Ordinary,
    /// 带可空类型映射，None 与普通重载及空映射分别保留。
    TypeMap(Option<&'a RdbcTypeMap>),
    /// 带原目标类型引用，不复制或提前转化调用者对象。
    Typed(&'a RdbcTargetType),
}
