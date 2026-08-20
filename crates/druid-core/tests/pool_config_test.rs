extern crate druid_core as druid;
use druid_core::core::PoolConfig;
use std::time::Duration;

// ── PoolConfig builder ─────────────────────────────────────────

#[test]
fn pool_config_builder_default() {
    let config = PoolConfig::builder().build();
    assert!(config.name.is_empty());
    assert!(config.url.is_empty());
    assert!(config.driver_name.is_empty());
    assert!(config.username.is_empty());
    assert!(config.password.is_empty());
}

#[test]
fn pool_config_builder_name() {
    let config = PoolConfig::builder().name("my-pool").build();
    assert_eq!(config.name, "my-pool");
}

#[test]
fn pool_config_builder_url() {
    let config = PoolConfig::builder().url("sqlite::memory:").build();
    assert_eq!(config.url, "sqlite::memory:");
}

#[test]
fn pool_config_builder_driver_name() {
    let config = PoolConfig::builder().driver_name("sqlite").build();
    assert_eq!(config.driver_name, "sqlite");
}

#[test]
fn pool_config_builder_credentials() {
    let config = PoolConfig::builder()
        .username("admin")
        .password("secret")
        .build();
    assert_eq!(config.username, "admin");
    assert_eq!(config.password, "secret");
}

#[test]
fn pool_config_builder_max_open() {
    let config = PoolConfig::builder().max_open(20).build();
    assert_eq!(config.max_open, 20);
}

#[test]
fn pool_config_builder_min_idle() {
    let config = PoolConfig::builder().min_idle(5).build();
    assert_eq!(config.min_idle, 5);
}

#[test]
fn pool_config_builder_initial_size() {
    let config = PoolConfig::builder().initial_size(10).build();
    assert_eq!(config.initial_size, 10);
}

#[test]
fn pool_config_builder_async_init() {
    let config = PoolConfig::builder().async_init(true).build();
    assert!(config.async_init);
}

#[test]
fn pool_config_builder_init_exception_throw() {
    let config = PoolConfig::builder().init_exception_throw(false).build();
    assert!(!config.init_exception_throw);
}

#[test]
fn pool_config_builder_acquire_timeout() {
    let config = PoolConfig::builder()
        .acquire_timeout(Duration::from_secs(30))
        .build();
    assert_eq!(config.acquire_timeout, Duration::from_secs(30));
}

#[test]
fn pool_config_builder_not_full_timeout_retry_count() {
    let config = PoolConfig::builder()
        .not_full_timeout_retry_count(3)
        .build();
    assert_eq!(config.not_full_timeout_retry_count, 3);
}

#[test]
fn pool_config_builder_max_lifetime() {
    let config = PoolConfig::builder()
        .max_lifetime(Duration::from_secs(3600))
        .build();
    assert_eq!(config.max_lifetime, Duration::from_secs(3600));
}

#[test]
fn pool_config_builder_eviction_interval() {
    let config = PoolConfig::builder()
        .eviction_interval(Duration::from_secs(60))
        .build();
    assert_eq!(config.eviction_interval, Duration::from_secs(60));
}

#[test]
fn pool_config_builder_min_evictable_idle() {
    let config = PoolConfig::builder()
        .min_evictable_idle(Duration::from_secs(300))
        .build();
    assert_eq!(config.min_evictable_idle, Duration::from_secs(300));
}

#[test]
fn pool_config_builder_test_on_borrow() {
    let config = PoolConfig::builder().test_on_borrow(true).build();
    assert!(config.test_on_borrow);
}

#[test]
fn pool_config_builder_test_on_return() {
    let config = PoolConfig::builder().test_on_return(true).build();
    assert!(config.test_on_return);
}

#[test]
fn pool_config_builder_test_while_idle() {
    let config = PoolConfig::builder().test_while_idle(true).build();
    assert!(config.test_while_idle);
}

#[test]
fn pool_config_builder_validation_query() {
    let config = PoolConfig::builder().validation_query("SELECT 1").build();
    assert_eq!(config.validation_query.as_deref(), Some("SELECT 1"));
}

#[test]
fn pool_config_builder_keep_alive() {
    let config = PoolConfig::builder().keep_alive(true).build();
    assert!(config.keep_alive);
}

#[test]
fn pool_config_builder_leak_detection() {
    let config = PoolConfig::builder().leak_detection(true).build();
    assert!(config.leak_detection);
}

#[test]
fn pool_config_builder_leak_threshold() {
    let config = PoolConfig::builder()
        .leak_threshold(Duration::from_secs(60))
        .build();
    assert_eq!(config.leak_threshold, Duration::from_secs(60));
}

#[test]
fn pool_config_builder_slow_sql_threshold() {
    let config = PoolConfig::builder()
        .slow_sql_threshold(Duration::from_secs(5))
        .build();
    assert_eq!(config.slow_sql_threshold, Duration::from_secs(5));
}

#[test]
fn pool_config_builder_pool_prepared_statements() {
    let config = PoolConfig::builder().pool_prepared_statements(true).build();
    assert!(config.pool_prepared_statements);
}

#[test]
fn pool_config_builder_default_auto_commit() {
    let config = PoolConfig::builder().default_auto_commit(false).build();
    assert_eq!(config.default_auto_commit, Some(false));
}

#[test]
fn pool_config_builder_break_after_acquire_failure() {
    let config = PoolConfig::builder()
        .break_after_acquire_failure(true)
        .build();
    assert!(config.break_after_acquire_failure);
}

#[test]
fn pool_config_builder_connection_error_retry_attempts() {
    let config = PoolConfig::builder()
        .connection_error_retry_attempts(5)
        .build();
    assert_eq!(config.connection_error_retry_attempts, 5);
}

#[test]
fn pool_config_builder_on_fatal_error_max_active() {
    let config = PoolConfig::builder().on_fatal_error_max_active(1).build();
    assert_eq!(config.on_fatal_error_max_active, 1);
}

#[test]
fn pool_config_builder_async_close_connection() {
    let config = PoolConfig::builder().async_close_connection(true).build();
    assert!(config.async_close_connection);
}

#[test]
fn pool_config_builder_chaining() {
    let config = PoolConfig::builder()
        .name("test")
        .url("sqlite::memory:")
        .max_open(10)
        .min_idle(2)
        .test_on_borrow(true)
        .keep_alive(true)
        .build();
    assert_eq!(config.name, "test");
    assert_eq!(config.url, "sqlite::memory:");
    assert_eq!(config.max_open, 10);
    assert_eq!(config.min_idle, 2);
    assert!(config.test_on_borrow);
    assert!(config.keep_alive);
}
