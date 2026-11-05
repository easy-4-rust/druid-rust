//! `javax.sql.RowSet` 剩余两个未评估 identity 的契约测试。
//!
//! 对应 Java：`javax.sql.RowSet#wasNull()@java.sql.ResultSet` 与
//! `javax.sql.RowSet#TYPE`。
//!
//! `wasNull()` 是 `javax.sql.RowSet` 从 `java.sql.ResultSet` 继承的方法，Java 语义是
//! “上一次读取的列是否为 SQL NULL”；`TYPE` 是 `javax.sql.RowSet` 继承自
//! `java.sql.ResultSet` 的常量，数值等于 `ResultSet.TYPE_FORWARD_ONLY`（1003）。

use druid_rdbc::extensions::{
    RowSet, RowSetInternal, RowSetMetaData, RowSetMetaDataBuffer, RowSetReader, SharedRowSet,
    SharedRowSetMetaData,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt, ResultSetUpdate};
use druid_rdbc::statement::ResultSetType;
use druid_rdbc::types::{RdbcObject, Value};

/// 测试行的两列标签。
const LABELS: [&str; 2] = ["null_col", "value_col"];

/// 返回 1-based 列下标。
fn column(index: usize) -> ColumnIndex {
    ColumnIndex::new(index).unwrap()
}

/// 构建两列的只读 metadata。
fn metadata() -> SharedRowSetMetaData {
    let mut meta = RowSetMetaDataBuffer::new();
    meta.set_column_count(LABELS.len() as i32).unwrap();
    for (offset, label) in LABELS.iter().enumerate() {
        meta.set_column_label(column(offset + 1), Some((*label).to_owned()))
            .unwrap();
    }
    SharedRowSetMetaData::new(Box::new(meta))
}

/// 发布一行“NULL 列 + 非 NULL 列”的行集数据。
struct NullAndValueRowReader {
    meta: SharedRowSetMetaData,
}

#[async_trait::async_trait]
impl RowSetReader for NullAndValueRowReader {
    async fn read_data(
        &mut self,
        caller: &mut dyn RowSetInternal,
    ) -> Result<(), druid_rdbc::error::RdbcError> {
        caller.set_meta_data(self.meta.clone())?;
        {
            let row_set = caller.row_set();
            row_set.move_to_insert_row()?;
            row_set.update_value(
                column(1),
                ResultSetUpdate::Object(RdbcObject::Scalar(Value::Null)),
            )?;
            row_set.update_value(
                column(2),
                ResultSetUpdate::Object(RdbcObject::Scalar(Value::String("text".to_owned()))),
            )?;
            row_set.insert_row()?;
            row_set.move_to_current_row()?;
        }
        Ok(())
    }
}

/// 返回已定位在第一行的行集。
async fn loaded_row_set() -> SharedRowSet {
    let mut source = SharedRowSet::new();
    source
        .set_reader(Some(Box::new(NullAndValueRowReader { meta: metadata() })))
        .unwrap();
    source.execute().await.unwrap();
    // 行集默认是 forward-only 游标，定位到第一行之前必须先切成可滚动游标。
    source
        .set_type(ResultSetType::ScrollInsensitive.rdbc_code())
        .unwrap();
    assert!(source.first().await.unwrap());
    source
}

/// `javax.sql.RowSet#wasNull()` 报告上一次读取的列是否为 SQL NULL。
#[tokio::test]
async fn row_set_was_null_reports_the_last_read_column_null_state() {
    let source = loaded_row_set().await;

    // 先读非 NULL 列：wasNull 必须是 false。
    assert_eq!(source.string(column(2)).unwrap(), Some("text".to_owned()));
    assert!(!source.was_null().unwrap());

    // 再读 NULL 列：wasNull 必须变成 true。
    assert_eq!(source.string(column(1)).unwrap(), None);
    assert!(source.was_null().unwrap());
}

/// `javax.sql.RowSet#TYPE` 常量取值等于 `ResultSet.TYPE_FORWARD_ONLY`（1003）。
#[tokio::test]
async fn row_set_type_constant_matches_java_row_set_type() {
    // Java: javax.sql.RowSet#TYPE == java.sql.ResultSet#TYPE_FORWARD_ONLY == 1003。
    assert_eq!(ResultSetType::ForwardOnly.rdbc_code(), 1003);

    // RowSet#setType(1003) 后 getType 回读同一个游标类型。
    let mut source = SharedRowSet::new();
    source
        .set_type(ResultSetType::ForwardOnly.rdbc_code())
        .unwrap();
    assert_eq!(
        source.result_set_type().unwrap(),
        ResultSetType::ForwardOnly
    );
}
