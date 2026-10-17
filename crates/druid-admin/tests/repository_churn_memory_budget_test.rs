//! 验证 Admin 在持续 service/session 更替时的 TTL 回收与进程内存预算。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use druid_admin::model::dto::{DataSourceContent, SqlListContent, WallResult};
use druid_admin::repository::{
    DataSourceEntry, DataSourceId, MetricsRepository, MetricsRepositoryConfig,
};
use druid_metrics::ServiceIdentity;

const GENERATIONS: u64 = 50;
const SERVICES_PER_GENERATION: usize = 32;
const DATASOURCES_PER_SERVICE: usize = 2;
const LIVE_PROJECTIONS: usize = SERVICES_PER_GENERATION * DATASOURCES_PER_SERVICE;
const SQL_BYTES_PER_PROJECTION: usize = 60 * 1024;
const MAX_SAMPLED_PHYSICAL_MEMORY_BYTES: usize = 128 * 1024 * 1024;

fn sample_physical_memory(peak: &mut usize) {
    let current = memory_stats::memory_stats()
        .expect("physical memory measurement must be available")
        .physical_mem;
    assert!(current > 0, "physical memory measurement must be nonzero");
    *peak = (*peak).max(current);
}

fn projection(
    generation: u64,
    service: usize,
    datasource: usize,
) -> (DataSourceId, DataSourceEntry) {
    let boot_id = format!("boot-{generation}");
    let instance_id = format!("pod-{service}");
    let service_identity =
        ServiceIdentity::try_new("orders", &instance_id, &boot_id).expect("valid service identity");
    let identity = i64::try_from(datasource).expect("bounded datasource ID");
    let id = DataSourceId {
        service_identity: service_identity.clone(),
        identity,
    };
    let entry = DataSourceEntry {
        datasource: DataSourceContent {
            identity,
            service_id: Some(service_identity.shared_service_id()),
            service_name: Some("orders".into()),
            instance_id: Some(instance_id.into()),
            boot_id: Some(boot_id.into()),
            active_count: 1,
            ..Default::default()
        },
        sql_stats: HashMap::from([(
            1,
            SqlListContent {
                sql: Some("x".repeat(SQL_BYTES_PER_PROJECTION)),
                ..Default::default()
            },
        )]),
        wall: WallResult::default(),
        last_updated_ms: 1,
        sequence: 1,
    };
    (id, entry)
}

#[test]
fn fifty_expired_session_generations_stay_within_sampled_memory_budget() {
    let config = MetricsRepositoryConfig::try_new(
        SERVICES_PER_GENERATION,
        DATASOURCES_PER_SERVICE,
        1,
        Duration::from_millis(200),
        1 << 20,
    )
    .expect("valid bounded repository");
    let repository = MetricsRepository::with_config(config);
    let mut sampled_peak_bytes = 0;
    let mut batch_micros = Vec::with_capacity(GENERATIONS as usize);
    let mut previous_ids = Vec::new();
    sample_physical_memory(&mut sampled_peak_bytes);

    for generation in 0..GENERATIONS {
        if generation > 0 {
            std::thread::sleep(Duration::from_millis(250));
        }
        sample_physical_memory(&mut sampled_peak_bytes);
        let mut batch = Vec::with_capacity(LIVE_PROJECTIONS);
        for service in 0..SERVICES_PER_GENERATION {
            for datasource in 0..DATASOURCES_PER_SERVICE {
                batch.push(projection(generation, service, datasource));
            }
        }
        let current_ids = batch.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
        let started = Instant::now();
        assert!(
            repository.apply_batch(batch),
            "generation {generation} rejected"
        );
        batch_micros.push(started.elapsed().as_micros());
        sample_physical_memory(&mut sampled_peak_bytes);

        assert_eq!(repository.len(), LIVE_PROJECTIONS);
        assert!(previous_ids
            .iter()
            .all(|id| repository.get_datasource(id).is_none()));
        assert_eq!(
            repository.limit_counters(),
            (0, generation * LIVE_PROJECTIONS as u64, 0),
            "unexpected capacity rejection or TTL eviction count"
        );
        previous_ids = current_ids;
    }

    sample_physical_memory(&mut sampled_peak_bytes);
    let retained_bytes = repository.retained_state_bytes();
    assert!(
        retained_bytes >= LIVE_PROJECTIONS * SQL_BYTES_PER_PROJECTION,
        "the current generation must retain its full SQL payload"
    );
    batch_micros.sort_unstable();
    let percentile =
        |percent: usize| batch_micros[(batch_micros.len() * percent).div_ceil(100) - 1];
    #[cfg(target_os = "windows")]
    let metric_name = "working_set";
    #[cfg(not(target_os = "windows"))]
    let metric_name = "rss";
    eprintln!(
        "admin_session_churn_budget generations={GENERATIONS} retained={} retained_state_bytes={retained_bytes} ttl_evicted={} capacity_rejected=0 batch_p50_us={} batch_p95_us={} batch_p99_us={} sampled_peak_{metric_name}_bytes={sampled_peak_bytes} sampled_memory_limit_bytes={MAX_SAMPLED_PHYSICAL_MEMORY_BYTES}",
        repository.len(),
        repository.limit_counters().1,
        percentile(50),
        percentile(95),
        percentile(99),
    );
    assert!(
        sampled_peak_bytes <= MAX_SAMPLED_PHYSICAL_MEMORY_BYTES,
        "Admin session churn exceeded its 128 MiB sampled physical memory budget"
    );
}
