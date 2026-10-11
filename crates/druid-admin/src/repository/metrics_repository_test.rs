use std::collections::HashMap;

use druid_metrics::ServiceIdentity;

use super::{DataSourceEntry, DataSourceId, MetricsRepository};
use crate::model::dto::{DataSourceContent, WallResult};

fn entry(sequence: u64, active_count: i64) -> (DataSourceId, DataSourceEntry) {
    let service_identity = ServiceIdentity::try_new("orders", "pod-1", "boot-1").unwrap();
    (
        DataSourceId {
            service_identity,
            identity: 7,
        },
        DataSourceEntry {
            datasource: DataSourceContent {
                identity: 7,
                active_count,
                ..Default::default()
            },
            sql_stats: HashMap::new(),
            wall: WallResult::default(),
            last_updated_ms: 1,
            sequence,
        },
    )
}

#[test]
fn successor_epoch_accepts_reset_sequence_and_rejects_its_own_stale_snapshot() {
    let repository = MetricsRepository::new();
    let (id, first) = entry(9, 9);
    assert!(repository.replace_stream_for_epoch(
        &id.service_identity,
        41,
        vec![(id.clone(), first)]
    ));

    let (_, successor) = entry(1, 10);
    assert!(repository.replace_stream_for_epoch(
        &id.service_identity,
        42,
        vec![(id.clone(), successor)]
    ));
    assert_eq!(
        repository
            .get_datasource(&id)
            .unwrap()
            .datasource
            .active_count,
        10
    );

    let (_, incremental) = entry(2, 12);
    assert!(repository.apply_batch_for_epoch(42, vec![(id.clone(), incremental)]));
    let (_, duplicate_incremental) = entry(2, 13);
    assert!(!repository.apply_batch_for_epoch(42, vec![(id.clone(), duplicate_incremental)]));
    assert_eq!(
        repository
            .get_datasource(&id)
            .unwrap()
            .datasource
            .active_count,
        12
    );

    let (_, stale_successor) = entry(1, 11);
    assert!(!repository.replace_stream_for_epoch(
        &id.service_identity,
        42,
        vec![(id.clone(), stale_successor)]
    ));
    assert_eq!(
        repository
            .get_datasource(&id)
            .unwrap()
            .datasource
            .active_count,
        12
    );
}
