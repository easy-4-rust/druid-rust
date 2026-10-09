//! 完整原警告及缓冲元数据控制；不替代正式Cargo目标验收。
use druid_rdbc::error::{RdbcError, SqlWarning};
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::metadata::{ColumnNullability, ResultSetColumnType};
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::types::{Row, Value};
/// 保留警告四字段与两列元数据的类型、可空性及越界错误。
/// 对应 Java: java.sql.SQLWarning 与 java.sql.ResultSetMetaData。
#[test]
fn task6_sql_warning_and_result_set_metadata_preserve_structured_fields() {
    let mut warning = SqlWarning::new("first", Some("01000".to_string()), 7);
    warning.set_next_warning(SqlWarning::new("second", None, 8));
    assert_eq!(warning.message(), "first");
    assert_eq!(warning.sql_state(), Some("01000"));
    assert_eq!(warning.error_code(), 7);
    assert_eq!(warning.next_warning().unwrap().message(), "second");

    let result_set = BufferedRowSet::with_column_labels(
        vec![Row::new(vec![Value::Bool(true), Value::Null])],
        vec!["enabled".to_string(), "optional".to_string()],
    );
    let meta_data = result_set.meta_data().unwrap();
    assert_eq!(meta_data.column_label(ColumnIndex::new(1).unwrap()).unwrap(), "enabled");
    assert_eq!(
        meta_data.column_type(ColumnIndex::new(1).unwrap()).unwrap(),
        ResultSetColumnType::Boolean
    );
    assert_eq!(
        meta_data.column_type(ColumnIndex::new(2).unwrap()).unwrap(),
        ResultSetColumnType::Unknown
    );
    assert!(!meta_data.nullability(ColumnIndex::new(1).unwrap()).map(|value| matches!(value, ColumnNullability::Nullable)).unwrap());
    assert!(meta_data.nullability(ColumnIndex::new(2).unwrap()).map(|value| matches!(value, ColumnNullability::Nullable)).unwrap());
    assert!(matches!(
        (|| -> Result<String, RdbcError> { meta_data.column_label(ColumnIndex::new(0)?) })(),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(matches!(
        meta_data.column_type(ColumnIndex::new(3).unwrap()),
        Err(RdbcError::InvalidArgument(_))
    ));
}
