//! `java.sql.ResultSet` 剩余的流/资源更新重载：BufferedRowSet 精确拒绝且不改行状态。
//!
//! 负例覆盖 `updateAsciiStream`、`updateBinaryStream`、`updateNCharacterStream`、
//! `updateBlob(int,InputStream)`、`updateClob(int,Reader)`、`updateNClob(int,Reader)`
//! 以及 `updateObject` 的全部剩余重载。每个重载在索引入口与标签入口都必须返回
//! `RdbcError::SqlException`，SQLState `0A000`，运行时类
//! `java.sql.SQLFeatureNotSupportedException`，且 message 含
//! `buffered_row_set_resource_update`；失败之后当前行、全部列值与 `row_updated`
//! 状态必须保持不变。

use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::resource::{RdbcCharacterLength, RdbcInputStream, RdbcReader, RdbcStreamLength};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use druid_rdbc::types::{RdbcObject, RdbcUrl, Row, Value};

/// 缓冲结果的列标签；每个更新族占用一列，最后一列作为未受影响的对照列。
const LABELS: [&str; 8] = [
    "ascii_col",
    "binary_col",
    "nchar_col",
    "blob_col",
    "clob_col",
    "nclob_col",
    "object_col",
    "control_col",
];

/// 一个必须被精确拒绝的流/资源更新重载。
struct UnsupportedCase {
    /// 索引入口对应的 Java identity。
    identity_index: &'static str,
    /// 标签入口对应的 Java identity。
    identity_label: &'static str,
    /// 目标列（1-based）。
    column: usize,
    /// Rust canonical 更新描述符。
    update: ResultSetUpdate,
}

/// 构造 1-based 列下标。
fn column(index: usize) -> ColumnIndex {
    ColumnIndex::new(index).unwrap()
}

/// 构造一行 8 列的缓冲结果，并移动到当前行。
async fn buffered_row_set() -> BufferedRowSet {
    let values: Vec<Value> = (1..=LABELS.len())
        .map(|offset| Value::Int(offset as i64))
        .collect();
    let mut rows = BufferedRowSet::with_column_labels(
        vec![Row::new(values)],
        LABELS.iter().map(|label| (*label).to_owned()).collect(),
    );
    assert!(rows.next().await.unwrap());
    rows
}

/// 构造调用者持有的输入流。
fn input_stream() -> RdbcInputStream {
    RdbcInputStream::from_bytes(vec![1, 2, 3])
}

/// 构造调用者持有的字符 Reader。
fn reader() -> RdbcReader {
    RdbcReader::from_string("reader")
}

/// 构造非标量通用对象；canonical 更新描述符不携带 SQLType，
/// 因此 `updateObject` 的 SQLType 重载同样以非标量对象表达。
fn non_scalar_object() -> RdbcObject {
    RdbcObject::Url(RdbcUrl::new("https://example.invalid/resource"))
}

