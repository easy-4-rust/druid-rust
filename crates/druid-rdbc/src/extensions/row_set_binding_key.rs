/// RowSet 参数绑定的原始位置或名称；名称只有经 provider 解析后才对应 SQL 位置。
/// 对应 Java: javax.sql.RowSet#setXxx(int, ...) / setXxx(String, ...)（Rust内部键）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RowSetBindingKey {
    /// 已经通过标准 1-based 校验的位置。
    Index(usize),
    /// 保留大小写、空白及 Unicode 的原始非空名称。
    Name(String),
}
