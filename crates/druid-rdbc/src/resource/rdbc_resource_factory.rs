use crate::resource::{
    Array, Blob, Clob, NClob, RdbcArray, RdbcBlob, RdbcClob, RdbcNClob, RdbcRef, RdbcSqlXml,
    RdbcStruct, Ref, SqlXml, Struct,
};

/// Creates public RDBC handles without exposing their driver access implementations.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResourceHandleFactory;

impl ResourceHandleFactory {
    /// 将唯一标准Array对象装入共享值句柄，参数为独立拥有的驱动对象，返回保留身份及释放状态的句柄。
    /// 对应 Java: java.sql.Array；范围、映射和游标直接委托，不增加驱动端口。
    #[must_use]
    pub fn array(value: Box<dyn Array>) -> RdbcArray {
        RdbcArray::from_standard(value)
    }

    /// 将唯一标准Blob对象装入共享值句柄，参数为独立拥有的驱动对象，返回保留身份与释放状态的句柄。
    /// 对应 Java: java.sql.Blob；不提前读取内容，不增加驱动端口。
    #[must_use]
    pub fn blob(value: Box<dyn Blob>) -> RdbcBlob {
        RdbcBlob::from_standard(value)
    }

    /// 接管参数value唯一标准Clob，返回保留身份及释放状态的共享句柄。对应 Java: java.sql.Clob。
    #[must_use]
    pub fn clob(value: Box<dyn Clob>) -> RdbcClob {
        RdbcClob::from_standard(value)
    }
    /// 接管参数value唯一标准NClob，返回保留国家字符身份及共享Clob视图的句柄。对应 Java: java.sql.NClob。
    #[must_use]
    pub fn n_clob(value: Box<dyn NClob>) -> RdbcNClob {
        RdbcNClob::from_standard(value)
    }

    /// 将独立拥有的标准Ref装入共享值句柄，参数为唯一驱动对象，返回保持引用身份的句柄。
    /// 对应 Java: java.sql.Ref；不物化属性，不新增free或连接创建接口。
    #[must_use]
    pub fn reference(value: Box<dyn Ref>) -> RdbcRef {
        RdbcRef::from_standard(value)
    }

    /// 将独立拥有的标准Struct装入共享值句柄，参数为唯一驱动对象，返回延迟读取属性的句柄。
    /// 对应 Java: java.sql.Struct；用于getObject及嵌套属性，不新增驱动端口。
    #[must_use]
    pub fn structure(value: Box<dyn Struct>) -> RdbcStruct {
        RdbcStruct::from_standard(value)
    }

    /// 接管参数value唯一标准SQLXML，返回共享身份、一次性读写和释放确认的句柄。对应 Java: java.sql.SQLXML。
    #[must_use]
    pub fn sql_xml(value: Box<dyn SqlXml>) -> RdbcSqlXml {
        RdbcSqlXml::from_standard(value)
    }
}
