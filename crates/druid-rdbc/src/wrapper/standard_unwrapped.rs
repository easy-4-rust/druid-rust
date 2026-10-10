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
use std::any::TypeId;

/// 标准对象的 canonical trait-object 解包结果。
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum StandardUnwrapped<'a> {
    /// Canonical [`Connection`]。
    Connection(&'a dyn Connection),
    /// Canonical [`Statement`]。
    Statement(&'a dyn Statement),
    /// Canonical [`PreparedStatement`]。
    PreparedStatement(&'a dyn PreparedStatement),
    /// Canonical [`CallableStatement`]。
    CallableStatement(&'a dyn CallableStatement),
    /// Canonical [`ResultSet`]。
    ResultSet(&'a dyn ResultSet),
    /// 同一行集的标准接口，继承 ResultSet。
    RowSet(&'a dyn RowSet),
    /// Canonical live [`DatabaseMetaData`]。
    DatabaseMetaData(&'a dyn DatabaseMetaData),
    /// Canonical [`ResultSetMetaData`] snapshot。
    ResultSetMetaData(&'a dyn ResultSetMetaData),
    /// Canonical行集可变元数据接口；继承同一ResultSetMetaData。
    RowSetMetaData(&'a dyn RowSetMetaData),
    /// Canonical [`ParameterMetaData`] snapshot。
    ParameterMetaData(&'a dyn ParameterMetaData),
    /// Canonical [`CommonDataSource`]。
    CommonDataSource(&'a dyn CommonDataSource),
    /// Canonical [`DataSource`]。
    DataSource(&'a dyn DataSource),
    /// Canonical [`PooledConnection`]。
    PooledConnection(&'a dyn PooledConnection),
    /// Canonical [`ConnectionPoolDataSource`]。
    ConnectionPoolDataSource(&'a dyn ConnectionPoolDataSource),
    /// Canonical [`XaConnection`]。
    XaConnection(&'a dyn XaConnection),
    /// Canonical [`XaDataSource`]。
    XaDataSource(&'a dyn XaDataSource),
    /// Canonical [`Driver`]。
    Driver(&'a dyn Driver),
    /// Canonical [`ConnectionFactory`]。
    ConnectionFactory(&'a dyn ConnectionFactory),
    /// Canonical [`Wrapper`]。
    Wrapper(&'a dyn Wrapper),
}

impl StandardUnwrapped<'_> {
    pub(crate) fn type_id(&self) -> TypeId {
        match self {
            Self::Connection(_) => TypeId::of::<dyn Connection>(),
            Self::Statement(_) => TypeId::of::<dyn Statement>(),
            Self::PreparedStatement(_) => TypeId::of::<dyn PreparedStatement>(),
            Self::CallableStatement(_) => TypeId::of::<dyn CallableStatement>(),
            Self::ResultSet(_) => TypeId::of::<dyn ResultSet>(),
            Self::RowSet(_) => TypeId::of::<dyn RowSet>(),
            Self::DatabaseMetaData(_) => TypeId::of::<dyn DatabaseMetaData>(),
            Self::ResultSetMetaData(_) => TypeId::of::<dyn ResultSetMetaData>(),
            Self::RowSetMetaData(_) => TypeId::of::<dyn RowSetMetaData>(),
            Self::ParameterMetaData(_) => TypeId::of::<dyn ParameterMetaData>(),
            Self::CommonDataSource(_) => TypeId::of::<dyn CommonDataSource>(),
            Self::DataSource(_) => TypeId::of::<dyn DataSource>(),
            Self::PooledConnection(_) => TypeId::of::<dyn PooledConnection>(),
            Self::ConnectionPoolDataSource(_) => TypeId::of::<dyn ConnectionPoolDataSource>(),
            Self::XaConnection(_) => TypeId::of::<dyn XaConnection>(),
            Self::XaDataSource(_) => TypeId::of::<dyn XaDataSource>(),
            Self::Driver(_) => TypeId::of::<dyn Driver>(),
            Self::ConnectionFactory(_) => TypeId::of::<dyn ConnectionFactory>(),
            Self::Wrapper(_) => TypeId::of::<dyn Wrapper>(),
        }
    }
}

impl std::fmt::Debug for StandardUnwrapped<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("StandardUnwrapped")
            .field(&self.type_id())
            .finish()
    }
}
