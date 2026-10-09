//! 完整原元数据合同；缺失接口的编译证据不替代运行时证据。
use druid_rdbc::metadata::{
    ResultSetColumnMeta, ResultSetColumnType, ResultSetMetaData, ResultSetMetaDataSnapshot,
    ResultSetNullability,
};
use druid_rdbc::result_set::ColumnIndex;
/// 验证原24字段、10类类型身份及三态可空性。
/// 对应 Java: java.sql.ResultSetMetaData 与 java.sql.Types。
#[test]
fn task6_result_set_metadata_preserves_complete_rdbc_column_contract() {
    let column = ResultSetColumnMeta::new("amount_alias", "MONEY", ResultSetNullability::Unknown)
        .with_driver_type("amount", ResultSetColumnType::Decimal)
        .with_origin("main", "app", "invoice")
        .with_class_name("com.example.Money")
        .with_dimensions(19, 4, 24)
        .with_flags(true, false, true, true, true)
        .with_writability(false, true, true);
    let meta_data = ResultSetMetaDataSnapshot::new(vec![column]);

    assert_eq!(meta_data.column_count(), 1);
    assert_eq!(
        meta_data
            .column_label(ColumnIndex::new(1).unwrap())
            .unwrap(),
        "amount_alias"
    );
    assert_eq!(
        meta_data.column_name(ColumnIndex::new(1).unwrap()).unwrap(),
        "amount"
    );
    assert_eq!(
        meta_data.schema_name(ColumnIndex::new(1).unwrap()).unwrap(),
        "app"
    );
    assert_eq!(
        meta_data.table_name(ColumnIndex::new(1).unwrap()).unwrap(),
        "invoice"
    );
    assert_eq!(
        meta_data
            .catalog_name(ColumnIndex::new(1).unwrap())
            .unwrap(),
        "main"
    );
    assert_eq!(
        meta_data.column_type(ColumnIndex::new(1).unwrap()).unwrap(),
        ResultSetColumnType::Decimal
    );
    assert_eq!(
        meta_data
            .column_type(ColumnIndex::new(1).unwrap())
            .unwrap()
            .rdbc_type(),
        3
    );
    assert_eq!(
        meta_data
            .column_type_name(ColumnIndex::new(1).unwrap())
            .unwrap(),
        "MONEY"
    );
    assert_eq!(
        meta_data
            .column_class_name(ColumnIndex::new(1).unwrap())
            .unwrap(),
        "com.example.Money"
    );
    assert_eq!(
        meta_data.nullability(ColumnIndex::new(1).unwrap()).unwrap(),
        ResultSetNullability::Unknown
    );
    assert_eq!(
        meta_data
            .nullability(ColumnIndex::new(1).unwrap())
            .unwrap()
            .rdbc_code(),
        2
    );
    assert!(!matches!(
        meta_data.nullability(ColumnIndex::new(1).unwrap()).unwrap(),
        ResultSetNullability::Nullable
    ));
    assert!(meta_data
        .is_auto_increment(ColumnIndex::new(1).unwrap())
        .unwrap());
    assert!(!meta_data
        .is_case_sensitive(ColumnIndex::new(1).unwrap())
        .unwrap());
    assert!(meta_data
        .is_searchable(ColumnIndex::new(1).unwrap())
        .unwrap());
    assert!(meta_data.is_currency(ColumnIndex::new(1).unwrap()).unwrap());
    assert!(meta_data.is_signed(ColumnIndex::new(1).unwrap()).unwrap());
    assert_eq!(
        meta_data
            .column_display_size(ColumnIndex::new(1).unwrap())
            .unwrap(),
        24
    );
    assert_eq!(
        meta_data.precision(ColumnIndex::new(1).unwrap()).unwrap(),
        19
    );
    assert_eq!(meta_data.scale(ColumnIndex::new(1).unwrap()).unwrap(), 4);
    assert!(!meta_data
        .is_read_only(ColumnIndex::new(1).unwrap())
        .unwrap());
    assert!(meta_data.is_writable(ColumnIndex::new(1).unwrap()).unwrap());
    assert!(meta_data
        .is_definitely_writable(ColumnIndex::new(1).unwrap())
        .unwrap());

    let all_types = [
        (
            ResultSetColumnType::Unknown,
            1_111,
            "OTHER",
            "java.lang.Object",
            false,
        ),
        (
            ResultSetColumnType::Boolean,
            16,
            "BOOLEAN",
            "java.lang.Boolean",
            false,
        ),
        (
            ResultSetColumnType::Integer,
            -5,
            "BIGINT",
            "java.lang.Long",
            true,
        ),
        (
            ResultSetColumnType::Float,
            8,
            "DOUBLE",
            "java.lang.Double",
            true,
        ),
        (
            ResultSetColumnType::Decimal,
            3,
            "DECIMAL",
            "java.math.BigDecimal",
            true,
        ),
        (
            ResultSetColumnType::Date,
            91,
            "DATE",
            "java.sql.Date",
            false,
        ),
        (
            ResultSetColumnType::Time,
            92,
            "TIME",
            "java.sql.Time",
            false,
        ),
        (
            ResultSetColumnType::Timestamp,
            93,
            "TIMESTAMP",
            "java.sql.Timestamp",
            false,
        ),
        (
            ResultSetColumnType::Text,
            12,
            "VARCHAR",
            "java.lang.String",
            false,
        ),
        (ResultSetColumnType::Binary, -3, "VARBINARY", "[B", false),
    ];
    for (column_type, rdbc_type, type_name, class_name, signed) in all_types {
        assert_eq!(column_type.rdbc_type(), rdbc_type);
        assert_eq!(column_type.type_name(), type_name);
        assert_eq!(column_type.class_name(), class_name);
        assert_eq!(column_type.is_signed(), signed);
    }
    assert_eq!(ResultSetNullability::NoNulls.rdbc_code(), 0);
    assert_eq!(ResultSetNullability::Nullable.rdbc_code(), 1);
    assert_eq!(ResultSetNullability::Unknown.rdbc_code(), 2);
}

