//! Java PreparedStatementKey/Holder/Pool 等价契约。
//!
//! Java oracle：
//! - `com.alibaba.druid.bvt.pool.PreparedStatementKeyTest`
//! - `com.alibaba.druid.bvt.pool.basic.PreparedStatementKeyTest`
//! - `com.alibaba.druid.bvt.pool.PSCacheTest3`

use druid_core::{
    DruidError, PreparedStatementCacheStats, PreparedStatementHolder, PreparedStatementKey,
    PreparedStatementMethodType, PreparedStatementPool, SqlTextPreparedStatement,
};
use std::collections::HashSet;
use std::sync::Arc;

fn key(sql: &str) -> PreparedStatementKey {
    PreparedStatementKey::new(
        Some(sql.to_string()),
        Some("c1".to_string()),
        PreparedStatementMethodType::M1,
    )
    .expect("non-null SQL must create key")
}

fn holder(sql: &str) -> Arc<PreparedStatementHolder> {
    Arc::new(PreparedStatementHolder::new(
        key(sql),
        Arc::new(SqlTextPreparedStatement::new(sql)),
    ))
}

#[test]
fn prepared_statement_key_preserves_every_java_equality_dimension() {
    let base = PreparedStatementKey::full(
        Some("select 1".to_string()),
        Some("catalog".to_string()),
        PreparedStatementMethodType::M3,
        101,
        102,
        103,
        104,
        Some(vec![1, 2]),
        Some(vec!["id".to_string(), "name".to_string()]),
    )
    .unwrap();
    let same = base.clone();
    assert_eq!(base, same);

    let variants = [
        PreparedStatementKey::full(
            Some("select 2".to_string()),
            Some("catalog".to_string()),
            PreparedStatementMethodType::M3,
            101,
            102,
            103,
            104,
            Some(vec![1, 2]),
            Some(vec!["id".to_string(), "name".to_string()]),
        )
        .unwrap(),
        PreparedStatementKey::full(
            Some("select 1".to_string()),
            Some("other".to_string()),
            PreparedStatementMethodType::M3,
            101,
            102,
            103,
            104,
            Some(vec![1, 2]),
            Some(vec!["id".to_string(), "name".to_string()]),
        )
        .unwrap(),
        PreparedStatementKey::full(
            Some("select 1".to_string()),
            Some("catalog".to_string()),
            PreparedStatementMethodType::M2,
            101,
            102,
            103,
            104,
            Some(vec![1, 2]),
            Some(vec!["id".to_string(), "name".to_string()]),
        )
        .unwrap(),
        PreparedStatementKey::full(
            Some("select 1".to_string()),
            Some("catalog".to_string()),
            PreparedStatementMethodType::M3,
            201,
            102,
            103,
            104,
            Some(vec![1, 2]),
            Some(vec!["id".to_string(), "name".to_string()]),
        )
        .unwrap(),
        PreparedStatementKey::full(
            Some("select 1".to_string()),
            Some("catalog".to_string()),
            PreparedStatementMethodType::M3,
            101,
            202,
            103,
            104,
            Some(vec![1, 2]),
            Some(vec!["id".to_string(), "name".to_string()]),
        )
        .unwrap(),
        PreparedStatementKey::full(
            Some("select 1".to_string()),
            Some("catalog".to_string()),
            PreparedStatementMethodType::M3,
            101,
            102,
            203,
            104,
            Some(vec![1, 2]),
            Some(vec!["id".to_string(), "name".to_string()]),
        )
        .unwrap(),
        PreparedStatementKey::full(
            Some("select 1".to_string()),
            Some("catalog".to_string()),
            PreparedStatementMethodType::M3,
            101,
            102,
            103,
            204,
            Some(vec![1, 2]),
            Some(vec!["id".to_string(), "name".to_string()]),
        )
        .unwrap(),
        PreparedStatementKey::full(
            Some("select 1".to_string()),
            Some("catalog".to_string()),
            PreparedStatementMethodType::M3,
            101,
            102,
            103,
            104,
            Some(vec![2, 1]),
            Some(vec!["id".to_string(), "name".to_string()]),
        )
        .unwrap(),
        PreparedStatementKey::full(
            Some("select 1".to_string()),
            Some("catalog".to_string()),
            PreparedStatementMethodType::M3,
            101,
            102,
            103,
            104,
            Some(vec![1, 2]),
            Some(vec!["name".to_string(), "id".to_string()]),
        )
        .unwrap(),
    ];
    for variant in variants {
        assert_ne!(base, variant);
    }

    let mut set = HashSet::new();
    assert!(set.insert(base.clone()));
    assert!(!set.insert(base));
}

