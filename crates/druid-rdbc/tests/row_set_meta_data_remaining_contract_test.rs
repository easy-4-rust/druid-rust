//! `javax.sql.RowSetMetaData` 剩余未评估常量与继承 getter 的精确合同。
//!
//! 对应 Java：`javax.sql.RowSetMetaData#columnNoNulls`、
//! `javax.sql.RowSetMetaData#columnNullable`、
//! `javax.sql.RowSetMetaData#columnNullableUnknown`，
//! 以及继承自 `java.sql.ResultSetMetaData` 的 `getColumnClassName(int)`、
//! `isDefinitelyWritable(int)`、`isReadOnly(int)`、`isWritable(int)`。
//!
//! 常量取值必须与 Java 8 精确一致；继承 getter 在行集元数据缓冲上必须返回
//! 同一列快照的实际值；同时确认标准 trait 默认入口在没有列来源时的精确
//! operation 名称，不允许臆造 UnsupportedOperation 或伪造列属性。

use druid_rdbc::error::{RdbcError, RdbcErrorKind};
use druid_rdbc::extensions::{RowSetMetaData, RowSetMetaDataBuffer};
use druid_rdbc::metadata::{ColumnNullability, DatabaseMetaDataConstants, ResultSetMetaData};
use druid_rdbc::result_set::ColumnIndex;

/// 标准 trait 默认入口在没有列来源时使用的精确 operation 名称。
const RESULT_SET_META_DATA_COLUMN_OPERATION: &str = "result_set_meta_data_column";

/// 构造 1-based 列索引；0 非法，因此这里只需要覆盖有效列。
fn col(index: usize) -> ColumnIndex {
    ColumnIndex::new(index).unwrap()
}

/// 只实现 column_count 的最小元数据，用于确认 trait 默认入口的 UnsupportedOperation 语义。
#[derive(Debug)]
struct ColumnSourceLessMetaData;

druid_rdbc::impl_standard_wrapper!(ColumnSourceLessMetaData => ResultSetMetaData);

impl ResultSetMetaData for ColumnSourceLessMetaData {
    fn column_count(&self) -> usize {
        0
    }
}

#[test]
fn column_no_nulls_constant_matches_the_jdk8_value_zero() {
    assert_eq!(DatabaseMetaDataConstants::COLUMN_NO_NULLS, 0);
    assert_eq!(ColumnNullability::NoNulls.rdbc_code(), 0);
    assert_eq!(
        ColumnNullability::NoNulls.rdbc_code(),
        DatabaseMetaDataConstants::COLUMN_NO_NULLS
    );
    let mut metadata = RowSetMetaDataBuffer::new();
    metadata.set_column_count(1).unwrap();
    metadata
        .set_nullable(col(1), DatabaseMetaDataConstants::COLUMN_NO_NULLS)
        .unwrap();
    assert_eq!(
        metadata.nullability(col(1)).unwrap(),
        ColumnNullability::NoNulls
    );
}

#[test]
fn column_nullable_constant_matches_the_jdk8_value_one() {
    assert_eq!(DatabaseMetaDataConstants::COLUMN_NULLABLE, 1);
    assert_eq!(ColumnNullability::Nullable.rdbc_code(), 1);
    assert_eq!(
        ColumnNullability::Nullable.rdbc_code(),
        DatabaseMetaDataConstants::COLUMN_NULLABLE
    );
    let mut metadata = RowSetMetaDataBuffer::new();
    metadata.set_column_count(1).unwrap();
    metadata
        .set_nullable(col(1), DatabaseMetaDataConstants::COLUMN_NULLABLE)
        .unwrap();
    assert_eq!(
        metadata.nullability(col(1)).unwrap(),
        ColumnNullability::Nullable
    );
}

#[test]
fn column_nullable_unknown_constant_matches_the_jdk8_value_two() {
    assert_eq!(DatabaseMetaDataConstants::COLUMN_NULLABLE_UNKNOWN, 2);
    assert_eq!(ColumnNullability::Unknown.rdbc_code(), 2);
    assert_eq!(
        ColumnNullability::Unknown.rdbc_code(),
        DatabaseMetaDataConstants::COLUMN_NULLABLE_UNKNOWN
    );
    let mut metadata = RowSetMetaDataBuffer::new();
    metadata.set_column_count(1).unwrap();
    metadata
        .set_nullable(col(1), DatabaseMetaDataConstants::COLUMN_NULLABLE_UNKNOWN)
        .unwrap();
    assert_eq!(
        metadata.nullability(col(1)).unwrap(),
        ColumnNullability::Unknown
    );
}

#[test]
fn get_column_class_name_returns_the_buffer_column_snapshot_class_name() {
    let mut metadata = RowSetMetaDataBuffer::new();
    metadata.set_column_count(1).unwrap();
    let view: &dyn ResultSetMetaData = &metadata;
    assert_eq!(view.column_class_name(col(1)).unwrap(), "java.lang.Object");
    assert_eq!(
        view.column_class_name(col(2)).unwrap_err().kind(),
        RdbcErrorKind::SqlException
    );
}

#[test]
fn is_definitely_writable_returns_the_buffer_column_snapshot_flag() {
    let mut metadata = RowSetMetaDataBuffer::new();
    metadata.set_column_count(1).unwrap();
    let view: &dyn ResultSetMetaData = &metadata;
    assert!(view.is_definitely_writable(col(1)).unwrap());
    assert_eq!(
        view.is_definitely_writable(col(2)).unwrap_err().kind(),
        RdbcErrorKind::SqlException
    );
}

#[test]
fn is_read_only_returns_the_buffer_column_snapshot_flag() {
    let mut metadata = RowSetMetaDataBuffer::new();
    metadata.set_column_count(1).unwrap();
    let view: &dyn ResultSetMetaData = &metadata;
    assert!(!view.is_read_only(col(1)).unwrap());
    assert_eq!(
        view.is_read_only(col(2)).unwrap_err().kind(),
        RdbcErrorKind::SqlException
    );
}

#[test]
fn is_writable_returns_the_buffer_column_snapshot_flag() {
    let mut metadata = RowSetMetaDataBuffer::new();
    metadata.set_column_count(1).unwrap();
    let view: &dyn ResultSetMetaData = &metadata;
    assert!(view.is_writable(col(1)).unwrap());
    assert_eq!(
        view.is_writable(col(2)).unwrap_err().kind(),
        RdbcErrorKind::SqlException
    );
}

#[test]
fn inherited_getters_without_a_column_source_report_the_exact_default_operation() {
    let metadata = ColumnSourceLessMetaData;
    let view: &dyn ResultSetMetaData = &metadata;
    let errors = [
        view.column_class_name(col(1)).unwrap_err(),
        view.is_definitely_writable(col(1)).unwrap_err(),
        view.is_read_only(col(1)).unwrap_err(),
        view.is_writable(col(1)).unwrap_err(),
    ];
    for error in errors {
        assert_eq!(error.kind(), RdbcErrorKind::UnsupportedOperation);
        assert!(matches!(
            error,
            RdbcError::UnsupportedOperation { operation }
                if operation == RESULT_SET_META_DATA_COLUMN_OPERATION
        ));
    }
}
