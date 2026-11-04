//! `javax.sql.RowSet` 剩余的状态/属性方法与 `updateObject` 重载契约。
//!
//! 覆盖行状态标志（rowUpdated/rowInserted/rowDeleted/cancelRowUpdates）、
//! 游标属性（getCursorName/getFetchDirection/getFetchSize/setFetchDirection/
//! setFetchSize/getHoldability）、未实现的 ResultSet 入口（refreshRow/getStatement）、
//! TYPE 常量回读，以及 `updateObject` 的 int/SQLType/scale 六个重载。
//!
//! 不支持的方法必须精确报告 `RdbcError::UnsupportedOperation { operation }`，
//! 可用的方法必须返回真实可断言的值。

use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{
    RowSet, RowSetInternal, RowSetMetaData, RowSetMetaDataBuffer, RowSetReader, RowSetWriter,
    SharedRowSet, SharedRowSetMetaData,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use druid_rdbc::statement::{FetchDirection, ResultSetHoldability, ResultSetType};
use druid_rdbc::types::RdbcObject;

const COLUMNS: usize = 2;
const LABELS: [&str; COLUMNS] = ["c1", "c2"];

/// Java `java.sql.ResultSet.TYPE_SCROLL_INSENSITIVE`：RowSet 允许 first()/last() 滚动。
const TYPE_SCROLL_INSENSITIVE: i32 = 1004;
/// Java `java.sql.ResultSet.CONCUR_UPDATABLE`：RowSet 允许 update/insert/delete。
const CONCUR_UPDATABLE: i32 = 1008;

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

/// 读取器写入两行原始数据：`c1` 分别为 1 和 2。
struct TwoRowReader {
    meta: SharedRowSetMetaData,
}

#[async_trait::async_trait]
impl RowSetReader for TwoRowReader {
    async fn read_data(&mut self, caller: &mut dyn RowSetInternal) -> Result<(), RdbcError> {
        caller.set_meta_data(self.meta.clone())?;
        {
            let row_set = caller.row_set();
            for value in [1_i32, 2_i32] {
                row_set.move_to_insert_row()?;
                row_set.update_value(
                    column(1),
                    ResultSetUpdate::Object(RdbcObject::Integer(value)),
                )?;
                row_set.update_value(
                    column(2),
                    ResultSetUpdate::Object(RdbcObject::Integer(value * 10)),
                )?;
                row_set.insert_row()?;
                row_set.move_to_current_row()?;
            }
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

/// 构造已装载两行数据、可滚动、可更新的 RowSet；游标停在第 1 行。
async fn updatable_row_set() -> SharedRowSet {
    let mut source = SharedRowSet::new();
    source
        .set_reader(Some(Box::new(TwoRowReader { meta: metadata() })))
        .unwrap();
    source.set_writer(Some(Box::new(AcceptingWriter))).unwrap();
    source.execute().await.unwrap();
    source.set_read_only(false).unwrap();
    source.set_concurrency(CONCUR_UPDATABLE).unwrap();
    source.set_type(TYPE_SCROLL_INSENSITIVE).unwrap();
    assert!(source.first().await.unwrap());
    source
}

/// 断言某个入口精确报告不支持，并返回精确的 operation 名。
fn assert_unsupported<T>(result: Result<T, RdbcError>, expected_operation: &str, label: &str) {
    match result {
        Err(RdbcError::UnsupportedOperation { operation }) => {
            assert_eq!(
                operation, expected_operation,
                "{label} must report exact operation"
            );
        }
        Err(other) => panic!("{label} must report UnsupportedOperation, got {other:?}"),
        Ok(_) => panic!("{label} must not succeed"),
    }
}

#[tokio::test]
async fn row_set_row_state_flags_track_update_and_insert() {
    let mut source = updatable_row_set().await;

    // 读取后的原始行既没有更新也没有插入标记。
    assert!(!source.row_updated().unwrap());
    assert!(!source.row_inserted().unwrap());
    assert!(!source.row_deleted().unwrap());
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(1));

    // updateValue + updateRow 之后当前行带 updated 标记。
    source
        .update_value(column(1), ResultSetUpdate::Object(RdbcObject::Integer(100)))
        .unwrap();
    source.update_row().unwrap();
    assert!(source.row_updated().unwrap());
    assert!(!source.row_inserted().unwrap());
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(100));

    // 插入新行后该行没有原始值，rowInserted 为真且 rowUpdated 为假。
    source.move_to_insert_row().unwrap();
    source
        .update_value(column(1), ResultSetUpdate::Object(RdbcObject::Integer(3)))
        .unwrap();
    source.insert_row().unwrap();
    source.move_to_current_row().unwrap();
    assert!(source.last().await.unwrap());
    assert!(source.row_inserted().unwrap());
    assert!(!source.row_updated().unwrap());
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(3));
}

#[tokio::test]
async fn row_set_delete_marks_row_and_row_deleted_reports_current_row() {
    let mut source = updatable_row_set().await;

    // 活着的当前行不是删除行。
    assert!(!source.row_deleted().unwrap());

    // 删除当前行后该行离开可见槽位，当前行不再是已删除行。
    source.delete_row().unwrap();
    assert!(
        !matches!(source.row_deleted(), Ok(true)),
        "deleted row must not be reported as the current visible row"
    );
    // 删除行不再出现在可见行里。
    assert!(source.first().await.unwrap());
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(2));
}

