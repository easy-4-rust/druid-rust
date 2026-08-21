//! Adapter ownership boundary test.

use druid::core::{PhysicalConnection, PhysicalConnectionFactory, Pool};
use druid_wrapper::sqlx::bb8::SqlxBb8Pool;
use druid_wrapper::sqlx::deadpool::SqlxDeadpoolPool;
use druid_wrapper::sqlx::{SqlxConnectionAdapter, SqlxConnectionFactory};
use static_assertions::{assert_impl_all, assert_not_impl_any};

#[test]
fn direct_sqlx_types_are_compile_time_direct_connection_and_factory() {
    assert_impl_all!(SqlxConnectionAdapter: PhysicalConnection);
    assert_impl_all!(SqlxConnectionFactory: PhysicalConnectionFactory);
}

#[test]
fn external_pool_types_are_compile_time_pool_only() {
    assert_impl_all!(SqlxBb8Pool: Pool);
    assert_not_impl_any!(SqlxBb8Pool: PhysicalConnectionFactory);
    assert_impl_all!(SqlxDeadpoolPool: Pool);
    assert_not_impl_any!(SqlxDeadpoolPool: PhysicalConnectionFactory);
}
