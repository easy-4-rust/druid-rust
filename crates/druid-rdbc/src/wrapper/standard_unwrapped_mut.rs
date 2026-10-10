use crate::connection::Connection;
use crate::data_source::{
    CommonDataSource, ConnectionPoolDataSource, DataSource, PooledConnection, XaConnection,
    XaDataSource,
};
use crate::driver::{ConnectionFactory, Driver};
use crate::extensions::{RowSet, RowSetMetaData};
use crate::metadata::{DatabaseMetaData, ParameterMetaData, ResultSetMetaData};
use crate::result_set::ResultSet;
use crate::statement::{CallableStatement, PreparedStatement, Statement};
use crate::wrapper::Wrapper;

/// 标准对象的独占可变接口视图。对应 Java：java.sql.Wrapper#unwrap。
/// 视图借用原对象且不能复制；所有操作保持同一对象身份，不进行具体类型 downcast。
#[non_exhaustive]
pub enum StandardUnwrappedMut<'a> {
    /// 连接标准接口。
    Connection(&'a mut dyn Connection),
    /// 普通语句标准接口。
    Statement(&'a mut dyn Statement),
    /// 预编译语句标准接口。
    PreparedStatement(&'a mut dyn PreparedStatement),
    /// 可调用语句标准接口。
    CallableStatement(&'a mut dyn CallableStatement),
    /// 结果游标标准接口。
    ResultSet(&'a mut dyn ResultSet),
    /// 同一行集的标准接口，继承 ResultSet。
    RowSet(&'a mut dyn RowSet),
    /// 数据库元数据标准接口。
    DatabaseMetaData(&'a mut dyn DatabaseMetaData),
    /// 结果元数据标准接口。
    ResultSetMetaData(&'a mut dyn ResultSetMetaData),
    /// 行集元数据标准接口；setter作用于同一原对象。
    RowSetMetaData(&'a mut dyn RowSetMetaData),
    /// 参数元数据标准接口。
    ParameterMetaData(&'a mut dyn ParameterMetaData),
    /// 公共数据源标准接口。
    CommonDataSource(&'a mut dyn CommonDataSource),
    /// 数据源标准接口。
    DataSource(&'a mut dyn DataSource),
    /// 池化连接标准接口。
    PooledConnection(&'a mut dyn PooledConnection),
    /// 池化数据源标准接口。
    ConnectionPoolDataSource(&'a mut dyn ConnectionPoolDataSource),
    /// XA连接标准接口。
    XaConnection(&'a mut dyn XaConnection),
    /// XA数据源标准接口。
    XaDataSource(&'a mut dyn XaDataSource),
    /// 驱动标准接口。
    Driver(&'a mut dyn Driver),
    /// 连接工厂标准接口。
    ConnectionFactory(&'a mut dyn ConnectionFactory),
    /// 包装器标准接口。
    Wrapper(&'a mut dyn Wrapper),
}

impl std::fmt::Debug for StandardUnwrappedMut<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("StandardUnwrappedMut")
            .field(&std::mem::discriminant(self))
            .finish()
    }
}
