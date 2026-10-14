//! 验证 Admin 最新指标在实例重启和容量压力下的隔离与回收行为。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use druid_admin::model::dto::{DataSourceContent, SqlListContent, WallResult};
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

fn record_physical_memory_sample(
    peak: &mut usize,
    sample: Option<usize>,
) -> Result<(), &'static str> {
    let sample = sample.ok_or("physical memory metrics are unavailable")?;
    *peak = (*peak).max(sample);
    Ok(())
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

#[test]
fn global_datasource_and_sql_caps_reject_whole_batches_without_losing_existing_state() {
    let config = MetricsRepositoryConfig::try_new(4, 3, 2, Duration::from_secs(60), 1 << 20)
        .expect("valid repository")
        .with_global_limits(2, 2)
        .expect("valid global caps");
    let repository = MetricsRepository::with_config(config);
    let (first_id, mut first) = entry("boot-first", 1, 10);
    first.sql_stats.insert(1, SqlListContent::default());
    let (second_id, mut second) = entry("boot-second", 1, 20);
    second.sql_stats.insert(2, SqlListContent::default());
    assert!(repository.apply_batch(vec![(first_id.clone(), first), (second_id.clone(), second)]));

    let (third_id, third) = entry("boot-third", 1, 30);
    assert!(!repository.apply_batch(vec![(third_id.clone(), third)]));
    assert!(repository.get_datasource(&third_id).is_none());

    let (same_id, mut expanded) = entry("boot-first", 2, 99);
    expanded.sql_stats.insert(1, SqlListContent::default());
    expanded.sql_stats.insert(3, SqlListContent::default());
    assert!(!repository.apply_batch(vec![(same_id, expanded)]));
    assert_eq!(
        repository
            .get_datasource(&first_id)
            .unwrap()
            .datasource
            .active_count,
        10
    );
    assert_eq!(repository.len(), 2);
    assert_eq!(repository.limit_counters(), (2, 0, 0));

    let (snapshot_id, snapshot_entry) = entry("boot-first", 2, 80);
    let (mut extra_id, mut extra_entry) = entry("boot-first", 2, 81);
    extra_id.identity = 8;
    extra_entry.datasource.identity = 8;
    assert!(!repository.replace_stream(
        &first_id.service_identity,
        vec![(snapshot_id, snapshot_entry), (extra_id, extra_entry)]
    ));
    assert_eq!(
        repository
            .get_datasource(&first_id)
            .unwrap()
            .datasource
            .active_count,
        10
    );
    assert_eq!(repository.len(), 2);
    assert_eq!(repository.limit_counters(), (3, 0, 0));

    let (replacement_id, mut replacement) = entry("boot-first", 2, 40);
    replacement.sql_stats.insert(4, SqlListContent::default());
    assert!(repository.replace_stream(
        &replacement_id.service_identity,
        vec![(replacement_id.clone(), replacement)]
    ));
    assert_eq!(
        repository
            .get_datasource(&replacement_id)
            .unwrap()
            .datasource
            .active_count,
        40
    );
    assert_eq!(repository.len(), 2);
}

#[test]
fn global_caps_must_be_nonzero() {
    let config = MetricsRepositoryConfig::default();
    assert_eq!(config.max_total_datasources(), 4_096);
    assert_eq!(config.max_total_sql_entries(), 16_384);
    assert!(config.clone().with_global_limits(0, 1).is_err());
    assert!(config.clone().with_global_limits(1, 0).is_err());
    assert!(config.with_total_state_bytes(0).is_err());
}

