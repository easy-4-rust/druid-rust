//! `DruidPooledStatement` 的 Java 对照与真实 SQLite 契约测试。

use druid::core::{
    DruidError, DruidPooledConnection, DruidPooledStatement, ExecResult, PhysicalConnection,
    PhysicalConnectionFactory, PhysicalStatement, PhysicalStatementOptions, Row, SqlTextStatement,
    StatementExecuteResult, StatementGeneratedKeys, Value, Wrapper, WrapperExt,
};
use druid::toasty::ToastyConnectionFactory;
use std::any::TypeId;
use std::sync::{Arc, Mutex};

async fn sqlite_pooled_connection() -> DruidPooledConnection {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 SQLite 物理连接");
    DruidPooledConnection::new(physical, 7, Box::new(|_, _| {}))
}

struct MultiResultConnection {
    requests: Arc<Mutex<Vec<StatementGeneratedKeys>>>,
    closed: bool,
}

#[async_trait::async_trait]
impl PhysicalConnection for MultiResultConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, DruidError> {
        Ok(ExecResult::default())
    }

    async fn execute(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        assert_eq!(sql, "MULTI");
        assert!(params.is_empty());
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(generated_keys);
        Ok(vec![
            StatementExecuteResult::ResultSet(vec![Row::new(vec![Value::Int(10)])]),
            StatementExecuteResult::Update(ExecResult {
                rows_affected: 2,
                last_insert_id: Some(99),
                row_count: None,
            }),
            StatementExecuteResult::ResultSet(vec![Row::new(vec![Value::Int(20)])]),
        ])
    }

    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(Vec::new())
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed
    }

    fn driver_name(&self) -> &str {
        "multi-result"
    }
}

#[tokio::test]
async fn sqlite_statement_executes_query_update_and_batch_on_same_lease() {
    let mut connection = sqlite_pooled_connection().await;
    let mut statement = connection
        .create_statement()
        .await
        .expect("createStatement 必须创建池化普通语句");

    statement
        .execute_update(
            &mut connection,
            "CREATE TABLE statement_event (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                label TEXT NOT NULL
            )",
        )
        .await
        .expect("DDL 必须通过真实 SQLite Statement 执行");
    let inserted = statement
        .execute_update(
            &mut connection,
            "INSERT INTO statement_event(label) VALUES ('single')",
        )
        .await
        .expect("单条更新必须成功");
    assert_eq!(inserted.rows_affected, 1);
    assert_eq!(inserted.last_insert_id, Some(1));
    assert_eq!(statement.update_count(&mut connection).unwrap(), 1);

    statement
        .add_batch(
            &mut connection,
            "INSERT INTO statement_event(label) VALUES ('batch-1')",
        )
        .unwrap();
    statement
        .add_batch(
            &mut connection,
            "INSERT INTO statement_event(label) VALUES ('batch-2')",
        )
        .unwrap();
    let batch = statement
        .execute_batch(&mut connection)
        .await
        .expect("批次必须在真实 SQLite 上执行");
    assert_eq!(batch, [1, 1]);
    assert_eq!(
        statement.update_count(&mut connection).unwrap(),
        -1,
        "Java StatementProxyImpl 只有单元素 batch 才设置 updateCount"
    );
    // Java Statement.executeBatch 不隐式承诺 clearBatch；Rust 同样保留快照。
    statement.clear_batch(&mut connection).unwrap();
    statement
        .add_batch(
            &mut connection,
            "INSERT INTO statement_event(label) VALUES ('batch-single')",
        )
        .unwrap();
    assert_eq!(statement.execute_batch(&mut connection).await.unwrap(), [1]);
    assert_eq!(statement.update_count(&mut connection).unwrap(), 1);
    statement.clear_batch(&mut connection).unwrap();

    let rows = statement
        .execute_query(
            &mut connection,
            "SELECT id, label FROM statement_event ORDER BY id",
        )
        .await
        .expect("查询必须在同一真实连接上执行");
    assert_eq!(rows.len(), 4);
    assert_eq!(
        rows[0].values,
        vec![Value::Int(1), Value::String("single".to_string())]
    );
    assert_eq!(statement.fetch_row_peak(), 4);
    assert_eq!(statement.update_count(&mut connection).unwrap(), -1);
    assert_eq!(statement.exception_count(), 0);
}

