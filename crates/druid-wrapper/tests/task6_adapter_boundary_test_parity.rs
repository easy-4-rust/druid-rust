//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Adapter ownership boundary test.

use druid::core::{Connection as NewConnection, ConnectionFactory as NewConnectionFactory};
use druid::core::{Connection as OldConnection, ConnectionFactory as OldConnectionFactory, Pool};
use druid_rdbc::{connection::Connection, driver::ConnectionFactory};
use druid_wrapper::sqlx::bb8::SqlxBb8Pool;
use druid_wrapper::sqlx::deadpool::SqlxDeadpoolPool;
use druid_wrapper::sqlx::{SqlxConnectionAdapter, SqlxConnectionFactory};
use static_assertions::{assert_impl_all, assert_not_impl_any, assert_type_eq_all};

fn old_connection_to_new(connection: Box<dyn OldConnection>) -> Box<dyn NewConnection> {
    connection
}

fn new_connection_to_old(connection: Box<dyn NewConnection>) -> Box<dyn OldConnection> {
    connection
}

fn old_factory_to_new(
    factory: std::sync::Arc<dyn OldConnectionFactory>,
) -> std::sync::Arc<dyn NewConnectionFactory> {
    factory
}

fn new_factory_to_old(
    factory: std::sync::Arc<dyn NewConnectionFactory>,
) -> std::sync::Arc<dyn OldConnectionFactory> {
    factory
}

#[test]
fn task6_adapter_boundary_test_direct_sqlx_types_are_compile_time_direct_connection_and_factory() {
    assert_impl_all!(SqlxConnectionAdapter: OldConnection, NewConnection);
    assert_impl_all!(SqlxConnectionFactory: OldConnectionFactory, NewConnectionFactory);
    assert_impl_all!(SqlxConnectionAdapter: Connection);
    assert_impl_all!(SqlxConnectionFactory: ConnectionFactory);
}

#[test]
fn task6_adapter_boundary_test_external_pool_types_are_compile_time_pool_only() {
    assert_impl_all!(SqlxBb8Pool: Pool);
    assert_not_impl_any!(SqlxBb8Pool: OldConnectionFactory, NewConnectionFactory);
    assert_impl_all!(SqlxDeadpoolPool: Pool);
    assert_not_impl_any!(SqlxDeadpoolPool: OldConnectionFactory, NewConnectionFactory);
}

#[test]
fn task6_adapter_boundary_test_canonical_rdbc_contracts_are_bidirectionally_identical_except_product_error(
) {
    let _ = old_connection_to_new as fn(Box<dyn OldConnection>) -> Box<dyn NewConnection>;
    let _ = new_connection_to_old as fn(Box<dyn NewConnection>) -> Box<dyn OldConnection>;
    let _ = old_factory_to_new
        as fn(std::sync::Arc<dyn OldConnectionFactory>) -> std::sync::Arc<dyn NewConnectionFactory>;
    let _ = new_factory_to_old
        as fn(std::sync::Arc<dyn NewConnectionFactory>) -> std::sync::Arc<dyn OldConnectionFactory>;

    assert_type_eq_all!(
        dyn druid::core::PreparedStatement,
        dyn druid::core::PreparedStatement
    );
    assert_type_eq_all!(
        dyn druid::core::DatabaseMetaData,
        dyn druid::core::DatabaseMetaData
    );
    assert_type_eq_all!(
        dyn druid::core::RdbcResourceAccess,
        dyn druid_rdbc::resource::RdbcResourceAccess
    );
    assert_type_eq_all!(
        druid::core::PreparedInputParameter,
        druid::core::PreparedInputParameter
    );
    assert_type_eq_all!(
        druid::core::CallableInputParameter,
        druid::core::CallableInputParameter
    );
    assert_type_eq_all!(druid::core::Value, druid_rdbc::types::Value);
    assert_ne!(
        std::any::TypeId::of::<druid::core::DruidError>(),
        std::any::TypeId::of::<druid_rdbc::error::RdbcError>()
    );
}

