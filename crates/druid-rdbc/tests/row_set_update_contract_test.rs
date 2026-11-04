//! `javax.sql.RowSet` 继承 `ResultSet` 的标量 `updateXxx` 族：索引与标签入口。
//!
//! 每个用例在可更新 RowSet 上执行 update_value + updateRow + acceptChanges，
//! 再读取已发布行，证明更新真的落盘而不是只改 staged 状态。

use bigdecimal::BigDecimal;
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{
    RowSet, RowSetInternal, RowSetMetaData, RowSetMetaDataBuffer, RowSetReader, RowSetWriter,
    SharedRowSet, SharedRowSetMetaData,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use druid_rdbc::types::{RdbcNaiveDate, RdbcNaiveTime, RdbcObject, Value};
use std::str::FromStr;

const COLUMNS: usize = 16;
const LABELS: [&str; COLUMNS] = [
    "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "c9", "c10", "c11", "c12", "c13", "c14", "c15",
    "c16",
];

fn column(index: usize) -> ColumnIndex {
    ColumnIndex::new(index).unwrap()
}

fn metadata() -> SharedRowSetMetaData {
    let mut meta = RowSetMetaDataBuffer::new();
    meta.set_column_count(COLUMNS as i32).unwrap();
    for (offset, label) in LABELS.iter().enumerate() {
        meta.set_column_label(column(offset + 1), Some((*label).to_owned()))
            .unwrap();
    }
    SharedRowSetMetaData::new(Box::new(meta))
}

struct OneRowReader {
    meta: SharedRowSetMetaData,
}

#[async_trait::async_trait]
impl RowSetReader for OneRowReader {
    async fn read_data(&mut self, caller: &mut dyn RowSetInternal) -> Result<(), RdbcError> {
        caller.set_meta_data(self.meta.clone())?;
        {
            let row_set = caller.row_set();
            row_set.move_to_insert_row()?;
            for offset in 0..COLUMNS {
                row_set.update_value(
                    column(offset + 1),
                    ResultSetUpdate::Object(RdbcObject::Scalar(Value::Null)),
                )?;
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
        .set_reader(Some(Box::new(OneRowReader { meta: metadata() })))
        .unwrap();
    source.set_writer(Some(Box::new(AcceptingWriter))).unwrap();
    source.execute().await.unwrap();
    source.set_read_only(false).unwrap();
    source.set_concurrency(1008).unwrap();
    // ResultSet.TYPE_SCROLL_INSENSITIVE：默认 ForwardOnly 不允许 first()。
    source.set_type(1004).unwrap();
    assert!(source.first().await.unwrap());
    source
}

async fn assert_update(index: usize, update: ResultSetUpdate, expected: RdbcObject) {
    // 索引入口。
    let mut source = updatable_row_set().await;
    source.update_value(column(index), update.clone()).unwrap();
    source.update_row().unwrap();
    assert!(source.accept_changes().await.unwrap());
    assert_eq!(
        source.object(column(index)).unwrap(),
        expected,
        "index entry must publish column {index}"
    );

    // 标签入口。
    let mut labelled = updatable_row_set().await;
    labelled
        .update_value_by_label(LABELS[index - 1], update)
        .unwrap();
    labelled.update_row().unwrap();
    assert!(labelled.accept_changes().await.unwrap());
    assert_eq!(
        labelled.object_by_label(LABELS[index - 1]).unwrap(),
        expected,
        "label entry must publish column {index}"
    );
}

#[tokio::test]
async fn row_set_scalar_update_families_publish_through_index_and_label() {
    let date = RdbcNaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
    let time = RdbcNaiveTime::from_hms_nano_opt(8, 9, 10, 111_000_000).unwrap();
    let timestamp = date.and_time(time);

    assert_update(1, ResultSetUpdate::Null, RdbcObject::Scalar(Value::Null)).await;
    assert_update(2, ResultSetUpdate::Boolean(true), RdbcObject::Boolean(true)).await;
    assert_update(3, ResultSetUpdate::Byte(-8), RdbcObject::Byte(-8)).await;
    assert_update(4, ResultSetUpdate::Short(-300), RdbcObject::Short(-300)).await;
    assert_update(5, ResultSetUpdate::Int(42), RdbcObject::Integer(42)).await;
    assert_update(
        6,
        ResultSetUpdate::Long(9_000_000_000),
        RdbcObject::Long(9_000_000_000),
    )
    .await;
    assert_update(7, ResultSetUpdate::Float(1.5), RdbcObject::Float(1.5)).await;
    assert_update(8, ResultSetUpdate::Double(-2.5), RdbcObject::Double(-2.5)).await;
    assert_update(
        9,
        ResultSetUpdate::String(Some("text".to_owned())),
        RdbcObject::String("text".to_owned()),
    )
    .await;
    assert_update(
        10,
        ResultSetUpdate::NString(Some("国字".to_owned())),
        RdbcObject::NString("国字".to_owned()),
    )
    .await;
    assert_update(
        11,
        ResultSetUpdate::Bytes(Some(vec![1, 2, 3])),
        RdbcObject::Bytes(vec![1, 2, 3]),
    )
    .await;
    assert_update(
        12,
        ResultSetUpdate::BigDecimal(Some(BigDecimal::from_str("123.45").unwrap())),
        RdbcObject::BigDecimal(BigDecimal::from_str("123.45").unwrap()),
    )
    .await;
    assert_update(
        13,
        ResultSetUpdate::Date(Some(date)),
        RdbcObject::Date(date),
    )
    .await;
    assert_update(
        14,
        ResultSetUpdate::Time(Some(time)),
        RdbcObject::Time(time),
    )
    .await;
    assert_update(
        15,
        ResultSetUpdate::Timestamp(Some(timestamp)),
        RdbcObject::Timestamp(timestamp),
    )
    .await;
    assert_update(
        16,
        ResultSetUpdate::Object(RdbcObject::Integer(77)),
        RdbcObject::Integer(77),
    )
    .await;
}

#[tokio::test]
async fn row_set_update_rejects_missing_current_row() {
    let mut source = updatable_row_set().await;
    source.after_last().await.unwrap();
    assert!(source
        .update_value(column(1), ResultSetUpdate::Int(1))
        .is_err());
    assert!(source
        .update_value_by_label("c1", ResultSetUpdate::Int(1))
        .is_err());
}
