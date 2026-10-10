use crate::error::RdbcError;
use crate::statement::ParameterIndex;
use std::sync::Arc;

/// provider 对当前命令与原始参数名的显式位置解析器；不猜测 SQL 参数顺序。
/// 对应 Java: javax.sql.RowSet 命名参数与 RowSetInternal#getParams 的 Rust provider 适配。
pub type RowSetParameterResolver =
    Arc<dyn Fn(Option<&str>, &str) -> Result<ParameterIndex, RdbcError> + Send + Sync>;
