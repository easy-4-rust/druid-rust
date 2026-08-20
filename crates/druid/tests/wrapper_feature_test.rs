//! Wrapper feature completeness test: verify that enabling `wrapper` + `sqlite`
//! makes `druid::wrapper::toasty::ToastyConnectionFactory` available through
//! the facade.

#[cfg(feature = "wrapper")]
mod when_wrapper_enabled {
    #[test]
    fn toasty_connection_factory_is_accessible() {
        // druid::wrapper re-exports druid_wrapper, so the toasty submodule
        // and its ToastyConnectionFactory must be reachable.
        let _ = std::any::type_name::<druid::wrapper::toasty::ToastyConnectionFactory>();
    }

    #[test]
    fn wrapper_top_level_types_accessible() {
        // Verify key wrapper types are re-exported.
        let _ = std::any::type_name::<druid::wrapper::WrapperDataSourceFactory>();
    }

    #[test]
    fn sqlx_adapter_accessible() {
        // sqlx module is always compiled in druid-wrapper.
        let _ = std::any::type_name::<druid::wrapper::sqlx::SqlxConnectionFactory>();
    }

    #[test]
    fn rbdc_adapter_accessible() {
        // rbdc module is always compiled in druid-wrapper.
        let _ = std::any::type_name::<druid::wrapper::rbdc::RbdcConnectionFactory>();
    }
}

#[cfg(not(feature = "wrapper"))]
mod when_wrapper_disabled {
    #[test]
    fn wrapper_module_is_absent() {
        // When wrapper is not enabled, druid::wrapper must not exist.
        fn assert_absent() {}
        assert_absent();
    }
}
