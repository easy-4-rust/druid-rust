//! javax.sql.RowSetMetaData完整setter面与同一metadata身份合同。
use druid_rdbc::error::RdbcErrorKind;
use druid_rdbc::extensions::{RowSetMetaData, RowSetMetaDataBuffer};
use druid_rdbc::metadata::{ColumnNullability, ResultSetColumnType, ResultSetMetaData};
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::wrapper::{StandardUnwrapped, StandardUnwrappedMut, Wrapper};
use std::any::TypeId;

fn col(index: usize) -> ColumnIndex {
    ColumnIndex::new(index).unwrap()
}

#[test]
fn all_seventeen_setters_round_trip_through_the_inherited_standard_interface() {
    let mut metadata = RowSetMetaDataBuffer::new();
    let view: &mut dyn RowSetMetaData = &mut metadata;
    view.set_column_count(2).unwrap();
    let first = col(1);
    view.set_auto_increment(first, true).unwrap();
    view.set_case_sensitive(first, true).unwrap();
    view.set_searchable(first, true).unwrap();
    view.set_currency(first, true).unwrap();
    view.set_nullable(first, 1).unwrap();
    view.set_signed(first, true).unwrap();
    view.set_column_display_size(first, 80).unwrap();
    view.set_column_label(first, Some("price-label".into()))
        .unwrap();
    view.set_column_name(first, Some("price".into())).unwrap();
    view.set_schema_name(first, Some("sales".into())).unwrap();
    view.set_precision(first, 18).unwrap();
    view.set_scale(first, 4).unwrap();
    view.set_table_name(first, Some("orders".into())).unwrap();
    view.set_catalog_name(first, Some("shop".into())).unwrap();
    view.set_column_type(first, 3).unwrap();
    view.set_column_type_name(first, Some("MONEY".into()))
        .unwrap();
    assert_eq!(view.column_count(), 2);
    assert!(view.is_auto_increment(first).unwrap());
    assert!(view.is_case_sensitive(first).unwrap());
    assert!(view.is_searchable(first).unwrap());
    assert!(view.is_currency(first).unwrap());
    assert!(view.is_signed(first).unwrap());
    assert_eq!(
        view.nullability(first).unwrap(),
        ColumnNullability::Nullable
    );
    assert_eq!(view.column_display_size(first).unwrap(), 80);
    assert_eq!(view.column_label(first).unwrap(), "price-label");
    assert_eq!(view.column_name(first).unwrap(), "price");
    assert_eq!(view.schema_name(first).unwrap(), "sales");
    assert_eq!(view.precision(first).unwrap(), 18);
    assert_eq!(view.scale(first).unwrap(), 4);
    assert_eq!(view.table_name(first).unwrap(), "orders");
    assert_eq!(view.catalog_name(first).unwrap(), "shop");
    assert_eq!(
        view.column_type(first).unwrap(),
        ResultSetColumnType::Jdbc(3)
    );
    assert_eq!(view.column_type_name(first).unwrap(), "MONEY");
    let second = col(2);
    assert!(!view.is_auto_increment(second).unwrap());
    assert!(!view.is_searchable(second).unwrap());
    assert_eq!(
        view.nullability(second).unwrap(),
        ColumnNullability::Unknown
    );
    assert_eq!(view.column_label(second).unwrap(), "");
}

#[test]
fn invalid_values_and_column_ranges_are_sql_errors_without_mutating_prior_values() {
    let mut metadata = RowSetMetaDataBuffer::new();
    metadata.set_column_count(1).unwrap();
    metadata.set_precision(col(1), 12).unwrap();
    metadata.set_scale(col(1), 2).unwrap();
    metadata.set_column_type(col(1), 4).unwrap();
    for invalid in [0, -1] {
        assert_eq!(
            metadata.set_column_count(invalid).unwrap_err().kind(),
            RdbcErrorKind::SqlException
        );
    }
    for error in [
        metadata.set_precision(col(1), -1),
        metadata.set_scale(col(1), -1),
        metadata.set_column_display_size(col(1), -1),
        metadata.set_nullable(col(1), 3),
        metadata.set_nullable(col(1), -1),
        metadata.set_column_type(col(1), 999_999),
        metadata.set_column_name(col(2), Some("out-of-range".into())),
    ] {
        assert_eq!(error.unwrap_err().kind(), RdbcErrorKind::SqlException);
    }
    assert_eq!(metadata.column_count(), 1);
    assert_eq!(metadata.precision(col(1)).unwrap(), 12);
    assert_eq!(metadata.scale(col(1)).unwrap(), 2);
    assert_eq!(
        metadata.column_type(col(1)).unwrap(),
        ResultSetColumnType::Jdbc(4)
    );
    assert!(ColumnIndex::new(0).is_err());
    assert_eq!(
        metadata.column(col(2)).unwrap_err().kind(),
        RdbcErrorKind::SqlException
    );
}

