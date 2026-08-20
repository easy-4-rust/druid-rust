//! Facade compile test: verify that core API paths are accessible through `druid::`.

use druid::core::{DruidError, DruidPooledConnection, PhysicalConnection};
use druid::pool::{DruidDataSource, DruidPool};
use druid::sql::{Connection, ResultSet, SQLException};

#[test]
fn facade_preserves_core_rdbc_and_pool_paths() {
    fn assert_send<T: Send>() {}
    assert_send::<DruidError>();
}