/// 构造全部必须被拒绝的流/资源更新重载。
fn unsupported_cases() -> Vec<UnsupportedCase> {
    vec![
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateAsciiStream(int,java.io.InputStream)",
            identity_label: "java.sql.ResultSet#updateAsciiStream(java.lang.String,java.io.InputStream)",
            column: 1,
            update: ResultSetUpdate::AsciiStream {
                stream: Some(input_stream()),
                length: RdbcStreamLength::Unspecified,
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateAsciiStream(int,java.io.InputStream,int)",
            identity_label: "java.sql.ResultSet#updateAsciiStream(java.lang.String,java.io.InputStream,int)",
            column: 1,
            update: ResultSetUpdate::AsciiStream {
                stream: Some(input_stream()),
                length: RdbcStreamLength::Int(3),
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateAsciiStream(int,java.io.InputStream,long)",
            identity_label: "java.sql.ResultSet#updateAsciiStream(java.lang.String,java.io.InputStream,long)",
            column: 1,
            update: ResultSetUpdate::AsciiStream {
                stream: Some(input_stream()),
                length: RdbcStreamLength::Long(3),
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateBinaryStream(int,java.io.InputStream)",
            identity_label: "java.sql.ResultSet#updateBinaryStream(java.lang.String,java.io.InputStream)",
            column: 2,
            update: ResultSetUpdate::BinaryStream {
                stream: Some(input_stream()),
                length: RdbcStreamLength::Unspecified,
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateBinaryStream(int,java.io.InputStream,int)",
            identity_label: "java.sql.ResultSet#updateBinaryStream(java.lang.String,java.io.InputStream,int)",
            column: 2,
            update: ResultSetUpdate::BinaryStream {
                stream: Some(input_stream()),
                length: RdbcStreamLength::Int(3),
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateBinaryStream(int,java.io.InputStream,long)",
            identity_label: "java.sql.ResultSet#updateBinaryStream(java.lang.String,java.io.InputStream,long)",
            column: 2,
            update: ResultSetUpdate::BinaryStream {
                stream: Some(input_stream()),
                length: RdbcStreamLength::Long(3),
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateNCharacterStream(int,java.io.Reader)",
            identity_label: "java.sql.ResultSet#updateNCharacterStream(java.lang.String,java.io.Reader)",
            column: 3,
            update: ResultSetUpdate::NCharacterStream {
                reader: Some(reader()),
                length: RdbcCharacterLength::Unspecified,
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateNCharacterStream(int,java.io.Reader,long)",
            identity_label: "java.sql.ResultSet#updateNCharacterStream(java.lang.String,java.io.Reader,long)",
            column: 3,
            update: ResultSetUpdate::NCharacterStream {
                reader: Some(reader()),
                length: RdbcCharacterLength::Long(6),
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateBlob(int,java.io.InputStream)",
            identity_label: "java.sql.ResultSet#updateBlob(java.lang.String,java.io.InputStream)",
            column: 4,
            update: ResultSetUpdate::BlobStream {
                stream: Some(input_stream()),
                length: RdbcStreamLength::Unspecified,
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateBlob(int,java.io.InputStream,long)",
            identity_label: "java.sql.ResultSet#updateBlob(java.lang.String,java.io.InputStream,long)",
            column: 4,
            update: ResultSetUpdate::BlobStream {
                stream: Some(input_stream()),
                length: RdbcStreamLength::Long(3),
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateClob(int,java.io.Reader)",
            identity_label: "java.sql.ResultSet#updateClob(java.lang.String,java.io.Reader)",
            column: 5,
            update: ResultSetUpdate::ClobReader {
                reader: Some(reader()),
                length: RdbcCharacterLength::Unspecified,
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateClob(int,java.io.Reader,long)",
            identity_label: "java.sql.ResultSet#updateClob(java.lang.String,java.io.Reader,long)",
            column: 5,
            update: ResultSetUpdate::ClobReader {
                reader: Some(reader()),
                length: RdbcCharacterLength::Long(6),
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateNClob(int,java.io.Reader)",
            identity_label: "java.sql.ResultSet#updateNClob(java.lang.String,java.io.Reader)",
            column: 6,
            update: ResultSetUpdate::NClobReader {
                reader: Some(reader()),
                length: RdbcCharacterLength::Unspecified,
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateNClob(int,java.io.Reader,long)",
            identity_label: "java.sql.ResultSet#updateNClob(java.lang.String,java.io.Reader,long)",
            column: 6,
            update: ResultSetUpdate::NClobReader {
                reader: Some(reader()),
                length: RdbcCharacterLength::Long(6),
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateObject(int,java.lang.Object,int)",
            identity_label: "java.sql.ResultSet#updateObject(java.lang.String,java.lang.Object,int)",
            column: 7,
            update: ResultSetUpdate::ObjectWithScaleOrLength {
                value: non_scalar_object(),
                scale_or_length: 3,
            },
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateObject(int,java.lang.Object,java.sql.SQLType)",
            identity_label: "java.sql.ResultSet#updateObject(java.lang.String,java.lang.Object,java.sql.SQLType)",
            column: 7,
            update: ResultSetUpdate::Object(non_scalar_object()),
        },
        UnsupportedCase {
            identity_index: "java.sql.ResultSet#updateObject(int,java.lang.Object,java.sql.SQLType,int)",
            identity_label: "java.sql.ResultSet#updateObject(java.lang.String,java.lang.Object,java.sql.SQLType,int)",
            column: 7,
            update: ResultSetUpdate::ObjectWithScaleOrLength {
                value: non_scalar_object(),
                scale_or_length: 8,
            },
        },
    ]
}

/// 断言错误是精确的 SQLFeatureNotSupportedException。
fn assert_unsupported(error: RdbcError, identity: &str, entry: &str) {
    let RdbcError::SqlException(exception) = error else {
        panic!("{identity} {entry}: expected SQLFeatureNotSupportedException, got {error:?}");
    };
    assert_eq!(
        exception.sql_state(),
        Some("0A000"),
        "{identity} {entry}: SQLState"
    );
    assert_eq!(
        exception.class_name(),
        "java.sql.SQLFeatureNotSupportedException",
        "{identity} {entry}: class"
    );
    assert!(
        exception
            .message()
            .is_some_and(|message| message.contains("buffered_row_set_resource_update")),
        "{identity} {entry}: message {:?}",
        exception.message()
    );
}

/// 断言失败后当前行、全部列值与 row_updated 状态未被改变。
fn assert_state_unchanged(rows: &BufferedRowSet, baseline: &[Value], identity: &str, entry: &str) {
    assert_eq!(rows.row().unwrap(), 1, "{identity} {entry}: current row");
    for (offset, expected) in baseline.iter().enumerate() {
        assert_eq!(
            &rows.value(column(offset + 1)).unwrap(),
            expected,
            "{identity} {entry}: column {} value",
            offset + 1
        );
    }
    assert!(
        !rows.row_updated().unwrap(),
        "{identity} {entry}: row_updated must stay false"
    );
}

/// 索引入口与标签入口都必须精确拒绝流/资源更新，且失败不改变行状态。
#[tokio::test]
async fn buffered_row_set_stream_resource_updates_report_exact_unsupported_error() {
    let cases = unsupported_cases();
    let mut asserted: Vec<&'static str> = Vec::new();

    for case in &cases {
        let mut rows = buffered_row_set().await;
        let baseline: Vec<Value> = (1..=LABELS.len())
            .map(|offset| rows.value(column(offset)).unwrap())
            .collect();

        let index_error = rows
            .update_value(column(case.column), case.update.clone())
            .unwrap_err();
        assert_unsupported(index_error, case.identity_index, "index");
        assert_state_unchanged(&rows, &baseline, case.identity_index, "index");
        asserted.push(case.identity_index);

        let label_error = rows
            .update_value_by_label(LABELS[case.column - 1], case.update.clone())
            .unwrap_err();
        assert_unsupported(label_error, case.identity_label, "label");
        assert_state_unchanged(&rows, &baseline, case.identity_label, "label");
        asserted.push(case.identity_label);
    }

    // 每个 Java identity 都必须被逐项断言一次。
    let mut expected: Vec<&'static str> = cases
        .iter()
        .flat_map(|case| [case.identity_index, case.identity_label])
        .collect();
    expected.sort_unstable();
    let mut actual = asserted;
    actual.sort_unstable();
    assert_eq!(actual, expected, "every update identity must be asserted");
    assert_eq!(actual.len(), 34, "asserted identity count");
}
