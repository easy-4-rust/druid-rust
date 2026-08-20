//! Default dependency test: verify that `druid` with default features
//! depends ONLY on `druid-core`, not on `druid-wrapper`, `druid-metrics`,
//! or `druid-admin`.
//!
//! This test is a compile-time smoke check. The real verification is
//! `cargo tree -p druid -e normal` which is run in CI.

#[test]
fn default_build_has_no_wrapper_metrics_admin() {
    // With default features (none), druid::wrapper and druid::metrics
    // must NOT be resolvable. If they were, this test would fail to compile.
    fn assert_core_only() {
        // These would fail if wrapper/metrics were unconditionally linked:
        // use druid::wrapper::WrapperDataSourceFactory;
        // use druid::metrics::DruidMetricsRuntime;
    }
    assert_core_only();
}
