//! `java.sql.ResultSet#updateCharacterStream` 在 BufferedRowSet 上的精确负向合同。
//!
//! 与 `SharedRowSet` 不同，缓冲 ResultSet 不保存 CharacterStream 更新，统一返回
//! `buffered_row_set_resource_update` 的 SQLFeatureNotSupportedException。

use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::resource::{RdbcCharacterLength, RdbcReader};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use druid_rdbc::types::{Row, Value};

fn column(index: usize) -> ColumnIndex {
    ColumnIndex::new(index).unwrap()
}

fn row_set() -> BufferedRowSet {
    BufferedRowSet::with_column_labels(
        vec![Row::new(vec![Value::Int(1)])],
        vec!["text_col".to_owned()],
    )
}

fn assert_unsupported(error: RdbcError, label: &str, entry: &str) {
    let RdbcError::SqlException(exception) = error else {
        panic!("{label} {entry}: expected SQLFeatureNotSupportedException, got {error:?}");
    };
    assert_eq!(exception.sql_state(), Some("0A000"), "{label} {entry}");
    assert_eq!(
        exception.class_name(),
        "java.sql.SQLFeatureNotSupportedException",
        "{label} {entry}"
    );
    assert!(
        exception
            .message()
            .is_some_and(|message| message.contains("buffered_row_set_resource_update")),
        "{label} {entry}: {:?}",
        exception.message()
    );
}

#[tokio::test]
async fn buffered_row_set_character_stream_updates_are_explicitly_unsupported() {
    let updates = [
        (
            "updateCharacterStream(int,Reader)",
            ResultSetUpdate::CharacterStream {
                reader: Some(RdbcReader::from_string("a")),
                length: RdbcCharacterLength::Unspecified,
            },
        ),
        (
            "updateCharacterStream(int,Reader,int)",
            ResultSetUpdate::CharacterStream {
                reader: Some(RdbcReader::from_string("ab")),
                length: RdbcCharacterLength::Int(2),
            },
        ),
        (
            "updateCharacterStream(int,Reader,long)",
            ResultSetUpdate::CharacterStream {
                reader: Some(RdbcReader::from_string("abc")),
                length: RdbcCharacterLength::Long(3),
            },
        ),
    ];
    let mut rows = row_set();
    assert!(rows.next().await.unwrap());
    for (label, update) in updates {
        assert_unsupported(
            rows.update_value(column(1), update.clone()).unwrap_err(),
            label,
            "index",
        );
        assert_unsupported(
            rows.update_value_by_label("text_col", update).unwrap_err(),
            label,
            "label",
        );
        assert_eq!(rows.value(column(1)).unwrap(), Value::Int(1));
        assert_eq!(rows.row().unwrap(), 1);
        assert!(!rows.row_updated().unwrap());
    }
}
