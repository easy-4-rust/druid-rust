//! 明确不支持接口能力的精确负向合同。
//!
//! 对应 Java 默认方法：`SQLInput#readObject(Class)`、`SQLOutput#writeObject(Object, SQLType)`。
//! RowSet 的二进制流更新由 Rust provider 明确拒绝，不能伪装成成功写入。

use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{BufferedSqlInput, BufferedSqlOutput, SharedRowSet};
use druid_rdbc::resource::{RdbcStreamLength, SqlInput, SqlOutput};
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::ResultSetConcurrency;
use druid_rdbc::types::{JdbcType, RdbcTargetType, SqlType};

/// 断言 feature-not-supported 的精确 SQLState、class 与 message 片段。
fn assert_feature_not_supported(error: RdbcError, operation: &str) {
    let RdbcError::SqlException(exception) = error else {
        panic!("expected SQLFeatureNotSupportedException, got {error:?}");
    };
    assert_eq!(exception.sql_state(), Some("0A000"));
    assert_eq!(
        exception.class_name(),
        "java.sql.SQLFeatureNotSupportedException"
    );
    assert!(
        exception
            .message()
            .is_some_and(|message| message.contains(operation)),
        "unexpected message: {:?}",
        exception.message()
    );
}

#[tokio::test]
async fn sql_input_read_object_as_reports_precise_unsupported_operation() {
    let mut input = BufferedSqlInput::new(Vec::new());
    let error = input
        .read_object_as(&RdbcTargetType::String)
        .await
        .expect_err("default SQLInput#readObject(Class) must be unsupported");
    assert_feature_not_supported(error, "SQLInput#readObject(Class)");
}

#[tokio::test]
async fn sql_output_write_object_as_reports_precise_unsupported_operation() {
    let mut output = BufferedSqlOutput::new();
    let sql_type: &dyn SqlType = &JdbcType::VarChar;
    let error = output
        .write_object_as(None, sql_type)
        .await
        .expect_err("default SQLOutput#writeObject(Object, SQLType) must be unsupported");
    assert_feature_not_supported(error, "SQLOutput#writeObject(Object, SQLType)");
}

#[tokio::test]
async fn row_set_blob_stream_update_reports_precise_unsupported_operation() {
    use druid_rdbc::extensions::RowSet;

    let mut row_set = SharedRowSet::new();
    row_set
        .set_read_only(false)
        .expect("writable RowSet property");
    row_set
        .set_concurrency(ResultSetConcurrency::Updatable.rdbc_code())
        .expect("updatable RowSet property");
    for length in [RdbcStreamLength::Unspecified, RdbcStreamLength::Long(1)] {
        let error = row_set
            .update_blob_stream(
                ColumnIndex::new(1).expect("valid column index"),
                None,
                length,
            )
            .expect_err("RowSet binary stream update must be unsupported");
        assert_feature_not_supported(error, "row_set_lob_stream_update");
    }
}
