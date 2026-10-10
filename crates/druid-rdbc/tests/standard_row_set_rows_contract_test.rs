//! RowSetInternal 原始行与 disconnected 写回所需的真实 rich-row 状态合同。
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{RowSetRowChange, RowSetRowStore};
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::types::{RdbcObject, RdbcReader, Value};

fn column(value: usize) -> ColumnIndex {
    ColumnIndex::new(value).unwrap()
}
fn row(a: i32, b: &str) -> Vec<RdbcObject> {
    vec![RdbcObject::Integer(a), RdbcObject::String(b.into())]
}
fn sql_error<T>(result: Result<T, RdbcError>) {
    assert!(matches!(result, Err(RdbcError::SqlException(_))));
}

#[test]
fn original_snapshots_preserve_rich_values_and_shared_reader_cursor_without_consumption() {
    let reader = RdbcReader::from_utf16(vec![0x0061, 0xd800, 0x0062]);
    let original = vec![
        RdbcObject::NCharacterStream(reader.clone()),
        RdbcObject::Scalar(Value::Null),
    ];
    let mut store = RowSetRowStore::new(2);
    store.replace_rows(vec![original.clone()]).unwrap();
    let snapshot = store.original_rows();
    assert_eq!(snapshot, vec![original.clone()]);
    assert_eq!(store.original_row(Some(1)).unwrap(), vec![original.clone()]);
    assert_eq!(store.value(1, column(1)).unwrap(), original[0]);
    let RdbcObject::NCharacterStream(shared) = &snapshot[0][0] else {
        panic!("rich reader identity required")
    };
    let mut unit = [0];
    assert_eq!(shared.read_utf16(&mut unit).unwrap(), 1);
    assert_eq!(unit, [0x0061]);
    store
        .stage_update(1, column(1), RdbcObject::String("replacement".into()))
        .unwrap();
    store.update_row(1).unwrap();
    assert_eq!(store.original_rows(), snapshot);
    store.restore_originals();
    assert_eq!(store.value(1, column(1)).unwrap(), original[0]);
    drop(store);
    assert!(!reader.is_closed());
    let mut remainder = [0; 2];
    assert_eq!(reader.read_utf16(&mut remainder).unwrap(), 2);
    assert_eq!(remainder, [0xd800, 0x0062]);
}

#[test]
fn invalid_widths_indices_and_replacements_are_atomic_without_sparse_allocation() {
    let mut store = RowSetRowStore::new(2);
    let baseline = vec![row(1, "first"), row(2, "second")];
    store.replace_rows(baseline.clone()).unwrap();
    sql_error(store.replace_rows(vec![row(3, "valid prefix"), vec![RdbcObject::Integer(4)]]));
    assert_eq!(store.original_rows(), baseline);
    assert_eq!(store.len(), 2);
    sql_error(store.insert_row(vec![RdbcObject::Integer(3)]));
    assert_eq!(store.len(), 2);
    for invalid in [0, 3, usize::MAX] {
        sql_error(store.value(invalid, column(1)));
        sql_error(store.stage_update(invalid, column(1), RdbcObject::Integer(9)));
        sql_error(store.update_row(invalid));
        sql_error(store.cancel_row_updates(invalid));
        sql_error(store.delete_row(invalid));
        sql_error(store.original_row(Some(invalid)));
    }
    for invalid in [3, usize::MAX] {
        sql_error(store.value(1, column(invalid)));
        sql_error(store.stage_update(1, column(invalid), RdbcObject::Integer(9)));
    }
    sql_error(store.original_row(None));
    assert_eq!(store.original_rows(), baseline);
    assert!(store.changes().unwrap().is_empty());
}

#[test]
fn staged_updates_overlay_current_values_cancel_independently_and_commit_last_value() {
    let mut store = RowSetRowStore::new(2);
    store
        .replace_rows(vec![row(1, "first"), row(2, "second")])
        .unwrap();
    let baseline = store.original_rows();
    store
        .stage_update(1, column(1), RdbcObject::Integer(10))
        .unwrap();
    store
        .stage_update(1, column(1), RdbcObject::Integer(11))
        .unwrap();
    store
        .stage_update(2, column(2), RdbcObject::String("other staged".into()))
        .unwrap();
    assert_eq!(store.value(1, column(1)).unwrap(), RdbcObject::Integer(11));
    assert_eq!(store.original_rows(), baseline);
    sql_error(store.changes());
    store.cancel_row_updates(1).unwrap();
    assert_eq!(store.value(1, column(1)).unwrap(), RdbcObject::Integer(1));
    assert_eq!(
        store.value(2, column(2)).unwrap(),
        RdbcObject::String("other staged".into())
    );
    store.update_row(2).unwrap();
    store.update_row(2).unwrap();
    let changes = store.changes().unwrap();
    assert_eq!(changes.len(), 1);
    let RowSetRowChange::Updated {
        row_index,
        original,
        values,
    } = &changes[0]
    else {
        panic!("updated row required")
    };
    assert_eq!(*row_index, 2);
    assert_eq!(original, &baseline[1]);
    assert_eq!(values, &row(2, "other staged"));
    assert_eq!(store.original_rows(), baseline);
}

