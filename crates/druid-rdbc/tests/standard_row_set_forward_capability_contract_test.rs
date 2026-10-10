//! 固定 Java ResultSet 的前向游标导航与 fetchDirection 能力合同。
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{
    RowSet, RowSetInternal, RowSetMetaData, RowSetMetaDataBuffer, SharedRowSet,
    SharedRowSetMetaData,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetNavigation, ResultSetUpdate};
use druid_rdbc::statement::{FetchDirection, ResultSetType};
fn one_row() -> SharedRowSet {
    let mut source = SharedRowSet::new();
    source.set_type(1003).unwrap();
    source.set_read_only(false).unwrap();
    source.set_concurrency(1008).unwrap();
    let mut metadata = RowSetMetaDataBuffer::new();
    metadata.set_column_count(1).unwrap();
    source
        .set_meta_data(SharedRowSetMetaData::new(Box::new(metadata)))
        .unwrap();
    source.move_to_insert_row().unwrap();
    source
        .update_value(ColumnIndex::new(1).unwrap(), ResultSetUpdate::Int(37))
        .unwrap();
    source.insert_row().unwrap();
    source.move_to_current_row().unwrap();
    source
}
#[test]
fn declared_forward_only_rejects_every_scroll_operation_without_moving() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let mut source = one_row();
            assert_eq!(
                source.result_set_type().unwrap(),
                ResultSetType::ForwardOnly
            );
            assert!(source.next().await.unwrap());
            for navigation in [
                ResultSetNavigation::Previous,
                ResultSetNavigation::First,
                ResultSetNavigation::Last,
                ResultSetNavigation::BeforeFirst,
                ResultSetNavigation::AfterLast,
                ResultSetNavigation::Absolute(1),
                ResultSetNavigation::Relative(1),
            ] {
                let result = source.navigate(navigation).await;
                assert!(
                    matches!(result, Err(RdbcError::SqlException(_))),
                    "forward-only must reject {navigation:?}: {result:?}"
                );
                assert_eq!(source.row().unwrap(), 1);
            }
        });
}
#[test]
fn declared_forward_only_rejects_non_forward_fetch_direction_atomically() {
    let mut source = one_row();
    source.set_fetch_direction(FetchDirection::Forward).unwrap();
    for direction in [FetchDirection::Reverse, FetchDirection::Unknown] {
        let result = source.set_fetch_direction(direction);
        assert!(
            matches!(result, Err(RdbcError::SqlException(_))),
            "forward-only must reject {direction:?}: {result:?}"
        );
        assert_eq!(source.fetch_direction().unwrap(), FetchDirection::Forward);
    }
}
#[test]
fn declared_scroll_insensitive_allows_scroll_and_fetch_hints() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let mut source = one_row();
            source.set_type(1004).unwrap();
            assert!(source.first().await.unwrap());
            assert!(source.last().await.unwrap());
            source.before_first().await.unwrap();
            assert_eq!(source.row().unwrap(), 0);
            source.after_last().await.unwrap();
            assert_eq!(source.row().unwrap(), 0);
            assert!(source.absolute(-1).await.unwrap());
            assert!(!source.previous().await.unwrap());
            assert!(source.relative(1).await.unwrap());
            assert_eq!(source.row().unwrap(), 1);
            source.set_fetch_direction(FetchDirection::Reverse).unwrap();
            assert_eq!(source.fetch_direction().unwrap(), FetchDirection::Reverse);
            source.set_fetch_direction(FetchDirection::Unknown).unwrap();
            assert_eq!(source.fetch_direction().unwrap(), FetchDirection::Unknown);
        });
}