#[tokio::test]
async fn row_set_cancel_row_updates_drops_staged_column_update() {
    let mut source = updatable_row_set().await;

    // 先暂存一列更新：读取会看到 overlay 值。
    source
        .update_value(column(1), ResultSetUpdate::Object(RdbcObject::Integer(999)))
        .unwrap();
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(999));
    assert!(!source.row_updated().unwrap());

    // cancelRowUpdates 丢弃未提交的列更新，读取回到原始值。
    source.cancel_row_updates().unwrap();
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(1));
    assert!(!source.row_updated().unwrap());
}

#[tokio::test]
async fn row_set_fetch_direction_and_fetch_size_round_trip() {
    let mut source = updatable_row_set().await;

    // 初始 fetch direction 为前向，fetch size 为 0。
    assert_eq!(source.fetch_direction().unwrap(), FetchDirection::Forward);
    assert_eq!(source.fetch_size().unwrap(), 0);

    // setFetchDirection(int) 的整数入口与枚举入口保持同一身份。
    assert_eq!(
        FetchDirection::try_from(1000).unwrap(),
        FetchDirection::Forward
    );
    assert_eq!(
        FetchDirection::try_from(1001).unwrap(),
        FetchDirection::Reverse
    );
    assert_eq!(
        FetchDirection::try_from(1002).unwrap(),
        FetchDirection::Unknown
    );
    assert_eq!(FetchDirection::Reverse.rdbc_code(), 1001);

    source.set_fetch_direction(FetchDirection::Reverse).unwrap();
    assert_eq!(source.fetch_direction().unwrap(), FetchDirection::Reverse);
    source.set_fetch_direction(FetchDirection::Unknown).unwrap();
    assert_eq!(source.fetch_direction().unwrap(), FetchDirection::Unknown);

    // setFetchSize 后回读同一个值。
    source.set_fetch_size(25).unwrap();
    assert_eq!(source.fetch_size().unwrap(), 25);
    source.set_fetch_size(0).unwrap();
    assert_eq!(source.fetch_size().unwrap(), 0);
}

#[tokio::test]
async fn row_set_holdability_and_cursor_name_are_observable() {
    let source = updatable_row_set().await;

    assert_eq!(
        source.holdability().unwrap(),
        ResultSetHoldability::HoldOverCommit
    );
    assert_eq!(
        ResultSetHoldability::HoldOverCommit.rdbc_code(),
        1,
        "HOLD_CURSORS_OVER_COMMIT keeps its JDBC code"
    );
    // 行集没有服务端游标名，getCursorName 返回空。
    assert!(source.cursor_name().unwrap().is_none());
}

