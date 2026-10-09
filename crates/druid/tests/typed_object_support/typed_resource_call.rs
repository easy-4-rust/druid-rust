/// typed getObject 默认分派产生的真实资源读取轨迹。
/// 对应 Java: java.sql.ResultSet 的资源 getter 与 findColumn。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedResourceCall {
    FindColumn(String),
    Blob(usize),
    Clob(usize),
    NClob(usize),
    Array(usize),
    Ref(usize),
    RowId(usize),
    SqlXml(usize),
    Url(usize),
}
