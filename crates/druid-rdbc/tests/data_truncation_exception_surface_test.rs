//! `java.sql.DataTruncation` 的异常/警告继承面合同。
//!
//! Java 的 `DataTruncation` 继承 `SQLWarning`（`SQLWarning` 又继承 `SQLException`），
//! 因此除五个自有字段外还必须暴露：`SQLState`（读取 `01004` / 写入 `22001`）、
//! vendor code（恒为 0）、原因文本（`Data truncation`）、警告链
//! （`getNextWarning`/`setNextWarning`）以及可遍历的异常链视图。
//!
//! Rust 侧 `DataTruncation` 是值对象：SQLState 与 vendor code 保存在内嵌的
//! [`SqlWarning`] 中，警告链由 `next_warning` 承载，异常链由外层
//! [`SqlException`] 承载。

use druid_rdbc::error::{DataTruncation, SqlException, SqlWarning};

/// Java `DataTruncation` 的固定原因文本。
const JAVA_REASON: &str = "Data truncation";

#[test]
fn read_truncation_uses_jdk8_read_sql_state() {
    // 对应 Java: DataTruncation(int, boolean, boolean, int, int) 中 read=true
    // 时 super("Data truncation", "01004", 0)。
    let value = DataTruncation::new(3, false, true, Some(64), Some(32));

    assert_eq!(value.sql_warning().sql_state(), Some("01004"));
    assert_eq!(value.sql_state(), Some("01004"));
}

#[test]
fn write_truncation_uses_jdk8_write_sql_state() {
    // 对应 Java: read=false 时 super("Data truncation", "22001", 0)。
    let value = DataTruncation::new(3, true, false, Some(64), Some(32));

    assert_eq!(value.sql_warning().sql_state(), Some("22001"));
    assert_eq!(value.sql_state(), Some("22001"));
}

#[test]
fn vendor_code_is_zero_for_both_directions() {
    // 对应 Java: DataTruncation 继承 SQLException#getErrorCode，vendor code 恒为 0。
    for read in [false, true] {
        let value = DataTruncation::new(1, true, read, Some(8), Some(4));

        assert_eq!(value.sql_warning().error_code(), 0);
        assert_eq!(value.error_code(), 0);
    }
}

#[test]
fn reason_text_keeps_java_data_truncation_semantics() {
    // 对应 Java: super("Data truncation", ...)。
    let value = DataTruncation::new(1, false, true, None, None);

    assert_eq!(value.sql_warning().message(), JAVA_REASON);
    assert!(value.sql_warning().message().contains("truncation"));
}

#[test]
fn next_warning_chain_round_trips() {
    // 对应 Java: DataTruncation#setNextWarning(SQLWarning) 与 getNextWarning()。
    let mut tail = SqlWarning::new("tail warning", Some("01000".to_owned()), 7);
    tail.set_next_warning(SqlWarning::new("last warning", Some("01000".to_owned()), 8));

    let mut head = SqlWarning::new("head warning", Some("01000".to_owned()), 6);
    head.set_next_warning(tail);

    let mut value = DataTruncation::new(3, true, false, Some(64), Some(32));
    assert!(value.next_warning().is_none());

    value.set_next_warning(head);
    let head = value
        .next_warning()
        .expect("set_next_warning must publish the chain head");
    assert_eq!(head.message(), "head warning");
    assert_eq!(head.error_code(), 6);
    assert_eq!(
        head.next_warning().map(SqlWarning::message),
        Some("tail warning")
    );
    assert_eq!(
        head.next_warning()
            .and_then(SqlWarning::next_warning)
            .map(SqlWarning::message),
        Some("last warning")
    );
}

#[test]
fn to_sql_warning_keeps_state_vendor_code_and_chain() {
    // 对应 Java: 把 DataTruncation 当作 SQLWarning 观察时的等价视图。
    let mut value = DataTruncation::new(3, false, true, Some(64), Some(32));
    value.set_next_warning(SqlWarning::new("head warning", Some("01000".to_owned()), 6));

    let warning = value.to_sql_warning();
    assert_eq!(warning.message(), JAVA_REASON);
    assert_eq!(warning.sql_state(), Some("01004"));
    assert_eq!(warning.error_code(), 0);
    assert_eq!(
        warning.next_warning().map(SqlWarning::message),
        Some("head warning")
    );
}

#[test]
fn causes_view_iterates_the_warning_chain() {
    // 对应 Java: DataTruncation#iterator()（继承自 SQLException）的可遍历链视图。
    let mut value = DataTruncation::new(3, true, false, Some(64), Some(32));
    let mut tail = SqlWarning::new("tail warning", Some("01000".to_owned()), 7);
    tail.set_next_warning(SqlWarning::new("last warning", Some("01000".to_owned()), 8));
    let mut head = SqlWarning::new("head warning", Some("01000".to_owned()), 6);
    head.set_next_warning(tail);
    value.set_next_warning(head);

    let chain: Vec<&SqlWarning> = value.causes().collect();
    assert_eq!(chain.len(), 4);
    assert_eq!(chain[0].message(), JAVA_REASON);
    assert_eq!(chain[0].sql_state(), Some("22001"));
    assert_eq!(chain[1].message(), "head warning");
    assert_eq!(chain[2].message(), "tail warning");
    assert_eq!(chain[3].message(), "last warning");
}

#[test]
fn five_field_getters_are_unchanged() {
    // 对应 Java: getIndex/getParameter/getRead/getDataSize/getTransferSize。
    let value = DataTruncation::new(3, true, false, Some(64), Some(32));

    assert_eq!(value.index(), 3);
    assert!(value.parameter());
    assert!(!value.read());
    assert_eq!(value.data_size(), Some(64));
    assert_eq!(value.transfer_size(), Some(32));
}

#[test]
fn inherited_exception_chain_lives_on_the_carrying_sql_exception() {
    // 对应 Java: DataTruncation 继承 SQLException 的 getNextException/setNextException；
    // Rust 侧异常链由承载该截断诊断的 SqlException 承担。
    let value = DataTruncation::new(3, true, false, Some(64), Some(32));

    let mut exception = SqlException::new(
        value.error_code(),
        value.sql_state().map(str::to_owned),
        Some(value.sql_warning().message().to_owned()),
    );
    exception.set_next_exception(SqlException::driver(1205, "follow up failure"));

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), Some("22001"));
    assert_eq!(
        exception.next_exception().map(SqlException::error_code),
        Some(1205)
    );
    assert_eq!(
        exception.next_exception().and_then(SqlException::message),
        Some("follow up failure")
    );
}

#[test]
fn unknown_sizes_keep_none_and_java_measure_unknown_semantics() {
    // 对应 Java: dataSize/transferSize 为 MEASURE_UNKNOWN 时的 None 表示。
    let value = DataTruncation::new(1, false, true, None, None);

    assert_eq!(value.data_size(), None);
    assert_eq!(value.transfer_size(), None);
    assert_eq!(value.sql_state(), Some("01004"));
}
