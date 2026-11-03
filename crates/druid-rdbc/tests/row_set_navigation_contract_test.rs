//! `javax.sql.RowSet` 继承 `java.sql.ResultSet` 的导航谓词合同。
//!
//! 通过 `SharedRowSet` 的真实 reader / row-store 路径发布三行数据（列标签 `id`、`value`），
//! 再逐项断言 Java ResultSet 导航入口在 RowSet 上的真实观察：
//!
//! - `absolute(int)`：0 与越界返回 `false`；对应 Java: `java.sql.ResultSet#absolute(int)`
//! - `relative(int)`：相对当前位置前进或后退；对应 Java: `java.sql.ResultSet#relative(int)`
//! - `first()` / `last()`：定位首行与末行；对应 Java: `java.sql.ResultSet#first()`、`java.sql.ResultSet#last()`
//! - `before_first()` / `after_last()`：定位两个边界；对应 Java: `java.sql.ResultSet#beforeFirst()`、`java.sql.ResultSet#afterLast()`
//! - `is_first()` / `is_last()` / `is_before_first()` / `is_after_last()`：位置谓词；
//!   对应 Java: `java.sql.ResultSet#isFirst()`、`java.sql.ResultSet#isLast()`、
//!   `java.sql.ResultSet#isBeforeFirst()`、`java.sql.ResultSet#isAfterLast()`
//! - `find_column(String)`：返回 1-based 列号，未知标签报错；
//!   对应 Java: `java.sql.ResultSet#findColumn(String)`
//! - `row()`：作为游标位置证据；对应 Java: `java.sql.ResultSet#getRow()`
//!
//! 每个导航断言之后都读取 `row()` 或当前行列值，证明游标确实移动，而不仅仅返回布尔值。

