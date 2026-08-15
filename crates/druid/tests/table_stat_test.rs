use druid::stats::{
    TableStat, TableStatColumn, TableStatCondition, TableStatMode, TableStatName,
    TableStatRelationship,
};
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// ── TableStat ──────────────────────────────────────────────────

#[test]
fn table_stat_default() {
    let stat = TableStat::default();
    assert_eq!(stat.select_count(), 0);
    assert_eq!(stat.update_count(), 0);
    assert_eq!(stat.delete_count(), 0);
    assert_eq!(stat.insert_count(), 0);
    assert_eq!(stat.drop_count(), 0);
    assert_eq!(stat.merge_count(), 0);
    assert_eq!(stat.create_count(), 0);
    assert_eq!(stat.alter_count(), 0);
    assert_eq!(stat.create_index_count(), 0);
    assert_eq!(stat.drop_index_count(), 0);
    assert_eq!(stat.referenced_count(), 0);
    assert_eq!(stat.add_count(), 0);
    assert_eq!(stat.add_partition_count(), 0);
    assert_eq!(stat.analyze_count(), 0);
}

#[test]
fn table_stat_increment_select() {
    let mut stat = TableStat::default();
    stat.increment_select_count();
    stat.increment_select_count();
    assert_eq!(stat.select_count(), 2);
}

#[test]
fn table_stat_increment_update() {
    let mut stat = TableStat::default();
    stat.increment_update_count();
    assert_eq!(stat.update_count(), 1);
}

#[test]
fn table_stat_increment_delete() {
    let mut stat = TableStat::default();
    stat.increment_delete_count();
    assert_eq!(stat.delete_count(), 1);
}

#[test]
fn table_stat_increment_insert() {
    let mut stat = TableStat::default();
    stat.increment_insert_count();
    assert_eq!(stat.insert_count(), 1);
}

#[test]
fn table_stat_increment_drop() {
    let mut stat = TableStat::default();
    stat.increment_drop_count();
    assert_eq!(stat.drop_count(), 1);
}

#[test]
fn table_stat_increment_merge() {
    let mut stat = TableStat::default();
    stat.increment_merge_count();
    assert_eq!(stat.merge_count(), 1);
}

#[test]
fn table_stat_increment_create() {
    let mut stat = TableStat::default();
    stat.increment_create_count();
    assert_eq!(stat.create_count(), 1);
}

#[test]
fn table_stat_increment_alter() {
    let mut stat = TableStat::default();
    stat.increment_alter_count();
    assert_eq!(stat.alter_count(), 1);
}

#[test]
fn table_stat_increment_create_index() {
    let mut stat = TableStat::default();
    stat.increment_create_index_count();
    assert_eq!(stat.create_index_count(), 1);
}

#[test]
fn table_stat_increment_drop_index() {
    let mut stat = TableStat::default();
    stat.increment_drop_index_count();
    assert_eq!(stat.drop_index_count(), 1);
}

#[test]
fn table_stat_increment_referenced() {
    let mut stat = TableStat::default();
    stat.increment_referenced_count();
    assert_eq!(stat.referenced_count(), 1);
}

#[test]
fn table_stat_increment_add() {
    let mut stat = TableStat::default();
    stat.increment_add_count();
    assert_eq!(stat.add_count(), 1);
}

#[test]
fn table_stat_increment_add_partition() {
    let mut stat = TableStat::default();
    stat.increment_add_partition_count();
    assert_eq!(stat.add_partition_count(), 1);
}

#[test]
fn table_stat_increment_analyze() {
    let mut stat = TableStat::default();
    stat.increment_analyze_count();
    assert_eq!(stat.analyze_count(), 1);
}

#[test]
fn table_stat_setters() {
    let mut stat = TableStat::default();
    stat.set_drop_count(5);
    stat.set_select_count(10);
    stat.set_update_count(3);
    stat.set_delete_count(1);
    stat.set_insert_count(7);
    assert_eq!(stat.drop_count(), 5);
    assert_eq!(stat.select_count(), 10);
    assert_eq!(stat.update_count(), 3);
    assert_eq!(stat.delete_count(), 1);
    assert_eq!(stat.insert_count(), 7);
}

#[test]
fn table_stat_display_empty() {
    let stat = TableStat::default();
    assert_eq!(format!("{}", stat), "");
}

#[test]
fn table_stat_display_with_counts() {
    let mut stat = TableStat::default();
    stat.increment_select_count();
    stat.increment_insert_count();
    let s = format!("{}", stat);
    assert!(s.contains("Insert"));
    assert!(s.contains("Select"));
}

