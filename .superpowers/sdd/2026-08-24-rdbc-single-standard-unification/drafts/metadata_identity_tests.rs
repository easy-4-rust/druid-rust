//! 未接入构建的完整BASE恢复草稿，未运行，不计入完成。
use super::ResultSetMetaDataProxy;
use crate::core::ProxyAttributes;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ColumnNullability, ResultSetColumnType, ResultSetMetaData};
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use std::any::TypeId;
use std::sync::Arc;
#[path = "meta_data_identity_probe.rs"]
mod meta_data_identity_probe;
use meta_data_identity_probe::MetadataIdentityProbe;

#[test]
fn task6_metadata_preserves_getter_error_and_wrapper_identity() {
    let probe = Arc::new(MetadataIdentityProbe::new(None));
    let meta_data =
        ResultSetMetaDataProxy::new(Arc::clone(&probe), Arc::new(ProxyAttributes::default()));

    assert_eq!(meta_data.column_count(), 1);
    assert!(meta_data
        .is_auto_increment(ColumnIndex::new(1).unwrap())
        .unwrap());
    assert!(meta_data
        .is_case_sensitive(ColumnIndex::new(1).unwrap())
        .unwrap());
    assert!(meta_data
        .is_searchable(ColumnIndex::new(1).unwrap())
        .unwrap());
    assert!(meta_data.is_currency(ColumnIndex::new(1).unwrap()).unwrap());
    assert!(meta_data.is_nullable(ColumnIndex::new(1).unwrap()).unwrap());
    assert_eq!(
        meta_data.nullability(ColumnIndex::new(1).unwrap()).unwrap(),
        ColumnNullability::Nullable
    );
    assert_eq!(
        meta_data
            .nullable_code(ColumnIndex::new(1).unwrap())
            .unwrap(),
        1
    );
    assert!(meta_data.is_signed(ColumnIndex::new(1).unwrap()).unwrap());
    assert_eq!(
        meta_data
            .column_display_size(ColumnIndex::new(1).unwrap())
            .unwrap(),
        21
    );
    assert_eq!(
        meta_data
            .column_label(ColumnIndex::new(1).unwrap())
            .unwrap(),
        "label_1"
    );
    assert_eq!(
        meta_data.column_name(ColumnIndex::new(1).unwrap()).unwrap(),
        "name_1"
    );
    assert_eq!(
        meta_data.schema_name(ColumnIndex::new(1).unwrap()).unwrap(),
        "schema_1"
    );
    assert_eq!(
        meta_data.precision(ColumnIndex::new(1).unwrap()).unwrap(),
        31
    );
    assert_eq!(meta_data.scale(ColumnIndex::new(1).unwrap()).unwrap(), 5);
    assert_eq!(
        meta_data.table_name(ColumnIndex::new(1).unwrap()).unwrap(),
        "table_1"
    );
    assert_eq!(
        meta_data
            .catalog_name(ColumnIndex::new(1).unwrap())
            .unwrap(),
        "catalog_1"
    );
    assert_eq!(
        meta_data.column_type(ColumnIndex::new(1).unwrap()).unwrap(),
        ResultSetColumnType::Decimal
    );
    assert_eq!(
        meta_data.rdbc_type(ColumnIndex::new(1).unwrap()).unwrap(),
        3
    );
    assert_eq!(
        meta_data
            .column_type_name(ColumnIndex::new(1).unwrap())
            .unwrap(),
        "MONEY_1"
    );
    assert!(!meta_data
        .is_read_only(ColumnIndex::new(1).unwrap())
        .unwrap());
    assert!(meta_data.is_writable(ColumnIndex::new(1).unwrap()).unwrap());
    assert!(meta_data
        .is_definitely_writable(ColumnIndex::new(1).unwrap())
        .unwrap());
    assert_eq!(
        meta_data
            .column_class_name(ColumnIndex::new(1).unwrap())
            .unwrap(),
        "com.example.Money1"
    );
    assert_eq!(
        probe.calls(),
        vec![
            "column_count",
            "is_auto_increment",
            "is_case_sensitive",
            "is_searchable",
            "is_currency",
            "nullability",
            "nullability",
            "nullability",
            "is_signed",
            "column_display_size",
            "column_label",
            "column_name",
            "schema_name",
            "precision",
            "scale",
            "table_name",
            "catalog_name",
            "column_type",
            "column_type",
            "column_type_name",
            "is_read_only",
            "is_writable",
            "is_definitely_writable",
            "column_class_name",
        ]
    );

    assert!(matches!(
        Wrapper::unwrap(&meta_data, Some(TypeId::of::<dyn ResultSetMetaData>())),
        Ok(StandardUnwrapped::ResultSetMetaData(_))
    ));
    assert!(meta_data.is_wrapper_for_type::<ResultSetMetaDataProxy<MetadataIdentityProbe>>());
    assert!(meta_data.is_wrapper_for_type::<dyn ResultSetMetaData>());
    assert!(meta_data.is_wrapper_for_type::<MetadataIdentityProbe>());
    assert!(std::ptr::eq(
        meta_data
            .unwrap_ref::<MetadataIdentityProbe>()
            .expect("必须解包到底层具体 metadata"),
        probe.as_ref()
    ));
    assert!(std::ptr::eq(
        meta_data
            .unwrap(Some(TypeId::of::<dyn ResultSetMetaData>()))
            .and_then(|value| match value {
                StandardUnwrapped::ResultSetMetaData(raw) => Ok(raw),
                _ => Err(RdbcError::InvalidArgument(
                    "expected ResultSetMetaData view".to_owned()
                )),
            })
            .expect("必须解包到物理 metadata SPI"),
        probe.as_ref() as &dyn ResultSetMetaData
    ));
    assert_eq!(meta_data, meta_data.clone());
    assert!(format!("{meta_data:?}").contains("MetadataIdentityProbe"));

    let failing_probe = Arc::new(MetadataIdentityProbe::new(Some("column_label")));
    let failing = ResultSetMetaDataProxy::new(
        Arc::clone(&failing_probe),
        Arc::new(ProxyAttributes::default()),
    );
    assert!(matches!(
        failing.column_label(ColumnIndex::new(9).unwrap()),
        Err(RdbcError::DriverError(message))
            if message == "metadata probe failed at column_label"
    ));
    assert_eq!(failing_probe.calls(), vec!["column_label"]);
    assert_ne!(meta_data, failing);
    assert_ne!(meta_data, ResultSetMetaDataProxy::default());
}
