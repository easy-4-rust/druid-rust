//! `java.sql.DataTruncation` 构造器与字段访问器的无损构造合同。
//!
//! Rust 侧 `DataTruncation` 是 pub 字段值对象：Java 的 5 参数构造重载折叠为
//! [`DataTruncation::new`]；read 异常按 Java 约定 SQLState=01004，write 为 22001；
//! `data_size`/`transfer_size` 的未知长度以 None 保留（Java MEASURE_UNKNOWN）。

use druid_rdbc::error::DataTruncation;

#[test]
fn write_truncation_constructor_keeps_all_five_fields() {
    // 对应 Java: DataTruncation(int index, boolean parameter, boolean read,
    // int dataSize, int transferSize)
    let value = DataTruncation::new(3, true, false, Some(64), Some(32));
    assert_eq!(value.index(), 3);
    assert!(value.parameter());
    assert!(!value.read());
    assert_eq!(value.data_size(), Some(64));
    assert_eq!(value.transfer_size(), Some(32));
}

#[test]
fn read_truncation_constructor_keeps_all_five_fields() {
    // 对应 Java: DataTruncation(int index, boolean parameter, boolean read,
    // int dataSize, int transferSize)
    let value = DataTruncation::new(3, false, true, Some(64), Some(32));
    assert_eq!(value.index(), 3);
    assert!(!value.parameter());
    assert!(value.read());
    assert_eq!(value.data_size(), Some(64));
    assert_eq!(value.transfer_size(), Some(32));
}

#[test]
fn unknown_sizes_map_to_java_none() {
    // 对应 Java: dataSize/transferSize 为 MEASURE_UNKNOWN 时的 None 表示。
    let value = DataTruncation::new(1, false, true, None, None);
    assert_eq!(value.data_size(), None);
    assert_eq!(value.transfer_size(), None);
}

#[test]
fn column_and_parameter_positions_are_one_based() {
    // Java index 从 1 计；0 仍按原值保存（Java 允许并区分 unknown）。
    let value = DataTruncation::new(0, true, false, Some(8), Some(4));
    assert_eq!(value.index(), 0);
    assert!(value.parameter());
}