#[test]
fn nullable_names_reset_to_empty_and_snapshots_do_not_alias_later_updates() {
    let mut metadata = RowSetMetaDataBuffer::new();
    metadata.set_column_count(1).unwrap();
    metadata
        .set_column_label(col(1), Some("before".into()))
        .unwrap();
    let before = metadata.column(col(1)).unwrap();
    metadata.set_column_label(col(1), None).unwrap();
    metadata.set_column_name(col(1), None).unwrap();
    metadata.set_schema_name(col(1), None).unwrap();
    metadata.set_table_name(col(1), None).unwrap();
    metadata.set_catalog_name(col(1), None).unwrap();
    metadata.set_column_type_name(col(1), None).unwrap();
    assert_eq!(before.label(), "before");
    assert_eq!(metadata.column_label(col(1)).unwrap(), "");
    assert_eq!(metadata.column_name(col(1)).unwrap(), "");
    assert_eq!(metadata.schema_name(col(1)).unwrap(), "");
    assert_eq!(metadata.table_name(col(1)).unwrap(), "");
    assert_eq!(metadata.catalog_name(col(1)).unwrap(), "");
    assert_eq!(metadata.column_type_name(col(1)).unwrap(), "");
}

#[test]
fn wrapper_leaf_parent_and_mutable_views_refer_to_the_same_metadata() {
    let mut metadata = RowSetMetaDataBuffer::new();
    metadata.set_column_count(1).unwrap();
    assert!(metadata
        .is_wrapper_for(Some(TypeId::of::<dyn RowSetMetaData>()))
        .unwrap());
    assert!(metadata
        .is_wrapper_for(Some(TypeId::of::<dyn ResultSetMetaData>()))
        .unwrap());
    assert!(metadata
        .is_wrapper_for(Some(TypeId::of::<RowSetMetaDataBuffer>()))
        .unwrap());
    assert!(!metadata
        .is_wrapper_for(Some(TypeId::of::<String>()))
        .unwrap());
    {
        let StandardUnwrappedMut::RowSetMetaData(view) = metadata
            .unwrap_mut(Some(TypeId::of::<dyn RowSetMetaData>()))
            .unwrap()
        else {
            panic!("canonical leaf required")
        };
        view.set_column_name(col(1), Some("same-instance".into()))
            .unwrap();
    }
    let StandardUnwrapped::ResultSetMetaData(parent) = metadata
        .unwrap(Some(TypeId::of::<dyn ResultSetMetaData>()))
        .unwrap()
    else {
        panic!("canonical parent required")
    };
    assert_eq!(parent.column_name(col(1)).unwrap(), "same-instance");
    assert!(metadata.unwrap(None).is_err());
}

#[test]
fn count_changes_reset_metadata_and_large_declared_width_does_not_allocate_all_columns() {
    let mut metadata = RowSetMetaDataBuffer::new();
    metadata.set_column_count(2).unwrap();
    metadata
        .set_column_name(col(1), Some("old".into()))
        .unwrap();
    metadata.set_column_count(1).unwrap();
    assert_eq!(metadata.column_name(col(1)).unwrap(), "");
    assert!(metadata.column(col(2)).is_err());
    metadata.set_column_count(i32::MAX).unwrap();
    assert_eq!(metadata.column_count(), i32::MAX as usize);
    metadata
        .set_column_name(col(i32::MAX as usize), Some("last".into()))
        .unwrap();
    assert_eq!(
        metadata.column_name(col(i32::MAX as usize)).unwrap(),
        "last"
    );
    assert_eq!(metadata.column_name(col(1)).unwrap(), "");
}
