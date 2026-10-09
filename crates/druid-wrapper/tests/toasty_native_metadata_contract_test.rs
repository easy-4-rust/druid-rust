//! 同一真实Toasty SQLite查询的原生元数据交接控制。
#![cfg(feature = "sqlite")]
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::{GeneratedKeys, ParameterIndex, PreparedParameter, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::toasty::ToastyConnectionFactory;

#[tokio::test]
async fn native_labels_preserve_duplicate_order_case_and_empty_results() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let one = ColumnIndex::new(1).unwrap();
    let two = ColumnIndex::new(2).unwrap();
    let mut rows = statement
        .execute_query("SELECT 1 AS \"DuP\", 2 AS \"dup\" WHERE 0")
        .await
        .unwrap();
    assert!(!rows.next().await.unwrap());
    assert_eq!(rows.find_column("DUP"), Ok(one));
    let metadata = rows.meta_data().unwrap();
    assert_eq!(metadata.column_count(), 2);
    assert_eq!(metadata.column_label(one).unwrap(), "DuP");
    assert_eq!(metadata.column_label(two).unwrap(), "dup");
    rows.close().await.unwrap();
    let mut rows = statement
        .execute_query("SELECT 1 AS \"DuP\", 2 AS \"dup\"")
        .await
        .unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(rows.int_by_label("dup"), Ok(1));
    assert_eq!(rows.int(two), Ok(2));
    assert_eq!(
        rows.find_column("column_1"),
        Err(RdbcError::InvalidArgument(
            "unknown column: column_1".to_owned()
        ))
    );
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn native_wildcard_and_subquery_labels_are_retained_without_rows() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement
        .execute_update(
            "CREATE TABLE native_columns (id INTEGER, payload BLOB)",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    let mut rows = statement
        .execute_query("SELECT * FROM (SELECT * FROM native_columns) WHERE 0")
        .await
        .unwrap();
    assert!(!rows.next().await.unwrap());
    assert_eq!(rows.find_column("id"), Ok(ColumnIndex::new(1).unwrap()));
    assert_eq!(
        rows.find_column("payload"),
        Ok(ColumnIndex::new(2).unwrap())
    );
    let metadata = rows.meta_data().unwrap();
    assert_eq!(metadata.column_count(), 2);
    assert_eq!(
        metadata.column_label(ColumnIndex::new(1).unwrap()).unwrap(),
        "id"
    );
    assert_eq!(
        metadata.column_label(ColumnIndex::new(2).unwrap()).unwrap(),
        "payload"
    );
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn native_prepared_returning_keeps_labels_and_hides_internal_key_column() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let mut setup = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    setup
        .execute_update(
            "CREATE TABLE native_returning (id INTEGER PRIMARY KEY, note TEXT)",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    let mut prepared = connection.prepare_statement(
        "INSERT INTO native_returning(id, note) VALUES (?, ?) RETURNING id AS ReturnedId, note AS ReturnedNote",
        StatementOptions::default(),
    ).await.unwrap();
    prepared
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::Int(7),
        })
        .unwrap();
    prepared
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(2).unwrap(),
            value: Value::String("原始参数".to_owned()),
        })
        .unwrap();
    let mut rows = prepared.execute_prepared_query().await.unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(rows.int_by_label("ReturnedId"), Ok(7));
    assert_eq!(
        rows.string_by_label("ReturnedNote"),
        Ok(Some("原始参数".to_owned()))
    );
    let metadata = rows.meta_data().unwrap();
    assert_eq!(metadata.column_count(), 2);
    assert_eq!(
        metadata.column_label(ColumnIndex::new(1).unwrap()).unwrap(),
        "ReturnedId"
    );
    assert_eq!(
        metadata.column_label(ColumnIndex::new(2).unwrap()).unwrap(),
        "ReturnedNote"
    );
    assert!(!rows.next().await.unwrap());
    rows.close().await.unwrap();
    prepared.close().await.unwrap();
    let mut count = setup
        .execute_query("SELECT count(*) AS actual_count FROM native_returning")
        .await
        .unwrap();
    assert!(count.next().await.unwrap());
    assert_eq!(count.int(ColumnIndex::new(1).unwrap()), Ok(1));
    count.close().await.unwrap();
    setup.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn native_query_metadata_is_isolated_between_real_connections() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut left = factory.create().await.unwrap();
    let mut right = factory.create().await.unwrap();
    let mut left_statement = left
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut right_statement = right
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let (left_rows, right_rows) = tokio::join!(
        left_statement.execute_query("SELECT 11 AS LeftOnly"),
        right_statement.execute_query("SELECT 22 AS RightOnly"),
    );
    let mut left_rows = left_rows.unwrap();
    let mut right_rows = right_rows.unwrap();
    assert!(left_rows.next().await.unwrap());
    assert!(right_rows.next().await.unwrap());
    assert_eq!(left_rows.int_by_label("leftonly"), Ok(11));
    assert_eq!(right_rows.int_by_label("rightonly"), Ok(22));
    assert_eq!(
        left_rows.find_column("RightOnly"),
        Err(RdbcError::InvalidArgument(
            "unknown column: RightOnly".to_owned()
        ))
    );
    assert_eq!(
        right_rows.find_column("LeftOnly"),
        Err(RdbcError::InvalidArgument(
            "unknown column: LeftOnly".to_owned()
        ))
    );
    left_rows.close().await.unwrap();
    right_rows.close().await.unwrap();
    left_statement.close().await.unwrap();
    right_statement.close().await.unwrap();
    left.close().await.unwrap();
    right.close().await.unwrap();
}

