use druid::dynamic::HighAvailableDataSource;
use std::collections::HashMap;
use std::time::Duration;

#[test]
fn ha_set_data_source_file() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_data_source_file("/tmp/test.json");
}

#[test]
fn ha_set_property_prefix() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_property_prefix("druid.ha");
}

#[test]
fn ha_set_pool_purge_interval_seconds() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_pool_purge_interval_seconds(120);
}

#[test]
fn ha_set_allow_empty_pool_when_update() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_allow_empty_pool_when_update(true);
    ha.set_allow_empty_pool_when_update(false);
}

#[test]
fn ha_set_driver_class_name() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_driver_class_name(Some("com.mysql.Driver".to_owned()));
    ha.set_driver_class_name(None);
}

#[test]
fn ha_set_connect_properties() {
    let ha = HighAvailableDataSource::new("test");
    let mut props = HashMap::new();
    props.insert("user".to_owned(), "admin".to_owned());
    ha.set_connect_properties(Some(props));
    ha.set_connect_properties(None);
}

#[test]
fn ha_set_connection_properties() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_connection_properties(Some("user=admin;password=secret"));
    ha.set_connection_properties(None);
}

#[test]
fn ha_set_initial_size() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_initial_size(5);
}

#[test]
fn ha_set_max_active() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_max_active(20);
}

#[test]
fn ha_set_min_idle() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_min_idle(2);
}

#[test]
fn ha_set_max_wait() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_max_wait(Duration::from_secs(30));
}

#[test]
fn ha_set_validation_query() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_validation_query(Some("SELECT 1".to_owned()));
    ha.set_validation_query(None);
}

#[test]
fn ha_set_validation_query_timeout() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_validation_query_timeout(Duration::from_secs(5));
}

#[test]
fn ha_set_test_while_idle() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_test_while_idle(true);
}

#[test]
fn ha_set_pool_prepared_statements() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_pool_prepared_statements(true);
}

#[test]
fn ha_set_share_prepared_statements() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_share_prepared_statements(true);
}

#[test]
fn ha_set_max_pool_prepared_statement_per_connection_size() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_max_pool_prepared_statement_per_connection_size(50);
}

#[test]
fn ha_set_query_timeout() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_query_timeout(60);
}

#[test]
fn ha_set_transaction_query_timeout() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_transaction_query_timeout(120);
}

#[test]
fn ha_set_time_between_eviction_runs() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_time_between_eviction_runs(Duration::from_secs(60));
}

#[test]
fn ha_set_min_evictable_idle_time() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_min_evictable_idle_time(Duration::from_secs(300));
}

#[test]
fn ha_set_max_evictable_idle_time() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_max_evictable_idle_time(Duration::from_secs(900));
}

#[test]
fn ha_set_physical_timeout() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_physical_timeout(Some(Duration::from_secs(10)));
    ha.set_physical_timeout(None);
}

#[test]
fn ha_set_time_between_connect_error() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_time_between_connect_error(Duration::from_secs(30));
}

#[test]
fn ha_set_remove_abandoned() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_remove_abandoned(true);
}

#[test]
fn ha_set_remove_abandoned_timeout() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_remove_abandoned_timeout(Duration::from_secs(300));
}

#[test]
fn ha_set_log_abandoned() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_log_abandoned(true);
}

#[test]
fn ha_set_filters() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_filters(Some("stat,wall".to_owned()));
    ha.set_filters(None);
}

#[test]
fn ha_set_target_data_source() {
    let ha = HighAvailableDataSource::new("test");
    ha.set_target_data_source(Some("master".to_owned()));
    ha.set_target_data_source(None);
}

#[test]
fn ha_set_node_listener_none() {
    let ha = HighAvailableDataSource::new("test");
    assert!(ha.node_listener().is_none());
}

#[test]
fn ha_test_on_borrow_return() {
    let ha = HighAvailableDataSource::new("test");
    assert!(!ha.is_test_on_borrow());
    ha.set_test_on_borrow(true);
    assert!(ha.is_test_on_borrow());

    assert!(!ha.is_test_on_return());
    ha.set_test_on_return(true);
    assert!(ha.is_test_on_return());
}
