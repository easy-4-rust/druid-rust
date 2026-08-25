//! Adapter ownership boundary test.

use druid::core::{
    PhysicalConnection as OldPhysicalConnection,
    PhysicalConnectionFactory as OldPhysicalConnectionFactory, Pool,
};
use druid_rdbc::spi::{
    PhysicalConnection as NewPhysicalConnection,
    PhysicalConnectionFactory as NewPhysicalConnectionFactory,
};
use druid_rdbc::{connection::Connection, driver::ConnectionFactory};
use druid_wrapper::sqlx::bb8::SqlxBb8Pool;
use druid_wrapper::sqlx::deadpool::SqlxDeadpoolPool;
use druid_wrapper::sqlx::{SqlxConnectionAdapter, SqlxConnectionFactory};
use static_assertions::{assert_impl_all, assert_not_impl_any, assert_type_eq_all};

#[test]
fn direct_sqlx_types_are_compile_time_direct_connection_and_factory() {
    assert_impl_all!(SqlxConnectionAdapter: OldPhysicalConnection, NewPhysicalConnection);
    assert_impl_all!(SqlxConnectionFactory: OldPhysicalConnectionFactory, NewPhysicalConnectionFactory);
    assert_impl_all!(SqlxConnectionAdapter: Connection);
    assert_impl_all!(SqlxConnectionFactory: ConnectionFactory);
}

#[test]
fn external_pool_types_are_compile_time_pool_only() {
    assert_impl_all!(SqlxBb8Pool: Pool);
    assert_not_impl_any!(SqlxBb8Pool: OldPhysicalConnectionFactory, NewPhysicalConnectionFactory);
    assert_impl_all!(SqlxDeadpoolPool: Pool);
    assert_not_impl_any!(SqlxDeadpoolPool: OldPhysicalConnectionFactory, NewPhysicalConnectionFactory);
}

fn old_connection_to_new(
    connection: Box<dyn OldPhysicalConnection>,
) -> Box<dyn NewPhysicalConnection> {
    connection
}

fn new_connection_to_old(
    connection: Box<dyn NewPhysicalConnection>,
) -> Box<dyn OldPhysicalConnection> {
    connection
}

fn old_factory_to_new(
    factory: std::sync::Arc<dyn OldPhysicalConnectionFactory>,
) -> std::sync::Arc<dyn NewPhysicalConnectionFactory> {
    factory
}

fn new_factory_to_old(
    factory: std::sync::Arc<dyn NewPhysicalConnectionFactory>,
) -> std::sync::Arc<dyn OldPhysicalConnectionFactory> {
    factory
}

#[test]
fn old_and_new_rdbc_contracts_are_bidirectionally_identical_except_product_error() {
    let _ = old_connection_to_new
        as fn(Box<dyn OldPhysicalConnection>) -> Box<dyn NewPhysicalConnection>;
    let _ = new_connection_to_old
        as fn(Box<dyn NewPhysicalConnection>) -> Box<dyn OldPhysicalConnection>;
    let _ = old_factory_to_new
        as fn(
            std::sync::Arc<dyn OldPhysicalConnectionFactory>,
        ) -> std::sync::Arc<dyn NewPhysicalConnectionFactory>;
    let _ = new_factory_to_old
        as fn(
            std::sync::Arc<dyn NewPhysicalConnectionFactory>,
        ) -> std::sync::Arc<dyn OldPhysicalConnectionFactory>;

    assert_type_eq_all!(
        dyn druid::core::PhysicalPreparedStatement,
        dyn druid_rdbc::spi::PhysicalPreparedStatement
    );
    assert_type_eq_all!(
        dyn druid::core::PhysicalDatabaseMetaData,
        dyn druid_rdbc::spi::PhysicalDatabaseMetaData
    );
    assert_type_eq_all!(
        dyn druid::spi::RdbcResourceAccess,
        dyn druid_rdbc::resource::RdbcResourceAccess
    );
    assert_type_eq_all!(
        druid::core::PreparedInputParameter,
        druid_rdbc::spi::PreparedInputParameter
    );
    assert_type_eq_all!(
        druid::core::CallableInputParameter,
        druid_rdbc::spi::CallableInputParameter
    );
    assert_type_eq_all!(druid::core::Value, druid_rdbc::types::Value);
    assert_ne!(
        std::any::TypeId::of::<druid::core::DruidError>(),
        std::any::TypeId::of::<druid_rdbc::error::RdbcError>()
    );
}

