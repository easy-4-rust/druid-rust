//! `java.sql.DatabaseMetaData` 三个未评估方法的 trait 默认入口契约测试。
//!
//! 对应 Java：`java.sql.DatabaseMetaData#getConnection()`、
//! `java.sql.DatabaseMetaData#getMaxLogicalLobSize()`、
//! `java.sql.DatabaseMetaData#supportsRefCursors()`。
//!
//! Java 8 规定这三个方法在驱动不支持时抛出 `SQLFeatureNotSupportedException`，
//! 因此 Rust 标准层的默认入口必须返回同一语义的标准错误，并保留精确的 operation 名称；
//! 不允许伪造连接对象、LOB 上限或能力声明。

use druid_rdbc::error::{RdbcError, RdbcErrorKind, SqlExceptionKind};
use druid_rdbc::metadata::DatabaseMetaData;

/// `java.sql.DatabaseMetaData#getConnection()` 的默认 operation 名称。
const GET_CONNECTION_OPERATION: &str = "metadata_get_connection";

/// `java.sql.DatabaseMetaData#getMaxLogicalLobSize()` 的默认 operation 名称。
const GET_MAX_LOGICAL_LOB_SIZE_OPERATION: &str = "metadata_get_max_logical_lob_size";

/// `java.sql.DatabaseMetaData#supportsRefCursors()` 的默认 operation 名称。
const SUPPORTS_REF_CURSORS_OPERATION: &str = "metadata_supports_ref_cursors";

/// 只使用 trait 默认实现的 metadata，不覆写任何默认方法。
struct DefaultDatabaseMetaData;

druid_rdbc::impl_standard_wrapper!(DefaultDatabaseMetaData => DatabaseMetaData);

#[async_trait::async_trait]
impl DatabaseMetaData for DefaultDatabaseMetaData {}

/// 断言默认入口返回 `SQLFeatureNotSupportedException`，并保留精确的 operation 名称。
fn assert_feature_not_supported<T>(result: Result<T, RdbcError>, expected_operation: &str) {
    let error = match result {
        Ok(_) => panic!("default entry must report the unsupported feature"),
        Err(error) => error,
    };

    assert_eq!(
        error.kind(),
        RdbcErrorKind::SqlFeatureNotSupported,
        "default entry must map to the Java SQLFeatureNotSupportedException kind"
    );
    let exception = error
        .sql_exception()
        .expect("feature not supported must carry a sql exception");
    assert_eq!(exception.kind(), SqlExceptionKind::FeatureNotSupported);
    assert_eq!(
        exception.class_name(),
        "java.sql.SQLFeatureNotSupportedException"
    );
    assert_eq!(exception.sql_state(), Some("0A000"));

    // 精确 operation 名称必须完整保留在错误文本中，不允许被截断或替换。
    let expected_message = format!("RDBC operation is not supported: {expected_operation}");
    assert_eq!(
        exception.message(),
        Some(expected_message.as_str()),
        "default entry must keep the exact operation name"
    );
}

/// `getConnection()` 默认入口返回精确 operation 的不支持错误。
#[tokio::test]
async fn default_get_connection_reports_exact_unsupported_operation() {
    let mut metadata = DefaultDatabaseMetaData;
    assert_feature_not_supported(metadata.get_connection().await, GET_CONNECTION_OPERATION);
}

/// `getMaxLogicalLobSize()` 默认入口返回精确 operation 的不支持错误。
#[tokio::test]
async fn default_get_max_logical_lob_size_reports_exact_unsupported_operation() {
    let mut metadata = DefaultDatabaseMetaData;
    assert_feature_not_supported(
        metadata.get_max_logical_lob_size().await,
        GET_MAX_LOGICAL_LOB_SIZE_OPERATION,
    );
}

/// `supportsRefCursors()` 默认入口返回精确 operation 的不支持错误。
#[tokio::test]
async fn default_supports_ref_cursors_reports_exact_unsupported_operation() {
    let mut metadata = DefaultDatabaseMetaData;
    assert_feature_not_supported(
        metadata.supports_ref_cursors().await,
        SUPPORTS_REF_CURSORS_OPERATION,
    );
}

/// 三个默认入口在 canonical object-safe 入口 `Box<dyn DatabaseMetaData>` 上同样可达，
/// 并保留各自精确的 operation 名称。
#[tokio::test]
async fn default_entries_are_reachable_through_object_safe_metadata_entry() {
    let mut metadata: Box<dyn DatabaseMetaData> = Box::new(DefaultDatabaseMetaData);

    assert_feature_not_supported(metadata.get_connection().await, GET_CONNECTION_OPERATION);
    assert_feature_not_supported(
        metadata.get_max_logical_lob_size().await,
        GET_MAX_LOGICAL_LOB_SIZE_OPERATION,
    );
    assert_feature_not_supported(
        metadata.supports_ref_cursors().await,
        SUPPORTS_REF_CURSORS_OPERATION,
    );
}