#[tokio::test]
async fn sqlite_generic_execute_preserves_first_result_generated_keys_and_more_results() {
    let mut connection = sqlite_pooled_connection().await;
    let mut statement = connection.create_statement().await.unwrap();
    statement
        .execute_update(
            &mut connection,
            "CREATE TABLE generic_event (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                label TEXT NOT NULL
            )",
        )
        .await
        .unwrap();

    assert!(
        statement
            .execute(&mut connection, "SELECT 7 AS value")
            .await
            .unwrap(),
        "RDBC execute 对 ResultSet 首结果必须返回 true"
    );
    assert_eq!(statement.update_count(&mut connection).unwrap(), -1);
    let mut result_set = statement
        .result_set(&mut connection)
        .unwrap()
        .expect("查询首结果必须可通过 getResultSet 获取");
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(result_set.int(&mut connection, 1).unwrap(), 7);
    assert!(!statement.more_results(&mut connection).unwrap());
    assert!(
        result_set.is_closed(),
        "Java Druid getMoreResults 成功后直接标记最后一个 wrapper closed"
    );
    assert_eq!(statement.update_count(&mut connection).unwrap(), -1);
    assert!(statement.result_set(&mut connection).unwrap().is_none());

    assert!(!statement
        .execute(
            &mut connection,
            "INSERT INTO generic_event(label) VALUES ('plain')",
        )
        .await
        .unwrap());
    assert_eq!(statement.update_count(&mut connection).unwrap(), 1);
    let mut plain_keys = statement.generated_keys(&mut connection).unwrap();
    assert!(plain_keys.next(&mut connection).unwrap());
    assert_eq!(plain_keys.long(&mut connection, 1).unwrap(), 1);

    assert!(!statement
        .execute_with_generated_keys(
            &mut connection,
            "INSERT INTO generic_event(label) VALUES ('auto')",
            1,
        )
        .await
        .unwrap());
    let mut requested_keys = statement.generated_keys(&mut connection).unwrap();
    assert!(requested_keys.next(&mut connection).unwrap());
    assert_eq!(requested_keys.long(&mut connection, 1).unwrap(), 2);

    let index_error = statement
        .execute_with_column_indexes(
            &mut connection,
            "INSERT INTO generic_event(label) VALUES ('index')",
            &[1],
        )
        .await
        .unwrap_err();
    assert!(matches!(
        index_error,
        DruidError::UnsupportedOperation {
            operation: "statement_execute_generated_key_columns"
        }
    ));
    assert_eq!(statement.update_count(&mut connection).unwrap(), -1);
    let mut empty_keys = statement.generated_keys(&mut connection).unwrap();
    assert!(!empty_keys.next(&mut connection).unwrap());

    let names = vec!["id".to_string()];
    assert!(matches!(
        statement
            .execute_with_column_names(
                &mut connection,
                "INSERT INTO generic_event(label) VALUES ('name')",
                &names,
            )
            .await,
        Err(DruidError::UnsupportedOperation {
            operation: "statement_execute_generated_key_columns"
        })
    ));

    assert!(statement
        .execute(&mut connection, "SELECT 8")
        .await
        .unwrap());
    let still_open = statement
        .result_set(&mut connection)
        .unwrap()
        .expect("非法 current 参数前 ResultSet 必须存在");
    assert!(matches!(
        statement.more_results_with_current(&mut connection, 999),
        Err(DruidError::InvalidArgument(_))
    ));
    assert!(
        !still_open.is_closed(),
        "非法 getMoreResults 参数不得提前关闭旧 ResultSet"
    );
    assert_eq!(statement.exception_count(), 3);
}