#[test]
fn inserted_rows_have_no_original_and_insert_then_delete_does_not_request_database_write() {
    let mut store = RowSetRowStore::new(2);
    store.replace_rows(vec![row(1, "baseline")]).unwrap();
    assert_eq!(store.insert_row(row(2, "inserted")).unwrap(), 2);
    assert!(store.original_row(Some(2)).unwrap().is_empty());
    assert_eq!(store.original_rows(), vec![row(1, "baseline")]);
    store
        .stage_update(2, column(2), RdbcObject::String("new value".into()))
        .unwrap();
    store.update_row(2).unwrap();
    let changes = store.changes().unwrap();
    let RowSetRowChange::Inserted { row_index, values } = &changes[0] else {
        panic!("insert required")
    };
    assert_eq!(*row_index, 2);
    assert_eq!(values, &row(2, "new value"));
    store.delete_row(2).unwrap();
    assert!(store.changes().unwrap().is_empty());
    store.accept_changes().unwrap();
    assert_eq!(store.len(), 1);
    assert_eq!(store.original_rows(), vec![row(1, "baseline")]);
}

#[test]
fn deletion_retains_original_for_conflict_detection_and_restore_resurrects_original_rows() {
    let mut store = RowSetRowStore::new(2);
    let baseline = vec![row(1, "first"), row(2, "second")];
    store.replace_rows(baseline.clone()).unwrap();
    store
        .stage_update(1, column(2), RdbcObject::String("discard staged".into()))
        .unwrap();
    store.delete_row(1).unwrap();
    assert!(store.is_deleted(1).unwrap());
    assert_eq!(
        store.original_row(Some(1)).unwrap(),
        vec![baseline[0].clone()]
    );
    assert_eq!(store.original_rows(), baseline);
    sql_error(store.value(1, column(1)));
    sql_error(store.stage_update(1, column(1), RdbcObject::Integer(9)));
    sql_error(store.delete_row(1));
    let changes = store.changes().unwrap();
    let RowSetRowChange::Deleted {
        row_index,
        original,
    } = &changes[0]
    else {
        panic!("delete required")
    };
    assert_eq!(*row_index, 1);
    assert_eq!(original, &baseline[0]);
    store.insert_row(row(3, "discard insert")).unwrap();
    store.restore_originals();
    assert_eq!(store.len(), 2);
    assert!(!store.is_deleted(1).unwrap());
    assert_eq!(
        store.value(1, column(2)).unwrap(),
        RdbcObject::String("first".into())
    );
    assert!(store.changes().unwrap().is_empty());
    assert_eq!(store.original_rows(), baseline);
}

#[test]
fn only_explicit_success_accepts_new_baseline_and_pending_updates_cannot_be_lost() {
    let mut store = RowSetRowStore::new(2);
    store
        .replace_rows(vec![row(1, "first"), row(2, "delete")])
        .unwrap();
    store
        .stage_update(1, column(1), RdbcObject::Integer(7))
        .unwrap();
    sql_error(store.accept_changes());
    assert_eq!(
        store.original_rows(),
        vec![row(1, "first"), row(2, "delete")]
    );
    assert_eq!(store.value(1, column(1)).unwrap(), RdbcObject::Integer(7));
    store.update_row(1).unwrap();
    store.delete_row(2).unwrap();
    store.insert_row(row(3, "insert")).unwrap();
    let pending = store.changes().unwrap();
    assert_eq!(pending.len(), 3);
    // Writer 返回 false 或 Err 时不调用 accept；重复快照保留所有原始值和变化。
    assert_eq!(store.changes().unwrap(), pending);
    assert_eq!(
        store.original_rows(),
        vec![row(1, "first"), row(2, "delete")]
    );
    store.accept_changes().unwrap();
    assert!(store.changes().unwrap().is_empty());
    assert_eq!(store.len(), 2);
    assert_eq!(
        store.original_rows(),
        vec![row(7, "first"), row(3, "insert")]
    );
    assert_eq!(store.original_row(Some(2)).unwrap(), vec![row(3, "insert")]);
    assert!(!store.is_empty());
}