#[test]
fn total_state_bytes_budget_rejects_large_sql_batches_and_reclaims_replaced_bytes() {
    let (first_id, mut first) = entry("boot-aa", 1, 10);
    let (second_id, mut second) = entry("boot-bb", 1, 10);
    first.sql_stats.insert(
        1,
        SqlListContent {
            sql: Some("x".repeat(4_096)),
            ..Default::default()
        },
    );
    second.sql_stats.insert(
        1,
        SqlListContent {
            sql: Some("x".repeat(4_096)),
            ..Default::default()
        },
    );
    let (mut compact_id, compact) = entry("boot-aa", 2, 10);
    compact_id.identity = first_id.identity;
    let projection_bytes = |entry: &DataSourceEntry| {
        serde_json::to_vec(&(&entry.datasource, &entry.sql_stats, &entry.wall))
            .unwrap()
            .len()
    };
    let budget = projection_bytes(&first) + projection_bytes(&compact);
    let single_entry_budget = MetricsRepositoryConfig::default()
        .with_total_state_bytes(projection_bytes(&first) - 1)
        .unwrap();
    let single_entry_repository = MetricsRepository::with_config(single_entry_budget);
    assert!(!single_entry_repository.upsert_datasource(first_id.clone(), first.clone()));
    assert_eq!(single_entry_repository.len(), 0);
    let config = MetricsRepositoryConfig::try_new(2, 2, 1, Duration::from_secs(60), 1 << 20)
        .unwrap()
        .with_total_state_bytes(budget)
        .unwrap();
    let repository = MetricsRepository::with_config(config);

    assert!(repository.upsert_datasource(first_id.clone(), first.clone()));
    assert!(!repository.apply_batch(vec![(second_id.clone(), second.clone())]));
    assert_eq!(repository.len(), 1);
    assert_eq!(repository.get_datasource(&first_id).unwrap().sequence, 1);

    let (mut snapshot_extra_id, mut snapshot_extra) = entry("boot-aa", 2, 10);
    snapshot_extra_id.identity = 8;
    snapshot_extra.datasource.identity = 8;
    snapshot_extra.sql_stats = first.sql_stats.clone();
    snapshot_extra.sql_stats.get_mut(&1).unwrap().sql = Some("x".repeat(4_097));
    assert!(!repository.replace_stream(
        &first_id.service_identity,
        vec![
            (compact_id.clone(), compact.clone()),
            (snapshot_extra_id, snapshot_extra)
        ]
    ));
    assert_eq!(repository.get_datasource(&first_id).unwrap().sequence, 1);

    assert!(repository.upsert_datasource(compact_id, compact));
    assert!(repository.upsert_datasource(second_id, second));
    assert_eq!(repository.len(), 2);
    assert_eq!(repository.limit_counters(), (2, 0, 0));
}

#[test]
fn sampled_physical_memory_peak_tracks_highest_observation_and_requires_metrics() {
    let mut peak = 0;
    record_physical_memory_sample(&mut peak, Some(100)).unwrap();
    record_physical_memory_sample(&mut peak, Some(250)).unwrap();
    record_physical_memory_sample(&mut peak, Some(150)).unwrap();
    assert_eq!(peak, 250);
    assert!(record_physical_memory_sample(&mut peak, None).is_err());
}

#[test]
fn default_repository_projection_budget_keeps_sampled_physical_memory_below_128_mib() {
    const MAX_STATE_BYTES: usize = 64 * 1024 * 1024;
    const SQL_BYTES: usize = 60 * 1024;
    const MAX_SAMPLED_PHYSICAL_MEMORY_BYTES: usize = 128 * 1024 * 1024;

    let config = MetricsRepositoryConfig::try_new(1, 4_096, 1, Duration::from_secs(60), 1 << 20)
        .expect("one service can exercise the global byte budget")
        .with_total_state_bytes(MAX_STATE_BYTES)
        .expect("valid default repository byte budget");
    let repository = MetricsRepository::with_config(config);
    let mut accepted = 0usize;
    let mut sampled_peak_bytes = 0usize;
    record_physical_memory_sample(
        &mut sampled_peak_bytes,
        memory_stats::memory_stats().map(|stats| stats.physical_mem),
    )
    .expect("physical memory metrics must be available on supported CI platforms");

    for identity in 0..4_096 {
        let (mut id, mut value) = entry("boot-rss-budget", 1, 1);
        id.identity = identity;
        value.datasource.identity = identity;
        value.sql_stats.insert(
            1,
            SqlListContent {
                sql: Some("x".repeat(SQL_BYTES)),
                ..Default::default()
            },
        );
        if !repository.upsert_datasource(id, value) {
            break;
        }
        accepted += 1;
        if accepted % 32 == 0 {
            record_physical_memory_sample(
                &mut sampled_peak_bytes,
                memory_stats::memory_stats().map(|stats| stats.physical_mem),
            )
            .expect("physical memory metrics must remain available while sampling");
        }
    }

    let retained_bytes = repository.retained_state_bytes();
    record_physical_memory_sample(
        &mut sampled_peak_bytes,
        memory_stats::memory_stats().map(|stats| stats.physical_mem),
    )
    .expect("final physical memory sample must be available");

    #[cfg(target_os = "windows")]
    let metric_name = "working_set";
    #[cfg(not(target_os = "windows"))]
    let metric_name = "rss";

    eprintln!(
        "admin_repository_budget accepted={} retained_state_bytes={} sampled_peak_{}_bytes={} sampled_memory_limit_bytes={}",
        accepted,
        retained_bytes,
        metric_name,
        sampled_peak_bytes,
        MAX_SAMPLED_PHYSICAL_MEMORY_BYTES
    );
    assert!(
        accepted > 1_000,
        "test must approach the 64 MiB projection cap"
    );
    assert!(retained_bytes <= MAX_STATE_BYTES);
    assert!(
        MAX_STATE_BYTES - retained_bytes < SQL_BYTES * 2,
        "test must fill the projection budget within one record"
    );
    assert!(
        sampled_peak_bytes <= MAX_SAMPLED_PHYSICAL_MEMORY_BYTES,
        "Admin repository exceeded its 128 MiB sampled physical memory budget"
    );
}