#[test]
fn prepared_statement_key_maps_all_java_constructor_overloads_and_null_error() {
    let m1 =
        PreparedStatementKey::new(Some("x".to_string()), None, PreparedStatementMethodType::M1)
            .unwrap();
    let m2 = PreparedStatementKey::with_result_set(
        Some("x".to_string()),
        None,
        PreparedStatementMethodType::M2,
        1,
        2,
    )
    .unwrap();
    let m3 = PreparedStatementKey::with_result_set_holdability(
        Some("x".to_string()),
        None,
        PreparedStatementMethodType::M3,
        1,
        2,
        3,
    )
    .unwrap();
    let m4 = PreparedStatementKey::with_column_indexes(
        Some("x".to_string()),
        None,
        PreparedStatementMethodType::M4,
        Some(vec![4, 5]),
    )
    .unwrap();
    let m5 = PreparedStatementKey::with_column_names(
        Some("x".to_string()),
        None,
        PreparedStatementMethodType::M5,
        Some(vec!["a".to_string(), "b".to_string()]),
    )
    .unwrap();
    let m6 = PreparedStatementKey::with_auto_generated_keys(
        Some("x".to_string()),
        None,
        PreparedStatementMethodType::M6,
        9,
    )
    .unwrap();

    assert_eq!(m1.method_type(), PreparedStatementMethodType::M1);
    assert_eq!(m2.result_set_type(), 1);
    assert_eq!(m2.result_set_concurrency(), 2);
    assert_eq!(m3.result_set_holdability(), 3);
    assert_eq!(m4.column_indexes(), Some([4, 5].as_slice()));
    assert_eq!(
        m5.column_names(),
        Some(["a".to_string(), "b".to_string()].as_slice())
    );
    assert_eq!(m6.auto_generated_keys(), 9);
    assert_eq!(m1.catalog(), None);
    assert_eq!(m1.sql(), "x");

    assert_eq!(
        PreparedStatementKey::new(None, None, PreparedStatementMethodType::M1),
        Err(DruidError::InvalidArgument("sql is null".to_string()))
    );
}

#[test]
fn prepared_statement_holder_matches_java_counters_flags_and_peak_rules() {
    let holder = holder("select 1");
    assert_eq!(holder.hit_count(), 0);
    assert_eq!(holder.fetch_row_peak(), -1);
    assert_eq!(holder.default_row_prefetch(), -1);
    assert_eq!(holder.row_prefetch(), -1);
    assert!(!holder.is_in_use());
    assert!(!holder.is_pooling());
    assert!(!holder.is_enter_oracle_implicit_cache());

    holder.increment_hit_count();
    holder.increment_in_use_count();
    holder.increment_in_use_count();
    holder.decrement_in_use_count();
    holder.set_default_row_prefetch(16);
    holder.set_row_prefetch(8);
    holder.set_fetch_row_peak(10);
    holder.set_fetch_row_peak(5);
    holder.set_enter_oracle_implicit_cache(true);
    holder.set_pooling(true);

    assert_eq!(holder.hit_count(), 1);
    assert_eq!(holder.in_use_count(), 1);
    assert_eq!(holder.default_row_prefetch(), 16);
    assert_eq!(holder.row_prefetch(), 8);
    assert_eq!(holder.fetch_row_peak(), 10);
    assert!(holder.is_enter_oracle_implicit_cache());
    assert!(holder.is_pooling());
}