#[tokio::test]
async fn generic_execute_preserves_overload_arguments_and_ordered_multi_results() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let physical = Box::new(MultiResultConnection {
        requests: Arc::clone(&requests),
        closed: false,
    });
    let mut connection = DruidPooledConnection::new(physical, 17, Box::new(|_, _| {}));
    let mut statement = connection.create_statement().await.unwrap();
    let names = vec!["ID".to_string(), "id".to_string(), "ID".to_string()];

    assert!(statement
        .execute_with_column_names(&mut connection, "MULTI", &names)
        .await
        .unwrap());
    assert_eq!(
        requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_slice(),
        [StatementGeneratedKeys::ColumnNames(names)]
    );

    let mut first = statement.result_set(&mut connection).unwrap().unwrap();
    assert!(first.next(&mut connection).unwrap());
    assert_eq!(first.int(&mut connection, 1).unwrap(), 10);

    assert!(!statement.more_results(&mut connection).unwrap());
    assert!(first.is_closed());
    assert_eq!(statement.update_count(&mut connection).unwrap(), 2);
    assert!(statement.result_set(&mut connection).unwrap().is_none());

    assert!(statement
        .more_results_with_current(&mut connection, 2)
        .unwrap());
    assert_eq!(statement.update_count(&mut connection).unwrap(), -1);
    let mut third = statement.result_set(&mut connection).unwrap().unwrap();
    assert!(third.next(&mut connection).unwrap());
    assert_eq!(third.int(&mut connection, 1).unwrap(), 20);

    assert!(!statement
        .more_results_with_current(&mut connection, 3)
        .unwrap());
    assert!(third.is_closed());
    assert_eq!(statement.update_count(&mut connection).unwrap(), -1);
}

#[tokio::test]
async fn statement_overloads_preserve_result_set_options_and_properties() {
    let mut connection = sqlite_pooled_connection().await;
    let mut statement = connection
        .create_statement_with_holdability(1004, 1008, 2)
        .await
        .expect("完整 createStatement 重载必须成功");

    assert_eq!(statement.result_set_type(&connection).unwrap(), 1004);
    assert_eq!(statement.result_set_concurrency(&connection).unwrap(), 1008);
    assert_eq!(statement.result_set_holdability(&connection).unwrap(), 2);

    statement.set_max_field_size(&mut connection, 128).unwrap();
    statement.set_max_rows(&mut connection, 25).unwrap();
    statement.set_query_timeout(&mut connection, 3).unwrap();
    statement
        .set_fetch_direction(&mut connection, 1000)
        .unwrap();
    statement.set_fetch_size(&mut connection, 10).unwrap();
    statement
        .set_escape_processing(&mut connection, false)
        .unwrap();
    statement
        .set_cursor_name(&mut connection, "events")
        .unwrap();
    statement.clear_warnings(&mut connection).await.unwrap();
    statement.cancel(&mut connection).unwrap();

    assert_eq!(statement.max_field_size(&mut connection).unwrap(), 128);
    assert_eq!(statement.max_rows(&mut connection).unwrap(), 25);
    assert_eq!(statement.query_timeout(&mut connection).unwrap(), 3);
    assert_eq!(statement.fetch_direction(&mut connection).unwrap(), 1000);
    assert_eq!(statement.fetch_size(&mut connection).unwrap(), 10);
    assert!(!statement.is_poolable());
    statement.set_poolable(&mut connection, true).unwrap();
    assert!(matches!(
        statement.set_poolable(&mut connection, false),
        Err(DruidError::UnsupportedOperation {
            operation: "statement_set_poolable_false"
        })
    ));
    statement.close_on_completion(&mut connection).unwrap();
    assert!(statement.is_close_on_completion(&mut connection).unwrap());
    assert!(statement.is_wrapper_for_type::<dyn PhysicalStatement>());
    assert!(
        statement.unwrap_ref::<SqlTextStatement>().is_some(),
        "默认物理语句必须可按具体平台对象解包"
    );
}

