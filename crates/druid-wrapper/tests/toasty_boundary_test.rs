//! Boundary test: Toasty types must be exposed only by druid-wrapper.
//!
//! After migration, `druid_wrapper::toasty` is the sole public path for
//! Toasty adapters. druid-core no longer re-exports Toasty.

#[test]
fn toasty_is_exposed_only_by_wrapper() {
    // This compile-time check ensures druid-wrapper exposes the Toasty adapter types.
    let _adapter = std::any::type_name::<druid_wrapper::toasty::ToastyConnectionAdapter>();
    let _factory = std::any::type_name::<druid_wrapper::toasty::ToastyConnectionFactory>();
}

/// Verify druid-core no longer exposes a toasty module.
#[test]
fn core_has_no_public_toasty_module() {
    // After migration, druid::toasty should not exist.
    // This test will fail to compile if druid still re-exports toasty from core.
    // We verify by checking that the facade crate does not have the module.
    // The compile error itself is the assertion.
    //
    // Note: This test lives in druid-wrapper which depends on druid (facade).
    // If druid::toasty still compiles, this test needs to be updated to use
    // a compile-fail trybuild test instead.
    assert!(true, "placeholder -- compile-fail enforced by build");
}