#[test]
fn seven_adapter_families_implement_old_and_new_traits() {
    assert_impl_all!(SqlxConnectionAdapter: OldPhysicalConnection, NewPhysicalConnection);
    assert_impl_all!(SqlxConnectionFactory: OldPhysicalConnectionFactory, NewPhysicalConnectionFactory);
    assert_impl_all!(SqlxConnectionAdapter: Connection);
    assert_impl_all!(SqlxConnectionFactory: ConnectionFactory);
    assert_impl_all!(
        druid_wrapper::toasty::ToastyConnectionAdapter:
            OldPhysicalConnection,
            NewPhysicalConnection
    );
    assert_impl_all!(
        druid_wrapper::toasty::ToastyConnectionFactory:
            OldPhysicalConnectionFactory,
            NewPhysicalConnectionFactory
    );
    assert_impl_all!(druid_wrapper::toasty::ToastyConnectionAdapter: Connection);
    assert_impl_all!(druid_wrapper::toasty::ToastyConnectionFactory: ConnectionFactory);
    assert_impl_all!(
        druid_wrapper::rbdc::RbdcConnectionAdapter:
            OldPhysicalConnection,
            NewPhysicalConnection
    );
    assert_impl_all!(
        druid_wrapper::rbdc::RbdcConnectionFactory:
            OldPhysicalConnectionFactory,
            NewPhysicalConnectionFactory
    );
    assert_impl_all!(druid_wrapper::rbdc::RbdcConnectionAdapter: Connection);
    assert_impl_all!(druid_wrapper::rbdc::RbdcConnectionFactory: ConnectionFactory);

    #[cfg(feature = "jdbc-agent")]
    {
        assert_impl_all!(
            druid_wrapper::jdbc_agent::JdbcAgentConnection:
                OldPhysicalConnection,
                NewPhysicalConnection
        );
        assert_impl_all!(
            druid_wrapper::jdbc_agent::JdbcAgentConnectionFactory:
                OldPhysicalConnectionFactory,
                NewPhysicalConnectionFactory
        );
        assert_impl_all!(druid_wrapper::jdbc_agent::JdbcAgentConnection: Connection);
        assert_impl_all!(druid_wrapper::jdbc_agent::JdbcAgentConnectionFactory: ConnectionFactory);
    }
    #[cfg(feature = "libsql-native")]
    {
        assert_impl_all!(
            druid_wrapper::libsql::LibSqlConnectionAdapter:
                OldPhysicalConnection,
                NewPhysicalConnection
        );
        assert_impl_all!(
            druid_wrapper::libsql::LibSqlConnectionFactory:
                OldPhysicalConnectionFactory,
                NewPhysicalConnectionFactory
        );
        assert_impl_all!(druid_wrapper::libsql::LibSqlConnectionAdapter: Connection);
        assert_impl_all!(druid_wrapper::libsql::LibSqlConnectionFactory: ConnectionFactory);
    }
    #[cfg(feature = "http-sql")]
    {
        assert_impl_all!(
            druid_wrapper::http_sql::HttpSqlConnectionAdapter:
                OldPhysicalConnection,
                NewPhysicalConnection
        );
        assert_impl_all!(
            druid_wrapper::http_sql::HttpSqlConnectionFactory:
                OldPhysicalConnectionFactory,
                NewPhysicalConnectionFactory
        );
        assert_impl_all!(druid_wrapper::http_sql::HttpSqlConnectionAdapter: Connection);
        assert_impl_all!(druid_wrapper::http_sql::HttpSqlConnectionFactory: ConnectionFactory);
    }
    #[cfg(feature = "duckdb-native")]
    {
        assert_impl_all!(
            druid_wrapper::duckdb::DuckDbConnectionAdapter:
                OldPhysicalConnection,
                NewPhysicalConnection
        );
        assert_impl_all!(
            druid_wrapper::duckdb::DuckDbConnectionFactory:
                OldPhysicalConnectionFactory,
                NewPhysicalConnectionFactory
        );
        assert_impl_all!(druid_wrapper::duckdb::DuckDbConnectionAdapter: Connection);
        assert_impl_all!(druid_wrapper::duckdb::DuckDbConnectionFactory: ConnectionFactory);
    }
}
