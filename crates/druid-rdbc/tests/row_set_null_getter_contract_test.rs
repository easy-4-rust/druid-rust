//! `javax.sql.RowSet` 继承的 NULL 资源/流 getter 与状态方法。
//!
//! 逐项覆盖 getArray/getBlob/getClob/getNClob/getRef/getRowId/getSQLXML/getURL/
//! getAsciiStream/getUnicodeStream/getBinaryStream/getCharacterStream/
//! getNCharacterStream 的索引与标签入口；并断言 wasNull、warnings、holdability、
//! previous、Wrapper 与 getBigDecimal(scale)。

use bigdecimal::BigDecimal;
use druid_rdbc::extensions::{
    RowSet, RowSetInternal, RowSetMetaData, RowSetMetaDataBuffer, RowSetReader, SharedRowSet,
    SharedRowSetMetaData,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::statement::ResultSetHoldability;
use druid_rdbc::types::RdbcObject;
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use std::any::TypeId;
use std::str::FromStr;

const LABELS: [&str; 14] = [
    "array_col",
    "blob_col",
    "clob_col",
    "nclob_col",
    "ref_col",
    "rowid_col",
    "xml_col",
    "url_col",
    "ascii_col",
    "unicode_col",
    "binary_col",
    "character_col",
    "ncharacter_col",
    "decimal_col",
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

struct NullRowReader {
    meta: SharedRowSetMetaData,
}

#[async_trait::async_trait]
impl RowSetReader for NullRowReader {
    async fn read_data(
        &mut self,
        caller: &mut dyn RowSetInternal,
    ) -> Result<(), druid_rdbc::error::RdbcError> {
        caller.set_meta_data(self.meta.clone())?;
        {
            let row_set = caller.row_set();
            row_set.move_to_insert_row()?;
            for offset in 0..13 {
                row_set.update_value(
                    column(offset + 1),
                    druid_rdbc::result_set::ResultSetUpdate::Object(RdbcObject::Scalar(
                        druid_rdbc::types::Value::Null,
                    )),
                )?;
            }
            row_set.update_value(
                column(14),
                druid_rdbc::result_set::ResultSetUpdate::BigDecimal(Some(
                    BigDecimal::from_str("123.45").unwrap(),
                )),
            )?;
            row_set.insert_row()?;
            row_set.move_to_current_row()?;
        }
        Ok(())
    }
}

async fn loaded_row_set() -> SharedRowSet {
    let mut source = SharedRowSet::new();
    source
        .set_reader(Some(Box::new(NullRowReader { meta: metadata() })))
        .unwrap();
    source.execute().await.unwrap();
    source.set_type(1004).unwrap();
    assert!(source.first().await.unwrap());
    source
}

#[tokio::test]
async fn row_set_null_resource_and_stream_getters_return_none() {
    let source = loaded_row_set().await;

    assert!(source.array(column(1)).unwrap().is_none());
    assert!(source.array_by_label("array_col").unwrap().is_none());
    assert!(source.blob(column(2)).unwrap().is_none());
    assert!(source.blob_by_label("blob_col").unwrap().is_none());
    assert!(source.clob(column(3)).unwrap().is_none());
    assert!(source.clob_by_label("clob_col").unwrap().is_none());
    assert!(source.n_clob(column(4)).unwrap().is_none());
    assert!(source.n_clob_by_label("nclob_col").unwrap().is_none());
    assert!(source.reference(column(5)).unwrap().is_none());
    assert!(source.reference_by_label("ref_col").unwrap().is_none());
    assert!(source.row_id(column(6)).unwrap().is_none());
    assert!(source.row_id_by_label("rowid_col").unwrap().is_none());
    assert!(source.sql_xml(column(7)).unwrap().is_none());
    assert!(source.sql_xml_by_label("xml_col").unwrap().is_none());
    assert!(ResultSetExt::url(&source, column(8)).unwrap().is_none());
    assert!(ResultSetExt::url_by_label(&source, "url_col")
        .unwrap()
        .is_none());
    assert!(source.ascii_stream(column(9)).unwrap().is_none());
    assert!(source.ascii_stream_by_label("ascii_col").unwrap().is_none());
    assert!(source.unicode_stream(column(10)).unwrap().is_none());
    assert!(source
        .unicode_stream_by_label("unicode_col")
        .unwrap()
        .is_none());
    assert!(source.binary_stream(column(11)).unwrap().is_none());
    assert!(source
        .binary_stream_by_label("binary_col")
        .unwrap()
        .is_none());
    assert!(source.character_stream(column(12)).unwrap().is_none());
    assert!(source
        .character_stream_by_label("character_col")
        .unwrap()
        .is_none());
    assert!(source.n_character_stream(column(13)).unwrap().is_none());
    assert!(source
        .n_character_stream_by_label("ncharacter_col")
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn row_set_state_previous_wrapper_and_decimal_scale_are_observable() {
    let mut source = loaded_row_set().await;

    // 最后读取的是 decimal 列，wasNull 为 false；回到 NULL 列后为 true。
    assert_eq!(
        source.big_decimal_with_scale(column(14), 2).unwrap(),
        Some(BigDecimal::from_str("123.45").unwrap())
    );
    assert_eq!(
        source
            .big_decimal_by_label_with_scale("decimal_col", 2)
            .unwrap(),
        Some(BigDecimal::from_str("123.45").unwrap())
    );
    assert!(!source.was_null().unwrap());
    assert!(source.blob(column(2)).unwrap().is_none());
    assert!(source.was_null().unwrap());

    assert!(source.warnings().unwrap().is_none());
    source.clear_warnings().unwrap();
    assert!(source.warnings().unwrap().is_none());
    assert!(matches!(
        source.holdability().unwrap(),
        ResultSetHoldability::CloseAtCommit
            | ResultSetHoldability::HoldOverCommit
            | ResultSetHoldability::DriverValue(_)
    ));

    // 单行结果集：first() 已在第 1 行，previous() 必须回到 before-first。
    assert!(!source.previous().await.unwrap());
    assert_eq!(source.row().unwrap(), 0);

    let wrapper: &dyn Wrapper = &source;
    assert!(wrapper
        .is_wrapper_for(Some(TypeId::of::<dyn RowSet>()))
        .unwrap());
    match wrapper.unwrap(Some(TypeId::of::<dyn RowSet>())).unwrap() {
        StandardUnwrapped::RowSet(view) => {
            assert!(std::ptr::addr_eq(view, &source as &dyn RowSet))
        }
        other => panic!("expected RowSet view, got {other:?}"),
    }
}
