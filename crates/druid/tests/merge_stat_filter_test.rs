use druid::stats::{MergeStatFilter, StatsCollector};
use std::sync::Arc;
use std::time::Duration;

fn make_filter() -> MergeStatFilter {
    let collector = Arc::new(StatsCollector::new("test", Duration::from_secs(10)));
    MergeStatFilter::new(collector)
}

#[test]
fn merge_stat_filter_new() {
    let f = make_filter();
    assert!(f.is_merge_sql());
}

#[test]
fn merge_stat_filter_as_stat_filter() {
    let f = make_filter();
    let stat = f.as_stat_filter();
    assert!(stat.is_merge_sql());
}

#[test]
fn merge_stat_filter_name() {
    let f = make_filter();
    assert_eq!(druid::core::BeforeFilter::name(&f), "mergeStat");
    assert_eq!(druid::core::AfterFilter::name(&f), "mergeStat");
}