#[test]
fn prepared_statement_pool_matches_java_hit_miss_share_and_oracle_flags() {
    let stats = Arc::new(PreparedStatementCacheStats::default());
    let mut pool = PreparedStatementPool::new(3, false, true, stats.clone());
    let statement = holder("select 1");

    assert!(pool.get(statement.key()).is_none());
    pool.put(statement.clone());
    assert!(statement.is_pooling());
    assert!(statement.is_enter_oracle_implicit_cache());

    let hit = pool.get(statement.key()).unwrap();
    assert!(Arc::ptr_eq(&hit, &statement));
    assert_eq!(statement.hit_count(), 1);
    assert!(!statement.is_enter_oracle_implicit_cache());

    statement.increment_in_use_count();
    assert!(pool.get(statement.key()).is_none());
    assert_eq!(stats.cached_prepared_statement_hit_count(), 1);
    assert_eq!(stats.cached_prepared_statement_miss_count(), 1);
    assert_eq!(stats.cached_prepared_statement_access_count(), 2);

    let shared_stats = Arc::new(PreparedStatementCacheStats::default());
    let mut shared_pool = PreparedStatementPool::new(3, true, false, shared_stats.clone());
    shared_pool.put(statement.clone());
    assert!(shared_pool.get(statement.key()).is_some());
    assert_eq!(shared_stats.cached_prepared_statement_hit_count(), 1);
}

#[test]
fn prepared_statement_pool_matches_java_pscache3_access_order_lru() {
    let stats = Arc::new(PreparedStatementCacheStats::default());
    let mut pool = PreparedStatementPool::new(3, false, false, stats);
    let h0 = holder("select 0");
    let h1 = holder("select 1");
    let h2 = holder("select 2");
    let h3 = holder("select 3");
    let h4 = holder("select 4");

    pool.put(h0.clone());
    let active_h0 = pool.get(h0.key()).unwrap();
    active_h0.increment_in_use_count();
    pool.put(h1.clone());
    pool.put(h2.clone());
    assert_eq!(pool.size(), 3);

    pool.put(h3.clone());
    assert_eq!(pool.size(), 3);
    assert!(!h0.is_pooling());
    assert!(h1.is_pooling());
    assert!(h2.is_pooling());
    assert!(h3.is_pooling());
    assert!(!h0.statement().is_closed());

    pool.put(h4.clone());
    assert!(!h1.is_pooling());
    assert!(h2.is_pooling());
    assert!(h3.is_pooling());
    assert!(h4.is_pooling());

    active_h0.decrement_in_use_count();
    pool.put(active_h0);
    assert!(h0.is_pooling());
    assert!(!h2.is_pooling());
    assert!(h3.is_pooling());
    assert!(h4.is_pooling());
    assert_eq!(
        pool.keys_in_lru_order()
            .iter()
            .map(PreparedStatementKey::sql)
            .collect::<Vec<_>>(),
        vec!["select 3", "select 4", "select 0"]
    );
}

#[test]
fn clear_closes_idle_statements_but_preserves_in_use_java_reentry_semantics() {
    let stats = Arc::new(PreparedStatementCacheStats::default());
    let mut pool = PreparedStatementPool::new(3, false, false, stats.clone());
    let idle = holder("idle");
    let active = holder("active");
    active.increment_in_use_count();
    pool.put(idle.clone());
    pool.put(active.clone());

    pool.clear();
    assert!(pool.is_empty());
    assert!(idle.statement().is_closed());
    assert!(!active.statement().is_closed());
    assert!(!active.is_pooling());
    assert_eq!(stats.cached_prepared_statement_delete_count(), 1);

    active.decrement_in_use_count();
    pool.put(active.clone());
    assert!(active.is_pooling());
    assert_eq!(pool.size(), 1);
}