/// 已配置的原生驱动仍执行真实 SQL；失败查询之后保留新查询的独立列信息。
#[tokio::test]
async fn configured_native_driver_preserves_declared_types_after_query_failure() {
    let factory =
        ToastyConnectionFactory::from_driver(toasty_driver_sqlite::Sqlite::in_memory()).unwrap();
    assert_eq!(factory.max_connections(), Some(1));
    let mut connection = factory.create().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement
        .execute_update(
            "CREATE TABLE native_types (id INTEGER, payload BLOB)",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    let mut rows = statement
        .execute_query("SELECT * FROM native_types WHERE 0")
        .await
        .unwrap();
    assert!(!rows.next().await.unwrap());
    let metadata = rows.meta_data().unwrap();
    assert_eq!(
        metadata
            .column_type_name(ColumnIndex::new(1).unwrap())
            .unwrap(),
        "INTEGER"
    );
    assert_eq!(
        metadata
            .column_type_name(ColumnIndex::new(2).unwrap())
            .unwrap(),
        "BLOB"
    );
    rows.close().await.unwrap();
    assert!(statement
        .execute_query("SELECT * FROM native_missing_table")
        .await
        .is_err());
    let mut rows = statement
        .execute_query("SELECT 3 AS FreshLabel WHERE 0")
        .await
        .unwrap();
    assert!(!rows.next().await.unwrap());
    assert_eq!(
        rows.find_column("freshlabel"),
        Ok(ColumnIndex::new(1).unwrap())
    );
    assert_eq!(rows.meta_data().unwrap().column_count(), 1);
    assert_eq!(
        rows.find_column("payload"),
        Err(RdbcError::InvalidArgument(
            "unknown column: payload".to_owned()
        ))
    );
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
/// 用真实 SQLite 证明 EOF 不能重新开始，且关闭后导航保留原错误。
/// 对应 Java: ResultSet#next。
#[tokio::test]
async fn native_buffered_next_stays_exhausted_until_explicit_navigation() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rows = statement
        .execute_query("SELECT 1 AS id UNION ALL SELECT 2")
        .await
        .unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(rows.int(ColumnIndex::new(1).unwrap()), Ok(1));
    assert!(rows.next().await.unwrap());
    assert_eq!(rows.int(ColumnIndex::new(1).unwrap()), Ok(2));
    assert!(!rows.next().await.unwrap());
    assert!(
        !rows.next().await.unwrap(),
        "EOF 后 next 不得重新跳到第一行"
    );
    assert!(!rows.next().await.unwrap());
    rows.close().await.unwrap();
    assert_eq!(rows.next().await, Err(RdbcError::ConnectionDiscarded));
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 用真实结果验证双向、绝对、相对定位与空结果边界，不重新执行查询。
/// 对应 Java: ResultSet#previous/absolute/relative/beforeFirst/afterLast。
#[tokio::test]
async fn native_buffered_navigation_preserves_positions_and_empty_boundaries() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rows = statement
        .execute_query("SELECT 1 AS id UNION ALL SELECT 2")
        .await
        .unwrap();
    assert_eq!(rows.row(), Ok(0));
    assert_eq!(rows.is_before_first(), Ok(true));
    assert_eq!(rows.is_after_last(), Ok(false));
    assert!(!rows.previous().await.unwrap());
    assert!(rows.last().await.unwrap());
    assert_eq!(rows.row(), Ok(2));
    assert_eq!(rows.is_last(), Ok(true));
    assert!(rows.previous().await.unwrap());
    assert_eq!(rows.row(), Ok(1));
    assert_eq!(rows.is_first(), Ok(true));
    assert!(rows.relative(1).await.unwrap());
    assert_eq!(rows.row(), Ok(2));
    assert!(!rows.relative(i32::MAX).await.unwrap());
    assert_eq!(rows.is_after_last(), Ok(true));
    assert_eq!(rows.row(), Ok(0));
    assert!(!rows.next().await.unwrap());
    assert!(rows.previous().await.unwrap());
    assert_eq!(rows.row(), Ok(2));
    assert!(rows.absolute(-2).await.unwrap());
    assert_eq!(rows.row(), Ok(1));
    assert!(!rows.absolute(i32::MIN).await.unwrap());
    assert_eq!(rows.is_before_first(), Ok(true));
    assert!(rows.first().await.unwrap());
    assert!(rows.absolute(-1).await.unwrap());
    assert_eq!(rows.row(), Ok(2));
    rows.before_first().await.unwrap();
    assert_eq!(rows.row(), Ok(0));
    assert!(rows.next().await.unwrap());
    rows.after_last().await.unwrap();
    assert!(rows.previous().await.unwrap());
    assert_eq!(rows.row(), Ok(2));
    assert!(!rows.absolute(0).await.unwrap());
    assert_eq!(rows.is_before_first(), Ok(true));
    rows.close().await.unwrap();
    assert_eq!(rows.previous().await, Err(RdbcError::ConnectionDiscarded));
    assert_eq!(rows.row(), Err(RdbcError::ConnectionDiscarded));
    let mut empty = statement
        .execute_query("SELECT 1 AS id WHERE 0")
        .await
        .unwrap();
    assert_eq!(empty.is_before_first(), Ok(false));
    assert_eq!(empty.is_after_last(), Ok(false));
    assert_eq!(empty.is_first(), Ok(false));
    assert_eq!(empty.is_last(), Ok(false));
    assert_eq!(empty.row(), Ok(0));
    assert!(!empty.next().await.unwrap());
    assert!(!empty.next().await.unwrap());
    assert!(!empty.previous().await.unwrap());
    assert!(!empty.first().await.unwrap());
    assert!(!empty.last().await.unwrap());
    empty.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 只以实际 SQL NULL 证明可空；非空样本和空结果仍保留未知。
/// 对应 Java: ResultSetMetaData#isNullable。
#[tokio::test]
async fn native_buffered_nullability_requires_observed_null_evidence() {
    use druid_rdbc::metadata::ColumnNullability;
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let one = ColumnIndex::new(1).unwrap();
    let two = ColumnIndex::new(2).unwrap();
    let mut rows = statement
        .execute_query("SELECT 'first' AS optional_label, 1 AS id UNION ALL SELECT NULL, 2")
        .await
        .unwrap();
    let metadata = rows.meta_data().unwrap();
    assert_eq!(metadata.nullability(one), Ok(ColumnNullability::Nullable));
    assert_eq!(metadata.nullability(two), Ok(ColumnNullability::Unknown));
    assert_eq!(metadata.column_label(one).unwrap(), "optional_label");
    rows.close().await.unwrap();
    let mut empty = statement
        .execute_query("SELECT NULL AS optional_label WHERE 0")
        .await
        .unwrap();
    assert_eq!(
        empty.meta_data().unwrap().nullability(one),
        Ok(ColumnNullability::Unknown)
    );
    empty.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 原生列名资源读取保持NULL、定位、未知列与关闭错误的优先级。
/// 对应 Java: ResultSet#getRef/getBlob/getClob/getArray/getURL/getRowId/getNClob/getSQLXML。
#[tokio::test]
async fn native_resource_labels_preserve_null_and_error_priority() {
    use druid_rdbc::result_set::ResultSetRead;
    use druid_rdbc::types::RdbcObject;
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rows = statement
        .execute_query("SELECT 1 AS id, NULL AS empty_resource")
        .await
        .unwrap();
    let requests = [
        (ResultSetRead::Ref, "result_set_ref_by_label"),
        (ResultSetRead::Blob, "result_set_blob_by_label"),
        (ResultSetRead::Clob, "result_set_clob_by_label"),
        (ResultSetRead::Array, "result_set_array_by_label"),
        (ResultSetRead::Url, "result_set_url_by_label"),
        (ResultSetRead::RowId, "result_set_row_id_by_label"),
        (ResultSetRead::NClob, "result_set_n_clob_by_label"),
        (ResultSetRead::SqlXml, "result_set_sql_xml_by_label"),
    ];
    for (request, _) in &requests {
        assert_eq!(
            rows.read_object_by_label("id", request),
            Err(RdbcError::Other(
                "result set cursor is not positioned on a row".to_owned()
            ))
        );
    }
    assert!(rows.next().await.unwrap());
    for (request, operation) in &requests {
        assert_eq!(
            rows.read_object_by_label("id", request),
            Err(RdbcError::UnsupportedOperation { operation })
        );
        assert_eq!(
            rows.read_object_by_label("empty_resource", request),
            Ok(RdbcObject::Scalar(Value::Null))
        );
        assert_eq!(rows.was_null(), Ok(true));
        assert_eq!(
            rows.read_object_by_label("missing", request),
            Err(RdbcError::InvalidArgument(
                "unknown column: missing".to_owned()
            ))
        );
    }
    rows.close().await.unwrap();
    for (request, _) in &requests {
        assert_eq!(
            rows.read_object_by_label("missing", request),
            Err(RdbcError::ConnectionDiscarded)
        );
    }
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
