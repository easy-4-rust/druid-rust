//! Canonical standard compile-time conformance for every wrapper adapter family.

use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::metadata::DatabaseMetaData;
use druid_rdbc::statement::PreparedStatement;
use druid_rdbc::wrapper::Wrapper;
use static_assertions::assert_impl_all;

#[test]
fn configured_core_adapter_families_implement_the_single_standard_directly() {
    assert_impl_all!(druid_wrapper::sqlx::SqlxConnectionAdapter: Connection);
    assert_impl_all!(druid_wrapper::sqlx::SqlxConnectionFactory: ConnectionFactory);
    assert_impl_all!(druid_wrapper::sqlx::SqlxPreparedStatement: PreparedStatement);
    assert_impl_all!(druid_wrapper::sqlx::SqlxDatabaseMetaData<'static>: DatabaseMetaData);

    assert_impl_all!(druid_wrapper::rbdc::RbdcConnectionAdapter: Connection);
    assert_impl_all!(druid_wrapper::rbdc::RbdcConnectionFactory: ConnectionFactory);
    assert_impl_all!(druid_wrapper::rbdc::RbdcDatabaseMetaData<'static>: DatabaseMetaData);

    assert_impl_all!(druid_wrapper::toasty::ToastyConnectionAdapter: Connection);
    assert_impl_all!(druid_wrapper::toasty::ToastyConnectionFactory: ConnectionFactory);
    assert_impl_all!(druid_wrapper::toasty::ToastyDatabaseMetaData<'static>: DatabaseMetaData);
}

#[test]
fn catalog_data_sources_implement_canonical_wrapper_identity() {
    assert_impl_all!(druid_wrapper::rdbc::DruidRdbcDataSource: Wrapper);
    assert_impl_all!(druid_wrapper::rdbc::DynamicRdbcDataSource: Wrapper);
}

#[cfg(feature = "duckdb-native")]
#[test]
fn duckdb_family_implements_the_single_standard_directly() {
    assert_impl_all!(druid_wrapper::duckdb::DuckDbConnectionAdapter: Connection);
    assert_impl_all!(druid_wrapper::duckdb::DuckDbConnectionFactory: ConnectionFactory);
    assert_impl_all!(druid_wrapper::duckdb::DuckDbDatabaseMetaData: DatabaseMetaData);
}

#[cfg(feature = "http-sql")]
#[test]
fn http_sql_family_implements_the_single_standard_directly() {
    assert_impl_all!(druid_wrapper::http_sql::HttpSqlConnectionAdapter: Connection);
    assert_impl_all!(druid_wrapper::http_sql::HttpSqlConnectionFactory: ConnectionFactory);
    assert_impl_all!(druid_wrapper::http_sql::HttpSqlDatabaseMetaData: DatabaseMetaData);
}

#[cfg(feature = "libsql-native")]
#[test]
fn libsql_family_implements_the_single_standard_directly() {
    assert_impl_all!(druid_wrapper::libsql::LibSqlConnectionAdapter: Connection);
    assert_impl_all!(druid_wrapper::libsql::LibSqlConnectionFactory: ConnectionFactory);
    assert_impl_all!(druid_wrapper::libsql::LibSqlDatabaseMetaData: DatabaseMetaData);
}

#[cfg(feature = "jdbc-agent")]
#[test]
fn jdbc_agent_family_implements_the_single_standard_directly() {
    assert_impl_all!(druid_wrapper::jdbc_agent::JdbcAgentConnection: Connection);
    assert_impl_all!(druid_wrapper::jdbc_agent::JdbcAgentConnectionFactory: ConnectionFactory);
    assert_impl_all!(druid_wrapper::jdbc_agent::JdbcAgentDatabaseMetaData: DatabaseMetaData);
}
