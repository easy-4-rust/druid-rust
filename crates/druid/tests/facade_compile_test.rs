//! Facade compile test: verify that core API paths are accessible through `druid::`.

use druid::core::DruidError;

#[test]
fn facade_preserves_core_rdbc_and_pool_paths() {
    fn assert_send<T: Send>() {}
    assert_send::<DruidError>();
}
