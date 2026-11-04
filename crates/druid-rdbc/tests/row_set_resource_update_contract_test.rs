//! `javax.sql.RowSet` 继承的资源更新：NULL handle 可写入，LOB 流更新明确不支持。
//!
//! 正例：Ref/Blob/Clob/NClob/Array/RowId/SQLXML 的 NULL handle 经 updateRow +
//! acceptChanges 发布为 SQL NULL（索引与标签入口）。
//! 负例：所有 LOB/流 update 重载都返回 `row_set_lob_stream_update` 的
//! SQLFeatureNotSupportedException，测试逐项断言 class 与 message。

use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::extensions::{
    RowSet, RowSetInternal, RowSetMetaData, RowSetMetaDataBuffer, RowSetReader, RowSetWriter,
    SharedRowSet, SharedRowSetMetaData,
};
use druid_rdbc::resource::{RdbcCharacterLength, RdbcInputStream, RdbcReader, RdbcStreamLength};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use druid_rdbc::types::{RdbcObject, Value};

const LABELS: [&str; 7] = [
    "ref_col",
    "blob_col",
    "clob_col",
    "nclob_col",
    "array_col",
    "rowid_col",
    "xml_col",
];

fn column(index: usize) -> ColumnIndex {
    ColumnIndex::new(index).unwrap()
}

fn metadata() -> SharedRowSetMetaData {
    let mut meta = RowSetMetaDataBuffer::new();
    meta.set_column_count(LABELS.len() as i32).unwrap();
    for (offset, label) in LABELS.iter().enumerate() {
        meta.set_column_label(column(offset + 1), Some((*label).to_owned()))
            .unwrap();
    }
    SharedRowSetMetaData::new(Box::new(meta))
}

struct NonNullReader {
    meta: SharedRowSetMetaData,
}

#[async_trait::async_trait]
impl RowSetReader for NonNullReader {
    async fn read_data(&mut self, caller: &mut dyn RowSetInternal) -> Result<(), RdbcError> {
        caller.set_meta_data(self.meta.clone())?;
        {
            let row_set = caller.row_set();
            row_set.move_to_insert_row()?;
            for offset in 0..LABELS.len() {
                row_set.update_value(column(offset + 1), ResultSetUpdate::Int(1))?;
            }
            row_set.insert_row()?;
            row_set.move_to_current_row()?;
        }
        Ok(())
    }
}

struct AcceptingWriter;

#[async_trait::async_trait]
impl RowSetWriter for AcceptingWriter {
    async fn write_data(&mut self, _caller: &mut dyn RowSetInternal) -> Result<bool, RdbcError> {
        Ok(true)
    }
}

async fn updatable_row_set() -> SharedRowSet {
    let mut source = SharedRowSet::new();
    source
        .set_reader(Some(Box::new(NonNullReader { meta: metadata() })))
        .unwrap();
    source.set_writer(Some(Box::new(AcceptingWriter))).unwrap();
    source.execute().await.unwrap();
    source.set_read_only(false).unwrap();
    source.set_concurrency(1008).unwrap();
    source.set_type(1004).unwrap();
    assert!(source.first().await.unwrap());
    source
}

async fn assert_null_update(index: usize, update: ResultSetUpdate) {
    let mut source = updatable_row_set().await;
    assert_eq!(
        source.object(column(index)).unwrap(),
        RdbcObject::Integer(1),
        "baseline must be non-NULL"
    );
    source.update_value(column(index), update.clone()).unwrap();
    source.update_row().unwrap();
    assert!(source.accept_changes().await.unwrap());
    assert_eq!(
        source.object(column(index)).unwrap(),
        RdbcObject::Scalar(Value::Null),
        "index entry must publish SQL NULL"
    );

    let mut labelled = updatable_row_set().await;
    labelled
        .update_value_by_label(LABELS[index - 1], update)
        .unwrap();
    labelled.update_row().unwrap();
    assert!(labelled.accept_changes().await.unwrap());
    assert_eq!(
        labelled.object_by_label(LABELS[index - 1]).unwrap(),
        RdbcObject::Scalar(Value::Null),
        "label entry must publish SQL NULL"
    );
}

#[tokio::test]
async fn row_set_null_resource_handles_publish_through_index_and_label() {
    assert_null_update(1, ResultSetUpdate::Ref(None)).await;
    assert_null_update(2, ResultSetUpdate::Blob(None)).await;
    assert_null_update(3, ResultSetUpdate::Clob(None)).await;
    assert_null_update(4, ResultSetUpdate::NClob(None)).await;
    assert_null_update(5, ResultSetUpdate::Array(None)).await;
    assert_null_update(6, ResultSetUpdate::RowId(None)).await;
    assert_null_update(7, ResultSetUpdate::SqlXml(None)).await;
}

