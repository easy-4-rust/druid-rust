use druid_admin::model::dto::DataSourceContent;

#[test]
fn task6_admin_counter_keys_use_the_approved_driver_contract() {
    let content = DataSourceContent {
        driver_connect_count: 11,
        driver_close_count: 22,
        driver_connect_error_count: 33,
        ..DataSourceContent::default()
    };

    let encoded = serde_json::to_value(&content).expect("admin DTO must serialize");
    assert_eq!(encoded["DriverConnectCount"], 11);
    assert_eq!(encoded["DriverCloseCount"], 22);
    assert_eq!(encoded["DriverConnectErrorCount"], 33);
    assert!(encoded.get("PhysicalConnectCount").is_none());
    assert!(encoded.get("PhysicalCloseCount").is_none());
    assert!(encoded.get("PhysicalConnectErrorCount").is_none());

    let decoded: DataSourceContent =
        serde_json::from_value(encoded).expect("admin DTO must decode");
    assert_eq!(decoded.driver_connect_count, 11);
    assert_eq!(decoded.driver_close_count, 22);
    assert_eq!(decoded.driver_connect_error_count, 33);
}
