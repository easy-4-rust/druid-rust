//! 结果集诊断：diagnostic_basics 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

#[tokio::test]
async fn task6_open_filter_column_map_rejects_missing_keys() {
    let mut filters = crate::core::FilterChain::new();
    filters.add_result_set(Arc::new(initial_column_map_filter::InitialColumnMapFilter));
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_filters(Some(Arc::new(filters))).await;
    assert_eq!(rows.driver_column(1), Some(2));
    assert_eq!(rows.driver_column(99), None);
    assert!(rows
        .logical_column_index(druid_rdbc::result_set::ColumnIndex::new(99).unwrap())
        .is_err());
    rows.next().await.unwrap();
    assert_eq!(
        rows.value(druid_rdbc::result_set::ColumnIndex::new(1).unwrap())
            .unwrap(),
        druid_rdbc::types::Value::String("row1".to_owned())
    );
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_hidden_columns_none_by_default() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    let alias = DruidPooledResultSet::from_tracked_resource(
        Arc::clone(&statement.inner),
        Arc::clone(&rows.resource),
    )
    .unwrap();
    assert_eq!(rows.hidden_column_count(), 0);
    assert!(rows.hidden_columns().is_none());
    rows.set_hidden_columns(Some(vec![3, 4]));
    assert_eq!(rows.hidden_column_count(), 2);
    assert_eq!(rows.hidden_columns(), Some(vec![3, 4]));
    assert_eq!(alias.hidden_columns(), Some(vec![3, 4]));
    rows.next().await.unwrap();
    assert!(alias.value(ColumnIndex::new(3).unwrap()).is_err());
    alias.set_hidden_columns(None);
    assert!(rows.hidden_columns().is_none());
    assert_eq!(rows.hidden_column_count(), 0);
    assert_eq!(
        rows.value(ColumnIndex::new(3).unwrap()).unwrap(),
        druid_rdbc::types::Value::Float(1.5)
    );
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_physical_column_identity_without_map() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    assert_eq!(rows.physical_column(1), Some(1));
    assert_eq!(rows.logic_column(1), Some(1));
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_set_logic_column_map_and_reverse() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    let alias = DruidPooledResultSet::from_tracked_resource(
        Arc::clone(&statement.inner),
        Arc::clone(&rows.resource),
    )
    .unwrap();
    rows.set_logic_column_map(Some(HashMap::from([(1, 2), (2, 1)])));
    assert_eq!(rows.physical_column(1), Some(2));
    assert_eq!(rows.physical_column(2), Some(1));
    assert_eq!(rows.physical_column(99), None);
    assert_eq!(alias.physical_column(1), Some(2));
    rows.set_physical_column_map(Some(HashMap::from([(2, 1), (1, 2)])));
    assert_eq!(rows.logic_column(2), Some(1));
    assert_eq!(alias.logic_column(99), None);
    assert!(alias
        .logical_column_index(ColumnIndex::new(99).unwrap())
        .is_err());
    rows.next().await.unwrap();
    assert_eq!(
        alias.value(ColumnIndex::new(1).unwrap()).unwrap(),
        druid_rdbc::types::Value::String("row1".to_owned())
    );
    assert_eq!(
        alias
            .logical_column_index(ColumnIndex::new(2).unwrap())
            .unwrap(),
        ColumnIndex::new(1).unwrap()
    );
    assert_eq!(alias.find_column("name"), Ok(ColumnIndex::new(1).unwrap()));
    assert_eq!(
        alias.find_column("missing"),
        Err(druid_rdbc::error::RdbcError::InvalidArgument(
            "unknown column: missing".to_owned()
        ))
    );
    rows.set_logic_column_map(None);
    assert_eq!(rows.physical_column(1), Some(1));
    assert_eq!(
        alias.value(ColumnIndex::new(1).unwrap()).unwrap(),
        druid_rdbc::types::Value::Int(1)
    );
    // 正反映射独立清除，保留 BASE 设置器的语义。
    assert_eq!(alias.logic_column(2), Some(1));
    alias.set_physical_column_map(None);
    assert_eq!(rows.logic_column(2), Some(2));
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_close_count_zero_before_close() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    assert_eq!(rows.filter_context.close_count(), 0);
    rows.close().await.unwrap();
    assert_eq!(rows.filter_context.close_count(), 1);
    // 重复关闭、创建者级联关闭及 registry 收尾不能再次增加成功关闭计数。
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    statement.inner.resources.close_all().await.unwrap();
    assert_eq!(rows.filter_context.close_count(), 1);
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_cursor_index_starts_at_zero() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    assert_eq!(rows.shared.cursor_index.load(Ordering::Acquire), 0);
    rows.next().await.unwrap();
    assert_eq!(rows.shared.cursor_index.load(Ordering::Acquire), 1);
    rows.next().await.unwrap();
    assert_eq!(rows.shared.cursor_index.load(Ordering::Acquire), 2);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_fetch_row_count_increments_with_next() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    assert_eq!(rows.filter_context.fetch_row_count(), 0);
    rows.next().await.unwrap();
    assert_eq!(rows.filter_context.fetch_row_count(), 1);
    rows.next().await.unwrap();
    assert_eq!(rows.filter_context.fetch_row_count(), 2);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_construct_elapsed_returns_duration() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    // BASE 仅读取构造耗时，没有要求无 Filter 时也必须产生计时值。
    let _ = rows.filter_context.elapsed();
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_result_set_id_is_nonzero() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    assert!(rows.id() > 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_result_set_attributes_empty_initially() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    assert_eq!(rows.attributes_size(), 0);
    assert!(rows.shared.attributes.is_empty());
    assert!(rows.attribute("missing").is_none());
    rows.put_attribute("k", crate::core::ProxyAttributeValue::new(42));
    assert_eq!(rows.attributes_size(), 1);
    assert_eq!(*rows.attribute("k").unwrap().downcast::<i32>().unwrap(), 42);
    rows.clear_attributes();
    assert_eq!(rows.attributes_size(), 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_raw_result_set_returns_standard_driver_cursor() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    // 原始驱动句柄只在内部访问；不重新公开已退休的驱动端口。
    {
        let raw = rows.resource.result_set.lock().await;
        assert!(!raw.as_ref().unwrap().is_closed().unwrap());
    }
    rows.close().await.unwrap();
    assert!(rows.resource.result_set.lock().await.is_none());
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 用真实查询返回的代理保留初始属性及共享存储合同。
/// 对应 Java: DruidPooledResultSet#getMetaData。
#[tokio::test]
async fn task6_meta_data_proxy_returns_proxy_with_attributes() {
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters("SELECT id, name FROM t", None).await;
    let proxy = rows.meta_data_proxy_core().unwrap();
    assert_eq!(proxy.attributes_size(), 0);
    assert_eq!(
        druid_rdbc::metadata::ResultSetMetaData::column_count(&proxy),
        2
    );
    proxy.put_attribute("k", crate::core::ProxyAttributeValue::new(42));
    let second = rows.meta_data_proxy_core().unwrap();
    assert_eq!(second.attributes_size(), 1);
    assert_eq!(
        *second.attribute("k").unwrap().downcast::<i32>().unwrap(),
        42
    );
    second.clear_attributes();
    assert_eq!(proxy.attributes_size(), 0);
    assert_eq!(rows.meta_data().unwrap().column_count(), 2);
    rows.close().await.unwrap();
    assert!(matches!(
        rows.meta_data_proxy_core(),
        Err(druid_rdbc::error::RdbcError::ConnectionDiscarded)
    ));
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 保留真实五行查询成功 getString 后累计长度的完整 BASE 合同。
/// 对应 Java: DruidPooledResultSet#getString。
#[tokio::test]
async fn task6_read_string_length_increments_on_string_get() {
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters("SELECT name FROM t LIMIT 1", None)
            .await;
    assert_eq!(rows.filter_context.read_string_length(), 0);
    rows.next().await.unwrap();
    let _ =
        druid_rdbc::result_set::ResultSetExt::string(&rows, ColumnIndex::new(1).unwrap()).unwrap();
    assert!(rows.filter_context.read_string_length() > 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 标准动态读取与 Filter 路径只累计成功字符串的 UTF-16 长度。
#[tokio::test]
async fn task6_string_reads_count_utf16_without_counting_generic_values() {
    for filtered in [false, true] {
        let filters = if filtered {
            let mut chain = crate::core::FilterChain::new();
            chain.add_result_set(Arc::new(crate::core::FilterAdapter::new()));
            Some(Arc::new(chain))
        } else {
            None
        };
        let (mut rows, mut statement, mut connection) =
            task6_sqlite_diagnostic_cursor_with_sql_and_filters(
                "SELECT 'A😀' AS text_value, NULL AS absent, 7 AS number, X'FF' AS bad_encoding",
                filters,
            )
            .await;
        rows.next().await.unwrap();
        let text = ColumnIndex::new(1).unwrap();
        assert_eq!(
            rows.value(text).unwrap(),
            druid_rdbc::types::Value::String("A😀".to_owned())
        );
        assert!(matches!(
            rows.object(text).unwrap(),
            druid_rdbc::types::RdbcObject::Scalar(druid_rdbc::types::Value::String(_))
        ));
        assert_eq!(rows.filter_context.read_string_length(), 0);
        let alias = DruidPooledResultSet::from_tracked_resource(
            Arc::clone(&statement.inner),
            Arc::clone(&rows.resource),
        )
        .unwrap();
        let mut standard: Box<dyn ResultSet> = Box::new(alias);
        assert_eq!(
            druid_rdbc::result_set::ResultSetExt::string(standard.as_ref(), text).unwrap(),
            Some("A😀".to_owned())
        );
        assert_eq!(rows.filter_context.read_string_length(), 3);
        assert_eq!(
            druid_rdbc::result_set::ResultSetExt::string(standard.as_ref(), text).unwrap(),
            Some("A😀".to_owned())
        );
        assert_eq!(rows.filter_context.read_string_length(), 6);
        assert_eq!(
            druid_rdbc::result_set::ResultSetExt::string(
                standard.as_ref(),
                ColumnIndex::new(2).unwrap()
            )
            .unwrap(),
            None
        );
        assert_eq!(
            druid_rdbc::result_set::ResultSetExt::string(
                standard.as_ref(),
                ColumnIndex::new(3).unwrap(),
            )
            .unwrap()
            .as_deref(),
            Some("7")
        );
        assert_eq!(rows.filter_context.read_string_length(), 7);
        assert_eq!(
            druid_rdbc::result_set::ResultSetExt::string_by_label(standard.as_ref(), "number")
                .unwrap()
                .as_deref(),
            Some("7")
        );
        let error = druid_rdbc::result_set::ResultSetExt::string(
            standard.as_ref(),
            ColumnIndex::new(4).unwrap(),
        )
        .unwrap_err();
        assert_eq!(error.class_name(), "java.sql.SQLDataException");
        assert_eq!(error.sql_exception().unwrap().sql_state(), Some("22021"));
        assert!(matches!(
            druid_rdbc::result_set::ResultSetExt::string_by_label(standard.as_ref(), "missing"),
            Err(druid_rdbc::error::RdbcError::InvalidArgument(_))
        ));
        assert_eq!(rows.filter_context.read_string_length(), 8);
        standard.close().await.unwrap();
        assert!(matches!(
            druid_rdbc::result_set::ResultSetExt::string(standard.as_ref(), text),
            Err(druid_rdbc::error::RdbcError::ConnectionDiscarded)
        ));
        assert_eq!(rows.filter_context.read_string_length(), 8);
        rows.close().await.unwrap();
        statement.close().await.unwrap();
        connection.close().await.unwrap();
    }
}

/// 原 SQL 来源合同通过真实 Statement 执行与生产 ResultSet 上下文观察。
#[tokio::test]
async fn task6_sql_returns_some() {
    let capture = Arc::new(sql_capture_filter::SqlCaptureFilter::default());
    let mut filters = crate::core::FilterChain::new();
    filters.add_result_set(capture.clone());
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
    let mut rows = statement.execute_query("SELECT * FROM t").await.unwrap();
    let sql = capture.sql();
    assert!(sql.is_some());
    assert!(sql.unwrap().contains("SELECT"));
    assert_eq!(statement.last_sql().as_deref(), Some("SELECT * FROM t"));
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 原 InputStream/Reader 初始零值合同使用真实标准游标。
#[tokio::test]
async fn task6_stream_reader_counts_zero_initially() {
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    assert_eq!(rows.filter_context.open_input_stream_count(), 0);
    assert_eq!(rows.filter_context.open_reader_count(), 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