#[test]
fn table_stat_clone_eq() {
    let mut stat = TableStat::default();
    stat.increment_select_count();
    let stat2 = stat.clone();
    assert_eq!(stat, stat2);
}

#[test]
fn table_stat_debug() {
    let stat = TableStat::default();
    let dbg = format!("{:?}", stat);
    assert!(dbg.contains("TableStat"));
}

// ── TableStatName ──────────────────────────────────────────────

#[test]
fn table_stat_name_new() {
    let name = TableStatName::new("users");
    assert_eq!(name.name(), "users");
    assert_ne!(name.hash_code_64(), 0);
}

#[test]
fn table_stat_name_with_hash() {
    let name = TableStatName::with_hash("users", 42);
    assert_eq!(name.name(), "users");
    assert_eq!(name.hash_code_64(), 42);
}

#[test]
fn table_stat_name_eq_by_hash() {
    let n1 = TableStatName::with_hash("a", 42);
    let n2 = TableStatName::with_hash("b", 42);
    assert_eq!(n1, n2);
}

#[test]
fn table_stat_name_ne_by_hash() {
    let n1 = TableStatName::new("users");
    let n2 = TableStatName::new("orders");
    assert_ne!(n1, n2);
}

#[test]
fn table_stat_name_hash_trait() {
    let name = TableStatName::new("users");
    let mut h1 = DefaultHasher::new();
    name.hash(&mut h1);
    let hash1 = h1.finish();
    assert_ne!(hash1, 0);
}

#[test]
fn table_stat_name_display() {
    let name = TableStatName::new("USERS");
    let s = format!("{}", name);
    assert!(!s.is_empty());
}

// ── TableStatRelationship ──────────────────────────────────────

#[test]
fn table_stat_relationship_new() {
    let left = TableStatColumn::new(None, "a.id");
    let right = TableStatColumn::new(None, "b.id");
    let rel = TableStatRelationship::new(left, right, "=");
    assert_eq!(rel.operator(), "=");
}

#[test]
fn table_stat_relationship_display() {
    let left = TableStatColumn::new(None, "a");
    let right = TableStatColumn::new(None, "b");
    let rel = TableStatRelationship::new(left, right, "=");
    let s = format!("{}", rel);
    assert!(s.contains("="));
}

#[test]
fn table_stat_relationship_clone_eq() {
    let left = TableStatColumn::new(None, "a");
    let right = TableStatColumn::new(None, "b");
    let r1 = TableStatRelationship::new(left, right, "=");
    let r2 = r1.clone();
    assert_eq!(r1, r2);
}

// ── TableStatCondition ─────────────────────────────────────────

#[test]
fn table_stat_condition_new() {
    let col = TableStatColumn::new(None, "id");
    let cond = TableStatCondition::new(col, "=");
    assert_eq!(cond.operator(), "=");
    assert!(cond.values().is_empty());
}

#[test]
fn table_stat_condition_add_value() {
    let col = TableStatColumn::new(None, "id");
    let mut cond = TableStatCondition::new(col, "=");
    cond.add_value(Value::Number(42.into()));
    assert_eq!(cond.values().len(), 1);
}

#[test]
fn table_stat_condition_display_empty_values() {
    let col = TableStatColumn::new(None, "id");
    let cond = TableStatCondition::new(col, "IS NULL");
    let s = format!("{}", cond);
    assert!(s.contains("IS NULL"));
}

#[test]
fn table_stat_condition_display_single_value() {
    let col = TableStatColumn::new(None, "id");
    let mut cond = TableStatCondition::new(col, "=");
    cond.add_value(Value::Number(42.into()));
    let s = format!("{}", cond);
    assert!(s.contains("="));
}

#[test]
fn table_stat_condition_display_multiple_values() {
    let col = TableStatColumn::new(None, "id");
    let mut cond = TableStatCondition::new(col, "IN");
    cond.add_value(Value::Number(1.into()));
    cond.add_value(Value::Number(2.into()));
    cond.add_value(Value::Number(3.into()));
    let s = format!("{}", cond);
    assert!(s.contains("IN"));
    assert!(s.contains("("));
}

#[test]
fn table_stat_condition_eq_ignores_values() {
    let col = TableStatColumn::new(None, "id");
    let mut c1 = TableStatCondition::new(col.clone(), "=");
    c1.add_value(Value::Number(1.into()));
    let mut c2 = TableStatCondition::new(col, "=");
    c2.add_value(Value::Number(999.into()));
    assert_eq!(c1, c2);
}