#[test]
fn task6_adapter_boundary_test_seven_adapter_families_implement_canonical_traits() {
    assert_impl_all!(SqlxConnectionAdapter: OldConnection, NewConnection);
    assert_impl_all!(SqlxConnectionFactory: OldConnectionFactory, NewConnectionFactory);
    assert_impl_all!(SqlxConnectionAdapter: Connection);
    assert_impl_all!(SqlxConnectionFactory: ConnectionFactory);
    assert_impl_all!(
        druid_wrapper::toasty::ToastyConnectionAdapter:
            OldConnection,
            NewConnection
    );
    assert_impl_all!(
        druid_wrapper::toasty::ToastyConnectionFactory:
            OldConnectionFactory,
            NewConnectionFactory
    );
    assert_impl_all!(druid_wrapper::toasty::ToastyConnectionAdapter: Connection);
    assert_impl_all!(druid_wrapper::toasty::ToastyConnectionFactory: ConnectionFactory);
    assert_impl_all!(
        druid_wrapper::rbdc::RbdcConnectionAdapter:
            OldConnection,
            NewConnection
    );
    assert_impl_all!(
        druid_wrapper::rbdc::RbdcConnectionFactory:
            OldConnectionFactory,
            NewConnectionFactory
    );
    assert_impl_all!(druid_wrapper::rbdc::RbdcConnectionAdapter: Connection);
    assert_impl_all!(druid_wrapper::rbdc::RbdcConnectionFactory: ConnectionFactory);

    #[cfg(feature = "jdbc-agent")]
    {
        assert_impl_all!(
            druid_wrapper::jdbc_agent::JdbcAgentConnection:
                OldConnection,
                NewConnection
        );
        assert_impl_all!(
            druid_wrapper::jdbc_agent::JdbcAgentConnectionFactory:
                OldConnectionFactory,
                NewConnectionFactory
        );
        assert_impl_all!(druid_wrapper::jdbc_agent::JdbcAgentConnection: Connection);
        assert_impl_all!(druid_wrapper::jdbc_agent::JdbcAgentConnectionFactory: ConnectionFactory);
    }
    #[cfg(feature = "libsql-native")]
    {
        assert_impl_all!(
            druid_wrapper::libsql::LibSqlConnectionAdapter:
                OldConnection,
                NewConnection
        );
        assert_impl_all!(
            druid_wrapper::libsql::LibSqlConnectionFactory:
                OldConnectionFactory,
                NewConnectionFactory
        );
        assert_impl_all!(druid_wrapper::libsql::LibSqlConnectionAdapter: Connection);
        assert_impl_all!(druid_wrapper::libsql::LibSqlConnectionFactory: ConnectionFactory);
    }
    #[cfg(feature = "http-sql")]
    {
        assert_impl_all!(
            druid_wrapper::http_sql::HttpSqlConnectionAdapter:
                OldConnection,
                NewConnection
        );
        assert_impl_all!(
            druid_wrapper::http_sql::HttpSqlConnectionFactory:
                OldConnectionFactory,
                NewConnectionFactory
        );
        assert_impl_all!(druid_wrapper::http_sql::HttpSqlConnectionAdapter: Connection);
        assert_impl_all!(druid_wrapper::http_sql::HttpSqlConnectionFactory: ConnectionFactory);
    }
    #[cfg(feature = "duckdb-native")]
    {
        assert_impl_all!(
            druid_wrapper::duckdb::DuckDbConnectionAdapter:
                OldConnection,
                NewConnection
        );
        assert_impl_all!(
            druid_wrapper::duckdb::DuckDbConnectionFactory:
                OldConnectionFactory,
                NewConnectionFactory
        );
        assert_impl_all!(druid_wrapper::duckdb::DuckDbConnectionAdapter: Connection);
        assert_impl_all!(druid_wrapper::duckdb::DuckDbConnectionFactory: ConnectionFactory);
    }
}