/// 验证扩展类型的纯查询不会覆盖真实列描述符；参数为空，返回由断言验证。
/// 对应 Java: java.sql.Types 与 ResultSetMetaData#getColumnTypeName/getColumnClassName/isSigned。
#[test]
fn task6_metadata_extended_type_defaults_preserve_driver_descriptor_identity() {
    let jdbc = ResultSetColumnType::Jdbc(-5);
    assert_eq!(jdbc.rdbc_type(), -5);
    assert_eq!(jdbc.type_name(), "OTHER");
    assert_eq!(jdbc.class_name(), "java.lang.Object");
    assert!(!jdbc.is_signed());
    let named = ResultSetColumnType::Named("Vendor.Money_类型".to_owned());
    assert_eq!(named.rdbc_type(), 1_111);
    assert_eq!(named.type_name(), "Vendor.Money_类型");
    assert_eq!(named.class_name(), "java.lang.Object");
    assert!(!named.is_signed());
    let column = ResultSetColumnMeta::new("price", "Vendor.MONEY", ResultSetNullability::Unknown)
        .with_driver_type("native_price", named)
        .with_class_name("com.example.NativeMoney")
        .with_flags(false, false, true, true, true);
    let metadata = ResultSetMetaDataSnapshot::new(vec![column]);
    let index = ColumnIndex::new(1).unwrap();
    assert_eq!(metadata.column_type_name(index).unwrap(), "Vendor.MONEY");
    assert_eq!(
        metadata.column_class_name(index).unwrap(),
        "com.example.NativeMoney"
    );
    assert!(metadata.is_signed(index).unwrap());
    let neutral = ResultSetMetaDataSnapshot::new(vec![ResultSetColumnMeta::new(
        "amount",
        "DECIMAL",
        ResultSetNullability::Unknown,
    )]);
    assert_eq!(neutral.column_class_name(index).unwrap(), "");
    assert!(!neutral.is_signed(index).unwrap());
}