#[tokio::test]
async fn statement_close_and_old_lease_rejection_match_java_lifecycle() {
    let returned: Arc<Mutex<Option<Box<dyn PhysicalConnection>>>> = Arc::new(Mutex::new(None));
    let returned_from_callback = Arc::clone(&returned);
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::new(
        physical,
        9,
        Box::new(move |physical, _| {
            *returned_from_callback
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(physical);
        }),
    );
    let mut statement = connection.create_statement().await.unwrap();
    statement
        .close_with_connection(&mut connection)
        .expect("首次关闭必须成功");
    statement
        .close_with_connection(&mut connection)
        .expect("重复关闭必须幂等");
    assert!(statement.is_closed());
    assert!(matches!(
        statement.execute_query(&mut connection, "SELECT 1").await,
        Err(DruidError::Other(message)) if message == "statement is closed"
    ));

    let mut old_statement = connection.create_statement().await.unwrap();
    connection.close().await.expect("池化连接必须归还");
    let physical = returned
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .expect("回调必须收到物理连接");
    let mut next_lease = DruidPooledConnection::new(physical, 9, Box::new(|_, _| {}));
    assert!(matches!(
        old_statement
            .execute_query(&mut next_lease, "SELECT 1")
            .await,
        Err(DruidError::Other(message)) if message == "statement is closed"
    ));
}

#[tokio::test]
async fn statement_errors_batch_failure_and_wrapper_branches_are_observable() {
    let mut connection = sqlite_pooled_connection().await;
    let mut statement = connection.create_statement().await.unwrap();

    assert!(!statement.statement().is_closed());
    assert!(format!("{statement:?}").contains("DruidPooledStatement"));
    assert!(statement.is_wrapper_for_type::<DruidPooledStatement>());
    assert_eq!(
        Wrapper::as_any(&statement).type_id(),
        TypeId::of::<DruidPooledStatement>()
    );
    assert!(statement.unwrap_ref::<DruidPooledStatement>().is_some());
    assert!(statement.is_wrapper_for_type::<SqlTextStatement>());
    assert!(statement
        .unwrap(Some(TypeId::of::<dyn PhysicalStatement>()))
        .and_then(|value| value.statement())
        .is_some());
    assert!(!statement.is_wrapper_for(None));
    assert!(statement.unwrap(None).is_none());

    assert!(statement
        .execute_query(&mut connection, "NOT A SQLITE QUERY")
        .await
        .is_err());
    assert!(statement
        .execute_update(&mut connection, "NOT A SQLITE UPDATE")
        .await
        .is_err());
    assert!(statement
        .execute_query_result_set(&mut connection, "NOT A SQLITE RESULT SET")
        .await
        .is_err());
    assert_eq!(statement.exception_count(), 3);

    statement
        .add_batch(&mut connection, "CREATE TABLE batch_ok(id INTEGER)")
        .unwrap();
    statement
        .add_batch(&mut connection, "NOT A SQLITE BATCH")
        .unwrap();
    let batch_error = statement.execute_batch(&mut connection).await.unwrap_err();
    assert_eq!(batch_error.batch_update_counts(), Some([0].as_slice()));
    assert!(matches!(
        batch_error,
        DruidError::BatchUpdateException { .. }
    ));
    assert_eq!(statement.exception_count(), 4);

    statement.close_with_connection(&mut connection).unwrap();
    // Java clearBatch 在 closed 分支直接返回，不再访问物理 Statement。
    statement.clear_batch(&mut connection).unwrap();
}

#[test]
fn physical_statement_rejects_invalid_values_and_closed_access() {
    let statement = SqlTextStatement::new(PhysicalStatementOptions::default());
    assert!(matches!(
        statement.set_max_field_size(-1),
        Err(DruidError::InvalidArgument(_))
    ));
    assert!(matches!(
        statement.set_max_rows(-1),
        Err(DruidError::InvalidArgument(_))
    ));
    assert!(matches!(
        statement.set_query_timeout(-1),
        Err(DruidError::InvalidArgument(_))
    ));
    assert!(matches!(
        statement.set_fetch_size(-1),
        Err(DruidError::InvalidArgument(_))
    ));
    statement.close().unwrap();
    assert!(statement.is_closed());
    assert!(matches!(
        statement.max_rows(),
        Err(DruidError::Other(message)) if message == "statement is closed"
    ));
}