#[test]
fn table_stat_condition_hash_ignores_values() {
    let col = TableStatColumn::new(None, "id");
    let mut c1 = TableStatCondition::new(col.clone(), "=");
    c1.add_value(Value::Number(1.into()));
    let c2 = TableStatCondition::new(col, "=");
    let mut h1 = DefaultHasher::new();
    let mut h2 = DefaultHasher::new();
    c1.hash(&mut h1);
    c2.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
}

// ── TableStatColumn ────────────────────────────────────────────

#[test]
fn table_stat_column_new() {
    let col = TableStatColumn::new(Some("users".to_owned()), "id");
    assert_eq!(col.table(), Some("users"));
    assert_eq!(col.name(), "id");
    assert_eq!(col.full_name(), "users.id");
    assert_ne!(col.hash_code_64(), 0);
}

#[test]
fn table_stat_column_new_no_table() {
    let col = TableStatColumn::new(None, "id");
    assert!(col.table().is_none());
    assert_eq!(col.full_name(), "id");
}

#[test]
fn table_stat_column_flags() {
    let mut col = TableStatColumn::new(None, "id");
    assert!(!col.is_where());
    col.set_where(true);
    assert!(col.is_where());

    assert!(!col.is_select());
    col.set_selec(true);
    assert!(col.is_select());

    assert!(!col.is_group_by());
    col.set_group_by(true);
    assert!(col.is_group_by());

    assert!(!col.is_having());
    col.set_having(true);
    assert!(col.is_having());

    assert!(!col.is_join());
    col.set_join(true);
    assert!(col.is_join());

    assert!(!col.is_primary_key());
    col.set_primary_key(true);
    assert!(col.is_primary_key());

    assert!(!col.is_unique());
    col.set_unique(true);
    assert!(col.is_unique());

    assert!(!col.is_update());
    col.set_update(true);
    assert!(col.is_update());
}

#[test]
fn table_stat_column_data_type() {
    let mut col = TableStatColumn::new(None, "id");
    assert!(col.data_type().is_none());
    col.set_data_type(Some("INTEGER".to_owned()));
    assert_eq!(col.data_type(), Some("INTEGER"));
}

#[test]
fn table_stat_column_attributes() {
    let mut col = TableStatColumn::new(None, "id");
    assert!(col.attributes().is_empty());
    let mut attrs = std::collections::HashMap::new();
    attrs.insert("key".to_owned(), Value::String("val".to_owned()));
    col.set_attributes(attrs);
    assert_eq!(col.attributes().len(), 1);
}

#[test]
fn table_stat_column_display_with_table() {
    let col = TableStatColumn::new(Some("USERS".to_owned()), "ID");
    let s = format!("{}", col);
    assert!(s.contains("."));
}

#[test]
fn table_stat_column_display_without_table() {
    let col = TableStatColumn::new(None, "id");
    let s = format!("{}", col);
    assert!(!s.contains("."));
}

#[test]
fn table_stat_column_clone() {
    let mut col = TableStatColumn::new(Some("t".to_owned()), "c");
    col.set_where(true);
    let col2 = col.clone();
    assert_eq!(col, col2);
    assert!(col2.is_where());
}

#[test]
fn table_stat_column_eq_by_hash() {
    let c1 = TableStatColumn::with_hash(None, "a", 42);
    let c2 = TableStatColumn::with_hash(None, "b", 42);
    assert_eq!(c1, c2);
}

// ── TableStatMode ──────────────────────────────────────────────

#[test]
fn table_stat_mode_values() {
    assert_eq!(TableStatMode::Insert as i32, 1);
    assert_eq!(TableStatMode::Update as i32, 2);
    assert_eq!(TableStatMode::Delete as i32, 4);
    assert_eq!(TableStatMode::Select as i32, 8);
    assert_eq!(TableStatMode::Merge as i32, 16);
    assert_eq!(TableStatMode::Truncate as i32, 32);
    assert_eq!(TableStatMode::Alter as i32, 64);
    assert_eq!(TableStatMode::Drop as i32, 128);
    assert_eq!(TableStatMode::DropIndex as i32, 256);
    assert_eq!(TableStatMode::CreateIndex as i32, 512);
    assert_eq!(TableStatMode::Replace as i32, 1024);
    assert_eq!(TableStatMode::Desc as i32, 2048);
}

#[test]
fn table_stat_mode_clone_eq() {
    let m = TableStatMode::Select;
    let m2 = m;
    assert_eq!(m, m2);
}
