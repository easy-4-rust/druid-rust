extern crate druid_core as druid;
use druid_core::core::RdbcString;
use druid_core::sql::SymbolTable;
use std::sync::Arc;

#[test]
fn symbol_table_new() {
    let st = SymbolTable::new(1024);
    assert!(st.find_symbol(0).is_none());
}

#[test]
fn symbol_table_add_symbol_first_insert() {
    let mut st = SymbolTable::new(16);
    let buf = RdbcString::from_rust_str("hello");
    let val = st.add_symbol(&buf, 0, 5, 42);
    assert_eq!(val.to_rust_string().unwrap(), "hello");
}

#[test]
fn symbol_table_add_symbol_cache_hit() {
    let mut st = SymbolTable::new(16);
    let buf = RdbcString::from_rust_str("hello");
    let v1 = st.add_symbol(&buf, 0, 5, 42);
    let v2 = st.add_symbol(&buf, 0, 5, 42);
    assert!(Arc::ptr_eq(&v1, &v2));
}

#[test]
fn symbol_table_add_symbol_collision_different_hash() {
    let mut st = SymbolTable::new(16);
    let buf = RdbcString::from_rust_str("hello world");
    let v1 = st.add_symbol(&buf, 0, 5, 42);
    let v2 = st.add_symbol(&buf, 6, 5, 99);
    assert_eq!(v1.to_rust_string().unwrap(), "hello");
    assert_eq!(v2.to_rust_string().unwrap(), "world");
}

#[test]
fn symbol_table_find_symbol_hit() {
    let mut st = SymbolTable::new(16);
    let buf = RdbcString::from_rust_str("test");
    st.add_symbol(&buf, 0, 4, 7);
    assert!(st.find_symbol(7).is_some());
    assert_eq!(st.find_symbol(7).unwrap().to_rust_string().unwrap(), "test");
}

#[test]
fn symbol_table_find_symbol_miss() {
    let st = SymbolTable::new(16);
    assert!(st.find_symbol(999).is_none());
}

#[test]
fn symbol_table_add_symbol_bytes() {
    let mut st = SymbolTable::new(16);
    let val = st.add_symbol_bytes(b"hello", 0, 5, 42);
    assert_eq!(val.to_rust_string().unwrap(), "hello");
}

#[test]
fn symbol_table_add_symbol_bytes_cache_hit() {
    let mut st = SymbolTable::new(16);
    let v1 = st.add_symbol_bytes(b"hello", 0, 5, 42);
    let v2 = st.add_symbol_bytes(b"hello", 0, 5, 42);
    assert!(Arc::ptr_eq(&v1, &v2));
}

#[test]
fn symbol_table_add_symbol_bytes_collision() {
    let mut st = SymbolTable::new(16);
    let v1 = st.add_symbol_bytes(b"hello world", 0, 5, 42);
    let v2 = st.add_symbol_bytes(b"hello world", 6, 5, 99);
    assert_eq!(v1.to_rust_string().unwrap(), "hello");
    assert_eq!(v2.to_rust_string().unwrap(), "world");
}

#[test]
fn symbol_table_add_symbol_value_first() {
    let mut st = SymbolTable::new(16);
    let sym = Arc::new(RdbcString::from_rust_str("cached"));
    let val = st.add_symbol_value(Arc::clone(&sym), 7);
    assert!(Arc::ptr_eq(&sym, &val));
}

#[test]
fn symbol_table_add_symbol_value_cache_hit() {
    let mut st = SymbolTable::new(16);
    let s1 = Arc::new(RdbcString::from_rust_str("cached"));
    st.add_symbol_value(Arc::clone(&s1), 7);
    let s2 = Arc::new(RdbcString::from_rust_str("different"));
    let val = st.add_symbol_value(s2, 7);
    assert!(Arc::ptr_eq(&s1, &val));
}

#[test]
fn symbol_table_add_symbol_value_collision() {
    let mut st = SymbolTable::new(16);
    let s1 = Arc::new(RdbcString::from_rust_str("first"));
    st.add_symbol_value(Arc::clone(&s1), 42);
    let s2 = Arc::new(RdbcString::from_rust_str("second"));
    let val = st.add_symbol_value(s2, 99);
    assert_eq!(val.to_rust_string().unwrap(), "second");
}

#[test]
fn symbol_table_substring() {
    let mut st = SymbolTable::new(16);
    let buf = RdbcString::from_rust_str("abcde");
    let val = st.add_symbol(&buf, 1, 3, 1);
    assert_eq!(val.to_rust_string().unwrap(), "bcd");
}

#[test]
fn symbol_table_global_exists() {
    let st = druid_core::sql::GLOBAL_SYMBOL_TABLE.lock().unwrap();
    assert!(st.find_symbol(0).is_none());
}

#[test]
fn symbol_table_debug() {
    let st = SymbolTable::new(8);
    let dbg = format!("{:?}", st);
    assert!(dbg.contains("SymbolTable"));
}