use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{
    RowSet, RowSetInternal, RowSetMetaData, RowSetMetaDataBuffer, RowSetReader, SharedRowSet,
    SharedRowSetMetaData,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use druid_rdbc::types::RdbcObject;

/// Java `java.sql.ResultSet.TYPE_SCROLL_INSENSITIVE`。
const TYPE_SCROLL_INSENSITIVE: i32 = 1004;

/// 行数固定为 3，`id` 递增，`value` 与行号一一对应。
const ROW_COUNT: i32 = 3;

fn column(index: usize) -> ColumnIndex {
    ColumnIndex::new(index).unwrap()
}

/// 发布三行数据用的元数据：两列，标签为 `id` 与 `value`。
fn metadata() -> SharedRowSetMetaData {
    let mut meta = RowSetMetaDataBuffer::new();
    meta.set_column_count(2).unwrap();
    meta.set_column_label(column(1), Some("id".to_owned()))
        .unwrap();
    meta.set_column_label(column(2), Some("value".to_owned()))
        .unwrap();
    SharedRowSetMetaData::new(Box::new(meta))
}

/// 三行原始值，`id` 为 1/2/3，`value` 为 first/second/third。
fn navigation_rows() -> Vec<Vec<RdbcObject>> {
    vec![
        vec![
            RdbcObject::Integer(1),
            RdbcObject::String("first".to_owned()),
        ],
        vec![
            RdbcObject::Integer(2),
            RdbcObject::String("second".to_owned()),
        ],
        vec![
            RdbcObject::Integer(3),
            RdbcObject::String("third".to_owned()),
        ],
    ]
}

/// 自定义 reader：通过 RowSet 的 insert/update 行入口真实发布三行数据。
struct NavigationReader {
    rows: Vec<Vec<RdbcObject>>,
    meta: SharedRowSetMetaData,
}

#[async_trait::async_trait]
impl RowSetReader for NavigationReader {
    async fn read_data(&mut self, caller: &mut dyn RowSetInternal) -> Result<(), RdbcError> {
        caller.set_meta_data(self.meta.clone())?;
        {
            let row_set = caller.row_set();
            row_set.move_to_insert_row()?;
            for row in &self.rows {
                for (offset, value) in row.iter().enumerate() {
                    row_set
                        .update_value(column(offset + 1), ResultSetUpdate::Object(value.clone()))?;
                }
                row_set.insert_row()?;
            }
            row_set.move_to_current_row()?;
        }
        Ok(())
    }
}

/// 装载一个已发布三行数据、游标停在 before-first 的可滚动 RowSet。
async fn loaded_row_set() -> SharedRowSet {
    let mut source = SharedRowSet::new();
    // RowSet 必须声明可滚动类型，否则 absolute/first/last 等滚动入口被拒绝。
    source.set_type(TYPE_SCROLL_INSENSITIVE).unwrap();
    source
        .set_reader(Some(Box::new(NavigationReader {
            rows: navigation_rows(),
            meta: metadata(),
        })))
        .unwrap();
    RowSet::execute(&mut source).await.unwrap();
    assert_eq!(
        source.known_row_count(),
        Some(ROW_COUNT as u64),
        "reader must publish exactly three rows"
    );
    source
}

/// 读取当前行的 `id` 与 `value`，证明游标停在预期的那一行。
fn assert_current_row(source: &SharedRowSet, id: i32, value: &str) {
    assert_eq!(
        source.object(column(1)).unwrap(),
        RdbcObject::Integer(id),
        "current row id must match the expected row"
    );
    assert_eq!(
        source.object(column(2)).unwrap(),
        RdbcObject::String(value.to_owned()),
        "current row value must match the expected row"
    );
}

#[tokio::test]
async fn row_set_absolute_and_relative_navigation_move_the_cursor() {
    let mut source = loaded_row_set().await;

    // absolute(2) 定位第二行：返回 true，且游标与当前值同时证明移动完成。
    assert!(source.absolute(2).await.unwrap());
    assert_eq!(source.row().unwrap(), 2);
    assert_current_row(&source, 2, "second");

    // absolute(0) 等价 before-first：返回 false，游标回到边界。
    assert!(!source.absolute(0).await.unwrap());
    assert_eq!(source.row().unwrap(), 0);
    assert!(source.is_before_first().unwrap());

    // positive 越界的 absolute 落在 after-last：返回 false。
    assert!(!source.absolute(99).await.unwrap());
    assert_eq!(source.row().unwrap(), 0);
    assert!(source.is_after_last().unwrap());

    // negative 越界的 absolute 落在 before-first：返回 false。
    assert!(!source.absolute(-99).await.unwrap());
    assert_eq!(source.row().unwrap(), 0);
    assert!(source.is_before_first().unwrap());

    // relative 从 before-first 前进一行：返回 true，游标落在第一行。
    assert!(source.relative(1).await.unwrap());
    assert_eq!(source.row().unwrap(), 1);
    assert_current_row(&source, 1, "first");

    // relative 继续前进一行：游标落在第二行。
    assert!(source.relative(1).await.unwrap());
    assert_eq!(source.row().unwrap(), 2);
    assert_current_row(&source, 2, "second");

    // relative(0) 保持当前位置不变。
    assert!(source.relative(0).await.unwrap());
    assert_eq!(source.row().unwrap(), 2);
    assert_current_row(&source, 2, "second");

    // relative(-2) 越过首行回到 before-first：返回 false。
    assert!(!source.relative(-2).await.unwrap());
    assert_eq!(source.row().unwrap(), 0);
    assert!(source.is_before_first().unwrap());
}

#[tokio::test]
async fn row_set_first_last_and_boundary_predicates_track_the_cursor() {
    let mut source = loaded_row_set().await;

    // execute 之后游标位于 before-first，四个位置谓词同时成立/不成立。
    assert_eq!(source.row().unwrap(), 0);
    assert!(source.is_before_first().unwrap());
    assert!(!source.is_after_last().unwrap());
    assert!(!source.is_first().unwrap());
    assert!(!source.is_last().unwrap());

    // first() 落在第一行。
    assert!(source.first().await.unwrap());
    assert_eq!(source.row().unwrap(), 1);
    assert!(source.is_first().unwrap());
    assert!(!source.is_last().unwrap());
    assert!(!source.is_before_first().unwrap());
    assert!(!source.is_after_last().unwrap());
    assert_current_row(&source, 1, "first");

    // last() 落在最后一行。
    assert!(source.last().await.unwrap());
    assert_eq!(source.row().unwrap(), ROW_COUNT);
    assert!(source.is_last().unwrap());
    assert!(!source.is_first().unwrap());
    assert!(!source.is_before_first().unwrap());
    assert!(!source.is_after_last().unwrap());
    assert_current_row(&source, 3, "third");

    // before_first() 回到首个边界。
    source.before_first().await.unwrap();
    assert_eq!(source.row().unwrap(), 0);
    assert!(source.is_before_first().unwrap());
    assert!(!source.is_after_last().unwrap());
    assert!(!source.is_first().unwrap());
    assert!(!source.is_last().unwrap());

    // after_last() 落到尾部边界。
    source.after_last().await.unwrap();
    assert_eq!(source.row().unwrap(), 0);
    assert!(source.is_after_last().unwrap());
    assert!(!source.is_before_first().unwrap());
    assert!(!source.is_first().unwrap());
    assert!(!source.is_last().unwrap());
}

#[tokio::test]
async fn row_set_find_column_returns_one_based_index_and_rejects_unknown_labels() {
    let mut source = loaded_row_set().await;
    assert!(source.first().await.unwrap());

    // find_column 返回 1-based 列号。
    let id = source.find_column("id").unwrap();
    assert_eq!(id.get(), 1);
    let value = source.find_column("value").unwrap();
    assert_eq!(value.get(), 2);

    // 用返回的列号读回当前行，证明列号指向真实列。
    assert_eq!(source.object(id).unwrap(), RdbcObject::Integer(1));
    assert_eq!(
        source.object(value).unwrap(),
        RdbcObject::String("first".to_owned())
    );

    // 未知标签必须报错，且不移动游标。
    let error = source.find_column("missing").unwrap_err();
    assert!(
        matches!(error, RdbcError::SqlException(_)),
        "unknown column label must fail with a SQL exception: {error:?}"
    );
    assert_eq!(source.row().unwrap(), 1);
    assert_eq!(source.object(id).unwrap(), RdbcObject::Integer(1));
}
