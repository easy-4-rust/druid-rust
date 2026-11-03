//! `javax.sql.RowSet` 继承的 `getObject` 面：无类型、typed（Class）与 type-map 三种入口。
//!
//! 无类型与 typed 入口在共享行集上取值；type-map 入口使用标准默认实现并明确报告不支持，
//! 测试断言精确 operation 名而不把它当作成功。

use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{
    RowSet, RowSetInternal, RowSetMetaData, RowSetMetaDataBuffer, RowSetReader, SharedRowSet,
    SharedRowSetMetaData,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use druid_rdbc::types::{RdbcObject, RdbcTargetType};

fn column(index: usize) -> ColumnIndex {
    ColumnIndex::new(index).unwrap()
}

fn metadata() -> SharedRowSetMetaData {
    let mut meta = RowSetMetaDataBuffer::new();
    meta.set_column_count(2).unwrap();
    meta.set_column_label(column(1), Some("id".to_owned()))
        .unwrap();
    meta.set_column_label(column(2), Some("value".to_owned()))
        .unwrap();
    SharedRowSetMetaData::new(Box::new(meta))
}

struct TwoColumnReader {
    meta: SharedRowSetMetaData,
}

#[async_trait::async_trait]
impl RowSetReader for TwoColumnReader {
    async fn read_data(&mut self, caller: &mut dyn RowSetInternal) -> Result<(), RdbcError> {
        caller.set_meta_data(self.meta.clone())?;
        {
            let row_set = caller.row_set();
            row_set.move_to_insert_row()?;
            row_set.update_value(column(1), ResultSetUpdate::Int(42))?;
            row_set.update_value(column(2), ResultSetUpdate::String(Some("text".to_owned())))?;
            row_set.insert_row()?;
            row_set.move_to_current_row()?;
        }
        Ok(())
    }
}

async fn loaded_row_set() -> SharedRowSet {
    let mut source = SharedRowSet::new();
    source
        .set_reader(Some(Box::new(TwoColumnReader { meta: metadata() })))
        .unwrap();
    RowSet::execute(&mut source).await.unwrap();
    assert!(source.next().await.unwrap());
    source
}

#[tokio::test]
async fn row_set_object_and_typed_object_getters_match_by_index_and_label() {
    let source = loaded_row_set().await;

    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(42));
    assert_eq!(
        source.object_by_label("value").unwrap(),
        RdbcObject::String("text".to_owned())
    );
    assert_eq!(
        source
            .object_as(column(2), &RdbcTargetType::String)
            .unwrap(),
        RdbcObject::String("text".to_owned())
    );
    assert_eq!(
        source
            .object_as_by_label("id", &RdbcTargetType::Integer)
            .unwrap(),
        RdbcObject::Integer(42)
    );
}

#[tokio::test]
async fn row_set_type_map_object_getters_report_the_exact_default_operation() {
    let source = loaded_row_set().await;

    assert!(matches!(
        source.object_with_type_map(column(1), None),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_get_object_with_type_map"
        })
    ));
    assert!(matches!(
        source.object_with_type_map_by_label("value", None),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_get_object_by_label_with_type_map"
        })
    ));
    // 不支持不得改变当前行或 NULL 状态。
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(42));
    assert!(!source.was_null().unwrap());
}
