use crate::canonical_clob::Clob;
/// 国家字符集大对象；对应 Java: java.sql.NClob，继承全部Clob操作且不增加方法。
pub trait NClob: Clob {}
