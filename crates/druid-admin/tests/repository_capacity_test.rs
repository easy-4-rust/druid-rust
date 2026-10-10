//! 验证 Admin 最新指标在实例重启和容量压力下的隔离与回收行为。

use std::collections::HashMap;
use std::time::Duration;

use druid_admin::model::dto::{DataSourceContent, WallResult};
use druid_admin::repository::{
    DataSourceEntry, DataSourceId, MetricsRepository, MetricsRepositoryConfig,
};
use druid_metrics::ServiceIdentity;

fn entry(boot_id: &str, sequence: u64, active_count: i64) -> (DataSourceId, DataSourceEntry) {
    let service_identity = ServiceIdentity::try_new("orders", "pod-1", boot_id).unwrap();
    let id = DataSourceId {
        service_identity: service_identity.clone(),
        identity: 7,
    };
    let entry = DataSourceEntry {
        datasource: DataSourceContent {
            identity: 7,
            service_id: Some(service_identity.shared_service_id()),
            service_name: Some("orders".into()),
            instance_id: Some("pod-1".into()),
            boot_id: Some(boot_id.into()),
            active_count,
            ..Default::default()
        },
        sql_stats: HashMap::new(),
        wall: WallResult::default(),
        last_updated_ms: 1,
        sequence,
    };
    (id, entry)
}

#[test]
fn restarted_pod_sessions_are_isolated_and_capacity_rejection_is_atomic() {
    let config = MetricsRepositoryConfig::try_new(2, 1, 1, Duration::from_secs(10), 1 << 20)
        .expect("valid bounded repository");
    let repository = MetricsRepository::with_config(config);
    let (first_id, first) = entry("boot-1", 9, 11);
    let (second_id, second) = entry("boot-2", 1, 22);

    assert!(repository.upsert_datasource(first_id.clone(), first));
    assert!(repository.upsert_datasource(second_id.clone(), second));
    assert_ne!(first_id.service_identity, second_id.service_identity);
    assert_ne!(
        first_id.service_identity.service_id(),
        second_id.service_identity.service_id()
    );
    assert_eq!(
        repository
            .get_datasource(&first_id)
            .unwrap()
            .datasource
            .active_count,
        11
    );
    assert_eq!(
        repository
            .get_datasource(&second_id)
            .unwrap()
            .datasource
            .active_count,
        22
    );

    let (third_id, third) = entry("boot-3", 1, 33);
    assert!(!repository.replace_stream(&third_id.service_identity, vec![(third_id.clone(), third)]));
    assert_eq!(
        repository.len(),
        2,
        "rejected session cannot evict another boot"
    );
    assert_eq!(repository.limit_counters(), (1, 0, 0));

    assert!(repository.replace_stream(&first_id.service_identity, Vec::new()));
    let (_, third) = entry("boot-3", 1, 33);
    assert!(repository.replace_stream(&third_id.service_identity, vec![(third_id.clone(), third)]));
    assert!(repository.get_datasource(&first_id).is_none());
    assert_eq!(
        repository
            .get_datasource(&second_id)
            .unwrap()
            .datasource
            .active_count,
        22
    );
    assert_eq!(
        repository
            .get_datasource(&third_id)
            .unwrap()
            .datasource
            .active_count,
        33
    );
}

#[test]
fn expired_boot_sessions_release_capacity_using_server_receive_time() {
    let config = MetricsRepositoryConfig::try_new(2, 1, 1, Duration::from_millis(30), 1 << 20)
        .expect("valid short TTL");
    let repository = MetricsRepository::with_config(config);
    let (first_id, first) = entry("boot-1", 1, 1);
    let (second_id, second) = entry("boot-2", 1, 2);
    assert!(repository.upsert_datasource(first_id.clone(), first));
    assert!(repository.upsert_datasource(second_id.clone(), second));

    std::thread::sleep(Duration::from_millis(70));
    let (third_id, third) = entry("boot-3", 1, 3);
    assert!(repository.upsert_datasource(third_id.clone(), third));
    assert_eq!(repository.len(), 1);
    assert!(repository.get_datasource(&first_id).is_none());
    assert!(repository.get_datasource(&second_id).is_none());
    assert!(repository.get_datasource(&third_id).is_some());
    assert_eq!(repository.limit_counters(), (0, 2, 0));
}