/// 构造当前 RowSet 明确拒绝的 LOB/流 update 重载（索引与标签入口）。
fn unsupported_stream_updates() -> Vec<(&'static str, ResultSetUpdate)> {
    let stream = || Some(RdbcInputStream::from_bytes(vec![1, 2, 3]));
    let reader = || Some(RdbcReader::from_string("reader"));
    vec![
        (
            "updateAsciiStream(int,InputStream)",
            ResultSetUpdate::AsciiStream {
                stream: stream(),
                length: RdbcStreamLength::Unspecified,
            },
        ),
        (
            "updateAsciiStream(int,InputStream,int)",
            ResultSetUpdate::AsciiStream {
                stream: stream(),
                length: RdbcStreamLength::Int(3),
            },
        ),
        (
            "updateAsciiStream(int,InputStream,long)",
            ResultSetUpdate::AsciiStream {
                stream: stream(),
                length: RdbcStreamLength::Long(3),
            },
        ),
        (
            "updateBinaryStream(int,InputStream)",
            ResultSetUpdate::BinaryStream {
                stream: stream(),
                length: RdbcStreamLength::Unspecified,
            },
        ),
        (
            "updateBinaryStream(int,InputStream,int)",
            ResultSetUpdate::BinaryStream {
                stream: stream(),
                length: RdbcStreamLength::Int(3),
            },
        ),
        (
            "updateBinaryStream(int,InputStream,long)",
            ResultSetUpdate::BinaryStream {
                stream: stream(),
                length: RdbcStreamLength::Long(3),
            },
        ),
        (
            "updateBlob(int,InputStream)",
            ResultSetUpdate::BlobStream {
                stream: stream(),
                length: RdbcStreamLength::Unspecified,
            },
        ),
        (
            "updateBlob(int,InputStream,long)",
            ResultSetUpdate::BlobStream {
                stream: stream(),
                length: RdbcStreamLength::Long(3),
            },
        ),
        (
            "updateClob(int,Reader)",
            ResultSetUpdate::ClobReader {
                reader: reader(),
                length: RdbcCharacterLength::Unspecified,
            },
        ),
        (
            "updateClob(int,Reader,long)",
            ResultSetUpdate::ClobReader {
                reader: reader(),
                length: RdbcCharacterLength::Long(6),
            },
        ),
        (
            "updateNClob(int,Reader)",
            ResultSetUpdate::NClobReader {
                reader: reader(),
                length: RdbcCharacterLength::Unspecified,
            },
        ),
        (
            "updateNClob(int,Reader,long)",
            ResultSetUpdate::NClobReader {
                reader: reader(),
                length: RdbcCharacterLength::Long(6),
            },
        ),
    ]
}

#[tokio::test]
async fn row_set_lob_stream_updates_report_exact_unsupported_error() {
    let mut source = updatable_row_set().await;
    for (label, update) in unsupported_stream_updates() {
        let index_error = source.update_value(column(1), update.clone()).unwrap_err();
        assert_lob_unsupported(index_error, label, "index");

        let label_error = source.update_value_by_label(LABELS[0], update).unwrap_err();
        assert_lob_unsupported(label_error, label, "label");
    }
}

/// Character/NCharacter reader 更新由 RowSet 存储为流值，不属于拒绝路径。
#[tokio::test]
async fn row_set_character_stream_updates_store_the_reader_handle() {
    let mut source = updatable_row_set().await;
    let reader = RdbcReader::from_string("reader");
    source
        .update_value(
            column(1),
            ResultSetUpdate::CharacterStream {
                reader: Some(reader.clone()),
                length: RdbcCharacterLength::Long(6),
            },
        )
        .unwrap();
    source.update_row().unwrap();
    assert!(source.accept_changes().await.unwrap());
    assert!(matches!(
        source.object(column(1)).unwrap(),
        RdbcObject::CharacterStream(_)
    ));
    assert_eq!(reader.read_to_string().unwrap(), "reader");

    let mut labelled = updatable_row_set().await;
    labelled
        .update_value_by_label(
            LABELS[0],
            ResultSetUpdate::NCharacterStream {
                reader: Some(RdbcReader::from_string("n-reader")),
                length: RdbcCharacterLength::Long(8),
            },
        )
        .unwrap();
    labelled.update_row().unwrap();
    assert!(labelled.accept_changes().await.unwrap());
    assert!(matches!(
        labelled.object_by_label(LABELS[0]).unwrap(),
        RdbcObject::NCharacterStream(_)
    ));
}

fn assert_lob_unsupported(error: RdbcError, label: &str, entry: &str) {
    let RdbcError::SqlException(exception) = error else {
        panic!("{label} {entry}: expected SQLFeatureNotSupportedException, got {error:?}");
    };
    assert_feature_not_supported(&exception, label, entry);
}

fn assert_feature_not_supported(exception: &SqlException, label: &str, entry: &str) {
    assert_eq!(
        exception.sql_state(),
        Some("0A000"),
        "{label} {entry}: SQLState"
    );
    assert_eq!(
        exception.class_name(),
        "java.sql.SQLFeatureNotSupportedException",
        "{label} {entry}: class"
    );
    assert!(
        exception
            .message()
            .is_some_and(|message| message.contains("row_set_lob_stream_update")),
        "{label} {entry}: message {:?}",
        exception.message()
    );
}
