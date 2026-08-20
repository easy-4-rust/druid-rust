//! RED tests for gRPC ingest and repository.
//!
//! Task 3 Step 1: Tests for receiving MetricsBatch via gRPC,
//! updating the in-memory repository, and deduplication.

use std::collections::HashMap;

use druid_admin::model::dto::{DataSourceContent, SqlListContent, WallResult};
use druid_admin::repository::{DataSourceEntry, DataSourceId, MetricsRepository};

fn make_entry(identity: i64, db_type: &str, sequence: u64) -> (DataSourceId, DataSourceEntry) {
    let id = DataSourceId {
        service_id: "svc-1".to_owned(),
        identity,
    };
    let entry = DataSourceEntry {
        datasource: DataSourceContent {
            identity,
            db_type: Some(db_type.to_owned()),
            ..Default::default()
        },
        sql_stats: HashMap::new(),
        wall: WallResult::default(),
        last_updated_ms: 1000,
        sequence,
    };
    (id, entry)
}

/// Repository must accept a new entry and store it.
#[test]
fn repository_accepts_new_entry() {
    let repo = MetricsRepository::new();
    let (id, entry) = make_entry(1, "mysql", 1);
    assert!(
        repo.upsert_datasource(id, entry),
        "first insert must be accepted"
    );
    assert_eq!(repo.len(), 1);
}

/// Repository must reject a duplicate sequence number.
#[test]
fn repository_rejects_duplicate_sequence() {
    let repo = MetricsRepository::new();
    let (id, entry) = make_entry(1, "mysql", 1);
    assert!(repo.upsert_datasource(id.clone(), entry.clone()));
    assert!(
        !repo.upsert_datasource(id, entry),
        "duplicate sequence must be rejected"
    );
    assert_eq!(repo.len(), 1, "length must remain 1");
}

/// Repository must reject a stale (older) sequence number.
#[test]
fn repository_rejects_stale_sequence() {
    let repo = MetricsRepository::new();
    let (id, entry1) = make_entry(1, "mysql", 5);
    assert!(repo.upsert_datasource(id.clone(), entry1));

    let (_, entry2) = make_entry(1, "mysql", 3);
    assert!(
        !repo.upsert_datasource(id, entry2),
        "stale sequence must be rejected"
    );
}

/// Repository must accept a newer sequence number.
#[test]
fn repository_accepts_newer_sequence() {
    let repo = MetricsRepository::new();
    let (id, entry1) = make_entry(1, "mysql", 1);
    assert!(repo.upsert_datasource(id.clone(), entry1));

    let (_, entry2) = make_entry(1, "postgres", 2);
    assert!(
        repo.upsert_datasource(id, entry2),
        "newer sequence must be accepted"
    );

    let all = repo.all_datasources();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].datasource.db_type.as_deref(), Some("postgres"));
}

/// Repository must track ingest and rejected counters.
#[test]
fn repository_tracks_counters() {
    let repo = MetricsRepository::new();
    let (id, entry) = make_entry(1, "mysql", 1);
    repo.upsert_datasource(id.clone(), entry.clone());
    repo.upsert_datasource(id, entry); // duplicate

    let (ingest, rejected) = repo.counters();
    assert_eq!(ingest, 2, "ingest count must be 2");
    assert_eq!(rejected, 1, "rejected count must be 1");
}

/// Repository must isolate entries from different service instances.
#[test]
fn repository_isolates_different_services() {
    let repo = MetricsRepository::new();

    let id1 = DataSourceId {
        service_id: "svc-a".to_owned(),
        identity: 1,
    };
    let id2 = DataSourceId {
        service_id: "svc-b".to_owned(),
        identity: 1,
    };

    let entry1 = DataSourceEntry {
        datasource: DataSourceContent {
            identity: 1,
            db_type: Some("mysql".to_owned()),
            ..Default::default()
        },
        sql_stats: HashMap::new(),
        wall: WallResult::default(),
        last_updated_ms: 0,
        sequence: 1,
    };
    let entry2 = DataSourceEntry {
        datasource: DataSourceContent {
            identity: 1,
            db_type: Some("postgres".to_owned()),
            ..Default::default()
        },
        sql_stats: HashMap::new(),
        wall: WallResult::default(),
        last_updated_ms: 0,
        sequence: 1,
    };

    assert!(repo.upsert_datasource(id1, entry1));
    assert!(repo.upsert_datasource(id2, entry2));
    assert_eq!(repo.len(), 2, "different services must be isolated");
}

/// Repository must aggregate SQL stats across all datasources.
#[test]
fn repository_aggregates_sql_stats() {
    let repo = MetricsRepository::new();

    let mut sql_stats = HashMap::new();
    sql_stats.insert(
        12345,
        SqlListContent {
            id: 1,
            sql: Some("SELECT 1".to_owned()),
            execute_count: 10,
            ..Default::default()
        },
    );

    let id = DataSourceId {
        service_id: "svc-1".to_owned(),
        identity: 1,
    };
    let entry = DataSourceEntry {
        datasource: DataSourceContent::default(),
        sql_stats,
        wall: WallResult::default(),
        last_updated_ms: 0,
        sequence: 1,
    };

    repo.upsert_datasource(id, entry);
    let all_sql = repo.all_sql_stats();
    assert_eq!(all_sql.len(), 1);
    assert_eq!(all_sql[0].sql.as_deref(), Some("SELECT 1"));
}

/// Repository must aggregate wall stats.
#[test]
fn repository_aggregates_wall_stats() {
    let repo = MetricsRepository::new();

    let mut wall = WallResult::default();
    wall.content.check_count = 100;

    let id = DataSourceId {
        service_id: "svc-1".to_owned(),
        identity: 1,
    };
    let entry = DataSourceEntry {
        datasource: DataSourceContent::default(),
        sql_stats: HashMap::new(),
        wall,
        last_updated_ms: 0,
        sequence: 1,
    };

    repo.upsert_datasource(id, entry);
    let agg = repo.aggregated_wall();
    assert_eq!(agg.content.check_count, 100);
}

/// Repository clear must remove all entries.
#[test]
fn repository_clear_removes_all() {
    let repo = MetricsRepository::new();
    let (id, entry) = make_entry(1, "mysql", 1);
    repo.upsert_datasource(id, entry);
    assert!(!repo.is_empty());

    repo.clear();
    assert!(repo.is_empty());
}
