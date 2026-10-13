//! 验证 Admin 最新指标在实例重启和容量压力下的隔离与回收行为。

use std::collections::HashMap;
use std::time::{Duration, Instant};

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

#[test]
fn full_snapshot_cannot_replace_newer_sequence_for_the_same_session() {
    let repository = MetricsRepository::new();
    let (id, newest) = entry("boot-1", 9, 90);
    assert!(repository.upsert_datasource(id.clone(), newest));

    let (_, stale) = entry("boot-1", 8, 80);
    assert!(!repository.replace_stream(&id.service_identity, vec![(id.clone(), stale)]));
    assert_eq!(repository.get_datasource(&id).unwrap().sequence, 9);
    assert_eq!(
        repository
            .get_datasource(&id)
            .unwrap()
            .datasource
            .active_count,
        90
    );

    let mut second_id = id.clone();
    second_id.identity = 8;
    let (_, stale) = entry("boot-1", 8, 80);
    let (_, mut additional) = entry("boot-1", 10, 100);
    additional.datasource.identity = 8;
    assert!(!repository.replace_stream(
        &id.service_identity,
        vec![(id.clone(), stale), (second_id.clone(), additional)]
    ));
    assert!(repository.get_datasource(&second_id).is_none());

    let (_, fresh) = entry("boot-1", 10, 100);
    assert!(repository.replace_stream(&id.service_identity, vec![(id.clone(), fresh)]));
    assert_eq!(repository.get_datasource(&id).unwrap().sequence, 10);
    assert_eq!(
        repository
            .get_datasource(&id)
            .unwrap()
            .datasource
            .active_count,
        100
    );
}

#[test]
fn full_repository_accepts_existing_identity_updates_without_exceeding_either_limit() {
    let config = MetricsRepositoryConfig::try_new(64, 4, 1, Duration::from_secs(60), 1 << 20)
        .expect("valid bounded repository");
    let repository = MetricsRepository::with_config(config);
    for service in 0..64 {
        for datasource in 0..4 {
            let (mut id, mut value) = entry(&format!("boot-{service}"), 1, 1);
            id.identity = datasource;
            value.datasource.identity = datasource;
            assert!(repository.upsert_datasource(id, value));
        }
    }
    assert_eq!(repository.len(), 256);

    let (mut existing_id, mut updated) = entry("boot-0", 2, 99);
    existing_id.identity = 0;
    updated.datasource.identity = 0;
    assert!(repository.upsert_datasource(existing_id.clone(), updated));
    assert_eq!(
        repository
            .get_datasource(&existing_id)
            .unwrap()
            .datasource
            .active_count,
        99
    );

    let (new_service_id, new_service) = entry("boot-over-cap", 1, 1);
    assert!(!repository.upsert_datasource(new_service_id, new_service));
    let (mut extra_id, mut extra) = entry("boot-0", 1, 1);
    extra_id.identity = 4;
    extra.datasource.identity = 4;
    assert!(!repository.upsert_datasource(extra_id, extra));
    assert_eq!(repository.len(), 256);
    assert_eq!(repository.limit_counters(), (2, 0, 0));
}

#[test]
fn expired_session_churn_reclaims_capacity_without_mixing_boot_identities() {
    let config = MetricsRepositoryConfig::try_new(32, 2, 1, Duration::from_secs(1), 1 << 20)
        .expect("valid churn budget");
    let repository = MetricsRepository::with_config(config);
    let mut first_generation = Vec::new();
    let mut insert_micros = Vec::new();

    for generation in 0..2 {
        if generation == 1 {
            std::thread::sleep(Duration::from_millis(1_100));
        }
        for service in 0..32 {
            let boot_id = format!("boot-{}", generation * 32 + service);
            for datasource in 0..2 {
                let (mut id, mut value) = entry(&boot_id, 1, i64::from(generation + 1));
                id.identity = datasource;
                value.datasource.identity = datasource;
                let started = Instant::now();
                assert!(repository.upsert_datasource(id.clone(), value));
                insert_micros.push(started.elapsed().as_micros());
                if generation == 0 {
                    first_generation.push(id);
                }
            }
        }
        assert_eq!(repository.len(), 64);
    }

    assert!(first_generation
        .iter()
        .all(|id| repository.get_datasource(id).is_none()));
    assert_eq!(repository.all_datasources().len(), 64);
    assert_eq!(repository.limit_counters(), (0, 64, 0));
    insert_micros.sort_unstable();
    let percentile =
        |percent: usize| insert_micros[(insert_micros.len() * percent).div_ceil(100) - 1];
    println!(
        "admin_session_churn insert_p50_us={} insert_p95_us={} insert_p99_us={} ttl_evicted=64 retained=64",
        percentile(50),
        percentile(95),
        percentile(99)
    );
}
