use druid::core::{FilterAdapter, FilterChain};
use std::sync::Arc;

// ── FilterChain basic operations ───────────────────────────────

#[test]
fn filter_chain_new_empty() {
    let chain = FilterChain::new();
    assert!(chain.is_empty());
    assert_eq!(chain.before_count(), 0);
    assert_eq!(chain.after_count(), 0);
    assert_eq!(chain.result_set_count(), 0);
    assert!(chain.filter_class_names().is_empty());
}

#[test]
fn filter_chain_add_single_filter() {
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    assert!(!chain.is_empty());
    assert_eq!(chain.before_count(), 1);
    assert_eq!(chain.after_count(), 1);
    assert_eq!(chain.result_set_count(), 1);
    assert_eq!(chain.filter_class_names().len(), 1);
    assert!(chain.filter_class_names()[0].contains("FilterAdapter"));
}

#[test]
fn filter_chain_add_multiple_filters() {
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    chain.add_filter(Arc::new(FilterAdapter::new()));
    assert_eq!(chain.before_count(), 2);
    assert_eq!(chain.after_count(), 2);
    assert_eq!(chain.result_set_count(), 2);
    assert_eq!(chain.filter_class_names().len(), 2);
}

// ── FilterAdapter basic operations ─────────────────────────────

#[test]
fn filter_adapter_new() {
    let adapter = FilterAdapter::new();
    let _ = adapter;
}

#[test]
fn filter_adapter_name() {
    let adapter = FilterAdapter::new();
    assert_eq!(druid::core::BeforeFilter::name(&adapter), "FilterAdapter");
    assert_eq!(druid::core::AfterFilter::name(&adapter), "FilterAdapter");
}
