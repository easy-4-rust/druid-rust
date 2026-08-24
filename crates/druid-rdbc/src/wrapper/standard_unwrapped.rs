use crate::connection::Connection;
use crate::data_source::{
    CommonDataSource, ConnectionPoolDataSource, DataSource, PooledConnection, XaConnection,
    XaDataSource,
};
use crate::driver::{ConnectionFactory, Driver};
use crate::metadata::{DatabaseMetaData, ParameterMetaData, ResultSetMetaData};
use crate::result_set::ResultSet;
use crate::statement::{CallableStatement, PreparedStatement, Statement};
use crate::wrapper::Wrapper;
use std::any::Any;

/// 标准对象的 object-safe 解包结果。
///
/// 对应 Java：`Wrapper#unwrap(Class)`。Canonical interfaces 保留 trait-object identity；
/// 具体 Rust 类型只通过 [`crate::wrapper::WrapperExt`] 执行 [`Any`] downcast。
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum StandardUnwrapped<'a> {
    /// 具体 Rust 对象，仅供 [`crate::wrapper::WrapperExt`] 使用。
    Object(&'a dyn Any),
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
    /// Canonical live [`DatabaseMetaData`]。
    DatabaseMetaData(&'a dyn DatabaseMetaData),
    /// Canonical immutable [`ResultSetMetaData`] snapshot。
    ResultSetMetaData(&'a dyn ResultSetMetaData),
    /// Canonical immutable [`ParameterMetaData`] snapshot。
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
    /// Canonical [`Wrapper`] view。
    Wrapper(&'a dyn Wrapper),
}

impl<'a> StandardUnwrapped<'a> {
    pub(crate) fn object(&self) -> Option<&'a dyn Any> {
        match self {
            Self::Object(object) => Some(*object),
            Self::Connection(_)
            | Self::Statement(_)
            | Self::PreparedStatement(_)
            | Self::CallableStatement(_)
            | Self::ResultSet(_)
            | Self::DatabaseMetaData(_)
            | Self::ResultSetMetaData(_)
            | Self::ParameterMetaData(_)
            | Self::CommonDataSource(_)
            | Self::DataSource(_)
            | Self::PooledConnection(_)
            | Self::ConnectionPoolDataSource(_)
            | Self::XaConnection(_)
            | Self::XaDataSource(_)
            | Self::Driver(_)
            | Self::ConnectionFactory(_)
            | Self::Wrapper(_) => None,
        }
    }
}

impl std::fmt::Debug for StandardUnwrapped<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Object(_) => "Object",
            Self::Connection(_) => "Connection",
            Self::Statement(_) => "Statement",
            Self::PreparedStatement(_) => "PreparedStatement",
            Self::CallableStatement(_) => "CallableStatement",
            Self::ResultSet(_) => "ResultSet",
            Self::DatabaseMetaData(_) => "DatabaseMetaData",
            Self::ResultSetMetaData(_) => "ResultSetMetaData",
            Self::ParameterMetaData(_) => "ParameterMetaData",
            Self::CommonDataSource(_) => "CommonDataSource",
            Self::DataSource(_) => "DataSource",
            Self::PooledConnection(_) => "PooledConnection",
            Self::ConnectionPoolDataSource(_) => "ConnectionPoolDataSource",
            Self::XaConnection(_) => "XaConnection",
            Self::XaDataSource(_) => "XaDataSource",
            Self::Driver(_) => "Driver",
            Self::ConnectionFactory(_) => "ConnectionFactory",
            Self::Wrapper(_) => "Wrapper",
        };
        formatter
            .debug_tuple("StandardUnwrapped")
            .field(&name)
            .finish()
    }
}