#[tokio::test]
async fn row_set_refresh_row_and_get_statement_report_unsupported() {
    let mut source = updatable_row_set().await;

    assert_unsupported(source.refresh_row(), "result_set_refresh_row", "refreshRow");
    assert_unsupported(
        source.get_statement(),
        "result_set_get_statement",
        "getStatement",
    );
}

#[tokio::test]
async fn row_set_type_constants_round_trip_through_set_type() {
    // TYPE 常量的 JDBC 数值必须与 Java 8 一致。
    assert_eq!(ResultSetType::ForwardOnly.rdbc_code(), 1003);
    assert_eq!(ResultSetType::ScrollInsensitive.rdbc_code(), 1004);
    assert_eq!(ResultSetType::ScrollSensitive.rdbc_code(), 1005);

    // RowSet#setType 保存同一个数值，getType 回读同一游标类型。
    let mut source = SharedRowSet::new();
    source.set_type(1003).unwrap();
    assert_eq!(
        source.result_set_type().unwrap(),
        ResultSetType::ForwardOnly
    );
    source.set_type(1004).unwrap();
    assert_eq!(
        source.result_set_type().unwrap(),
        ResultSetType::ScrollInsensitive
    );
    source.set_type(1005).unwrap();
    assert_eq!(
        source.result_set_type().unwrap(),
        ResultSetType::ScrollSensitive
    );
}

/// 用可更新 RowSet + AcceptingWriter 验证一个 updateObject 重载真的发布到已接受行。
async fn assert_published_by_index(update: ResultSetUpdate, expected: RdbcObject) {
    let mut source = updatable_row_set().await;
    source.update_value(column(1), update).unwrap();
    source.update_row().unwrap();
    assert!(source.accept_changes().await.unwrap());
    assert_eq!(
        source.object(column(1)).unwrap(),
        expected,
        "index entry must publish the updated object"
    );
}

/// 用标签入口验证同一个 updateObject 重载真的发布到已接受行。
async fn assert_published_by_label(label: &str, update: ResultSetUpdate, expected: RdbcObject) {
    let mut source = updatable_row_set().await;
    source.update_value_by_label(label, update).unwrap();
    source.update_row().unwrap();
    assert!(source.accept_changes().await.unwrap());
    assert_eq!(
        source.object_by_label(label).unwrap(),
        expected,
        "label entry must publish the updated object"
    );
}

#[tokio::test]
async fn row_set_update_object_scale_overloads_publish_values() {
    // updateObject(int columnIndex, Object x, int scaleOrLength)。
    assert_published_by_index(
        ResultSetUpdate::ObjectWithScaleOrLength {
            value: RdbcObject::Integer(11),
            scale_or_length: 2,
        },
        RdbcObject::Integer(11),
    )
    .await;
    assert_published_by_label(
        "c1",
        ResultSetUpdate::ObjectWithScaleOrLength {
            value: RdbcObject::Integer(21),
            scale_or_length: 2,
        },
        RdbcObject::Integer(21),
    )
    .await;

    // updateObject(int columnIndex, Object x, SQLType targetSqlType, int scaleOrLength)。
    assert_published_by_index(
        ResultSetUpdate::ObjectWithScaleOrLength {
            value: RdbcObject::Integer(31),
            scale_or_length: 3,
        },
        RdbcObject::Integer(31),
    )
    .await;
    assert_published_by_label(
        "c1",
        ResultSetUpdate::ObjectWithScaleOrLength {
            value: RdbcObject::Integer(41),
            scale_or_length: 3,
        },
        RdbcObject::Integer(41),
    )
    .await;
}

#[tokio::test]
async fn row_set_update_object_sql_type_overloads_publish_values() {
    // updateObject(int columnIndex, Object x, SQLType targetSqlType)。
    assert_published_by_index(
        ResultSetUpdate::Object(RdbcObject::Integer(12)),
        RdbcObject::Integer(12),
    )
    .await;
    assert_published_by_label(
        "c1",
        ResultSetUpdate::Object(RdbcObject::Integer(22)),
        RdbcObject::Integer(22),
    )
    .await;
}
