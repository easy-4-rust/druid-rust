//! 恢复 BASE generic execute 的缓冲行数与游标不提前消费语义。

#[path = "support/unknown_count_driver.rs"]
mod unknown_count_driver;

use druid::core::{
    AfterFilter, BeforeFilter, DruidError, ExecContext, ExecOperation, ExecResult, FilterChain,
};
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{
    CurrentResultDisposition, ExecutionResult, GeneratedKeys, ParameterIndex, PreparedParameter,
    StatementOptions,
};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionFactory;
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::{Arc, Mutex};

/// 单次 Filter 观察：SQL、执行类别、参数、是否为前置事件及可知行数。
type ExecuteEvent = (String, ExecOperation, Vec<Value>, bool, Option<u64>);

/// 记录真实 pooled execution 的前后 Filter 事件。对应 BASE: PreparedExecuteRecorder。
#[derive(Default)]
struct ExecuteRecorder {
    events: Mutex<Vec<ExecuteEvent>>,
    row_counts: Mutex<Vec<Option<u64>>>,
}

#[async_trait::async_trait]
impl BeforeFilter for ExecuteRecorder {
    fn name(&self) -> &str {
        "generic-execute-recorder"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        self.events.lock().unwrap().push((
            context.sql.clone(),
            context.operation,
            context.params.to_vec(),
            true,
            None,
        ));
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for ExecuteRecorder {
    fn name(&self) -> &str {
        "generic-execute-recorder"
    }

    async fn after(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        _elapsed: std::time::Duration,
    ) -> Result<(), DruidError> {
        self.row_counts
            .lock()
            .unwrap()
            .push(result.as_ref().unwrap().row_count);
        self.events.lock().unwrap().push((
            context.sql.clone(),
            context.operation,
            context.params.to_vec(),
            false,
            result
                .as_ref()
                .ok()
                .map(|result| result.row_count.unwrap_or(result.rows_affected)),
        ));
        Ok(())
    }
}

#[tokio::test]
async fn prepared_generic_execute_reports_buffered_rows_without_advancing_cursor() {
    exercise_buffered_execution(true).await;
}

#[tokio::test]
async fn statement_generic_execute_reports_buffered_rows_without_advancing_cursor() {
    exercise_buffered_execution(false).await;
}

#[tokio::test]
async fn mutable_standard_unwrap_operates_on_real_prepared_proxy() {
    use druid_rdbc::wrapper::StandardUnwrappedMut;
    use std::any::TypeId;

    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        let pool = DruidPool::builder()
            .factory(factory)
            .max_open(1)
            .max_idle(1)
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        let mut prepared = connection
            .prepare_statement("SELECT ?1 AS value", StatementOptions::default())
            .await
            .unwrap();
        prepared
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::Int(7),
            })
            .unwrap();
        let statement: &mut dyn druid_rdbc::statement::Statement = prepared.as_mut();
        let StandardUnwrappedMut::PreparedStatement(view) = statement
            .unwrap_mut(Some(TypeId::of::<
                dyn druid_rdbc::statement::PreparedStatement,
            >()))
            .unwrap()
        else {
            panic!("可变解包必须保留 PreparedStatement 叶子接口");
        };
        view.set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::Int(9),
        })
        .unwrap();
        let mut rows = view.execute_prepared_query().await.unwrap();
        assert!(rows.next().await.unwrap());
        assert_eq!(
            rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(9)
        );
        rows.close().await.unwrap();
        let mut original_rows = prepared.execute_prepared_query().await.unwrap();
        assert!(original_rows.next().await.unwrap());
        assert_eq!(
            original_rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(9)
        );
        let StandardUnwrappedMut::Statement(view) = prepared
            .unwrap_mut(Some(TypeId::of::<dyn druid_rdbc::statement::Statement>()))
            .unwrap()
        else {
            panic!("可变父接口必须保留同一对象");
        };
        view.close().await.unwrap();
        assert!(prepared.is_closed().unwrap());
        assert!(original_rows.is_closed().unwrap());
        connection.close().await.unwrap();
        pool.close().await;
    }
}

#[tokio::test]
async fn result_statement_preserves_prepared_owner_parameters_lifetime_and_cache() {
    use druid_rdbc::statement::PreparedStatement;
    use druid_rdbc::wrapper::StandardUnwrappedMut;
    use std::any::TypeId;

    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        let pool = DruidPool::builder()
            .factory(factory)
            .max_open(1)
            .max_idle(1)
            .pool_prepared_statements(true)
            .max_pool_prepared_statements_per_connection(1)
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        let sql = "SELECT ?1 AS shared_owner_value";
        let mut original = connection
            .prepare_statement(sql, StatementOptions::default())
            .await
            .unwrap();
        original
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::Int(7),
            })
            .unwrap();
        let mut first_rows = original.execute_prepared_query().await.unwrap();
        let mut returned = first_rows
            .get_statement()
            .expect("结果必须支持标准创建语句入口")
            .expect("Prepared 查询必须有创建语句");
        let StandardUnwrappedMut::PreparedStatement(leaf) = returned
            .unwrap_mut(Some(TypeId::of::<dyn PreparedStatement>()))
            .unwrap()
        else {
            panic!("返回句柄必须保留 Prepared 叶子接口");
        };
        leaf.set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::Int(9),
        })
        .unwrap();
        first_rows.close().await.unwrap();
        let mut from_original = original.execute_prepared_query().await.unwrap();
        assert!(from_original.next().await.unwrap());
        assert_eq!(
            from_original.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(9)
        );
        assert!(first_rows.is_closed().unwrap());
        drop(original);
        first_rows.close().await.unwrap();
        drop(first_rows);
        let StandardUnwrappedMut::PreparedStatement(leaf) = returned
            .unwrap_mut(Some(TypeId::of::<dyn PreparedStatement>()))
            .unwrap()
        else {
            panic!("原变量释放后必须仍保留 Prepared 叶子接口");
        };
        let mut surviving_rows = leaf.execute_prepared_query().await.unwrap();
        assert!(surviving_rows.next().await.unwrap());
        assert_eq!(
            surviving_rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(9)
        );
        let mut sibling = surviving_rows.get_statement().unwrap().unwrap();
        returned.close().await.unwrap();
        assert!(sibling.is_closed().unwrap());
        assert!(surviving_rows.is_closed().unwrap());
        assert!(from_original.is_closed().unwrap());
        assert_eq!(pool.state().cached_prepared_statement_count, 1);
        sibling.close().await.unwrap();
        returned.close().await.unwrap();
        assert_eq!(pool.state().cached_prepared_statement_count, 1);
        let mut reused = connection
            .prepare_statement(sql, StatementOptions::default())
            .await
            .unwrap();
        assert_eq!(pool.state().cached_prepared_statement_hit_count, 1);
        reused
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::Int(11),
            })
            .unwrap();
        let mut fresh_rows = reused.execute_prepared_query().await.unwrap();
        assert!(fresh_rows.next().await.unwrap());
        assert_eq!(
            fresh_rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(11)
        );
        assert!(returned.is_closed().unwrap());
        reused.close().await.unwrap();
        connection.close().await.unwrap();
        pool.close().await;
    }
}

#[tokio::test]
async fn integer_result_disposition_preserves_closed_statement_error_precedence() {
    exercise_integer_disposition_closed_owner(false).await;
}

#[tokio::test]
async fn integer_result_disposition_preserves_expired_connection_error_precedence() {
    exercise_integer_disposition_closed_owner(true).await;
}

async fn exercise_integer_disposition_closed_owner(close_connection: bool) {
    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        let pool = DruidPool::builder()
            .factory(factory)
            .max_open(1)
            .max_idle(1)
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        let ordinary = connection
            .create_statement(StatementOptions::default())
            .await
            .unwrap();
        let prepared = connection
            .prepare_statement("SELECT 7", StatementOptions::default())
            .await
            .unwrap();
        let mut statements: Vec<Box<dyn druid_rdbc::statement::Statement>> =
            vec![ordinary, prepared];
        if close_connection {
            connection.close().await.unwrap();
        } else {
            for statement in &mut statements {
                statement.close().await.unwrap();
            }
        }
        for (index, statement) in statements.iter_mut().enumerate() {
            for current in [999, 0, -1, 1, 2, 3] {
                assert!(matches!(statement.get_more_results_current(current).await,
                    Err(druid_rdbc::error::RdbcError::ConnectionDiscarded)),
                    "关闭/失效检查必须先于整数校验：prepared={}, connection_closed={close_connection}, current={current}", index == 1);
            }
        }
        connection.close().await.unwrap();
        pool.close().await;
    }
}

#[tokio::test]
async fn prepared_integer_result_disposition_validates_before_cursor_side_effects() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let pool = DruidPool::builder()
        .factory(Arc::new(factory))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut prepared = connection
        .prepare_statement("SELECT ?1 AS value", StatementOptions::default())
        .await
        .unwrap();
    for (value, disposition) in [(7, 1), (8, 2), (9, 3)] {
        prepared
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::Int(value),
            })
            .unwrap();
        let mut results = prepared.execute_prepared().await.unwrap();
        let ExecutionResult::ResultSet(mut rows) = results.remove(0) else {
            panic!("SELECT 必须返回游标");
        };
        for invalid in [999, 0, -1, i32::MAX] {
            assert!(
                matches!(
                    prepared.get_more_results_current(invalid).await,
                    Err(druid_rdbc::error::RdbcError::InvalidArgument(_))
                ),
                "invalid={invalid}"
            );
            assert!(!rows.is_closed().unwrap());
            assert_eq!(prepared.get_update_count().unwrap(), None);
        }
        assert!(rows.next().await.unwrap());
        assert_eq!(
            rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(value)
        );
        assert!(!prepared
            .get_more_results_current(disposition)
            .await
            .unwrap());
        assert_eq!(rows.is_closed().unwrap(), disposition != 2);
        if disposition == 2 {
            assert_eq!(
                rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
                Value::Int(value)
            );
            assert!(!prepared.get_more_results_current(3).await.unwrap());
            assert!(rows.is_closed().unwrap());
        }
    }
    prepared.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn toasty_prepared_generic_execute_preserves_base_filter_and_generated_key_overloads() {
    let recorder = Arc::new(ExecuteRecorder::default());
    let mut chain = FilterChain::new();
    chain.add_before(recorder.clone());
    chain.add_after(recorder.clone());
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let pool = DruidPool::builder()
        .factory(Arc::new(factory))
        .max_open(1)
        .max_idle(1)
        .filter_chain(Arc::new(chain))
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    connection.exec(
        "CREATE TABLE prepared_execute_item(id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT NOT NULL)",
        Vec::new(),
    ).await.unwrap();
    recorder.events.lock().unwrap().clear();
    recorder.row_counts.lock().unwrap().clear();

    let query_sql = "SELECT ?1 AS value";
    let mut query = connection
        .prepare_statement(query_sql, StatementOptions::default())
        .await
        .unwrap();
    query
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::Int(7),
        })
        .unwrap();
    let mut results = query.execute_prepared().await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(query.get_update_count().unwrap(), None);
    let ExecutionResult::ResultSet(mut rows) = results.remove(0) else {
        panic!("BASE SELECT 7 必须返回游标");
    };
    assert_eq!(
        *recorder.events.lock().unwrap(),
        vec![
            (
                query_sql.to_owned(),
                ExecOperation::Execute,
                vec![Value::Int(7)],
                true,
                None
            ),
            (
                query_sql.to_owned(),
                ExecOperation::Execute,
                vec![Value::Int(7)],
                false,
                Some(1)
            ),
        ]
    );
    assert_eq!(*recorder.row_counts.lock().unwrap(), vec![Some(1)]);
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(7)
    );
    assert!(!query
        .get_more_results(CurrentResultDisposition::Default)
        .await
        .unwrap());
    assert!(rows.is_closed().unwrap());

    query
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::Int(8),
        })
        .unwrap();
    let mut results = query.execute_prepared().await.unwrap();
    let ExecutionResult::ResultSet(mut retained_rows) = results.remove(0) else {
        panic!("BASE SELECT 8 必须返回游标");
    };
    // typed KeepCurrent 的保留行为独立验证；不将它冒充 BASE 无效整数999的失败合同。
    assert!(!query
        .get_more_results(CurrentResultDisposition::KeepCurrent)
        .await
        .unwrap());
    assert!(!retained_rows.is_closed().unwrap());
    assert!(retained_rows.next().await.unwrap());
    assert_eq!(
        retained_rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(8)
    );
    query.close().await.unwrap();
    assert!(retained_rows.is_closed().unwrap());

    let insert_sql = "INSERT INTO prepared_execute_item(value) VALUES (?1)";
    for (offset, (label, request)) in [
        ("plain", GeneratedKeys::None),
        ("auto", GeneratedKeys::ReturnGeneratedKeys),
        (
            "indexed",
            GeneratedKeys::ColumnIndexes(vec![ColumnIndex::new(1).unwrap()]),
        ),
        ("named", GeneratedKeys::ColumnNames(vec!["id".to_owned()])),
    ]
    .into_iter()
    .enumerate()
    {
        let mut insert = connection
            .prepare_statement_with_generated_keys(insert_sql, StatementOptions::default(), request)
            .await
            .unwrap();
        let parameter_value = Value::String(label.to_owned());
        insert
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: parameter_value.clone(),
            })
            .unwrap();
        recorder.events.lock().unwrap().clear();
        recorder.row_counts.lock().unwrap().clear();
        let results = insert.execute_prepared().await.unwrap();
        assert!(matches!(
            results.as_slice(),
            [ExecutionResult::UpdateCount(1)]
        ));
        assert_eq!(insert.get_update_count().unwrap(), Some(1));
        assert_eq!(
            *recorder.events.lock().unwrap(),
            vec![
                (
                    insert_sql.to_owned(),
                    ExecOperation::Execute,
                    vec![parameter_value.clone()],
                    true,
                    None
                ),
                (
                    insert_sql.to_owned(),
                    ExecOperation::Execute,
                    vec![parameter_value],
                    false,
                    Some(1)
                ),
            ]
        );
        assert_eq!(*recorder.row_counts.lock().unwrap(), vec![None]);
        let mut keys = insert.generated_keys().await.unwrap();
        assert_eq!(keys.meta_data().unwrap().column_count(), 1);
        assert!(keys.next().await.unwrap());
        assert_eq!(
            keys.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(i64::try_from(offset + 1).unwrap())
        );
        assert!(!keys.next().await.unwrap());
        insert.close().await.unwrap();
        assert!(keys.is_closed().unwrap());
    }
    let rows = connection
        .fetch(
            "SELECT id, value FROM prepared_execute_item ORDER BY id",
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| row.values.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![Value::Int(1), Value::String("plain".to_owned())],
            vec![Value::Int(2), Value::String("auto".to_owned())],
            vec![Value::Int(3), Value::String("indexed".to_owned())],
            vec![Value::Int(4), Value::String("named".to_owned())],
        ]
    );
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn statement_close_cascades_to_real_buffered_cursors_without_more_results() {
    exercise_close_cascade(false, false).await;
}

#[tokio::test]
async fn prepared_close_cascades_to_real_buffered_cursors_without_more_results() {
    exercise_close_cascade(true, false).await;
}

#[tokio::test]
async fn prepared_cache_reuse_keeps_prior_cursor_closed() {
    exercise_close_cascade(true, true).await;
}

async fn exercise_close_cascade(prepared: bool, cached: bool) {
    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        let pool = DruidPool::builder()
            .factory(factory)
            .max_open(1)
            .max_idle(1)
            .pool_prepared_statements(cached)
            .max_pool_prepared_statements_per_connection(1)
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        {
            let (mut statement, mut results) = if prepared {
                let mut statement = connection
                    .prepare_statement("SELECT 7 AS value", StatementOptions::default())
                    .await
                    .unwrap();
                let results = statement.execute_prepared().await.unwrap();
                (
                    statement as Box<dyn druid_rdbc::statement::Statement>,
                    results,
                )
            } else {
                let mut statement = connection
                    .create_statement(StatementOptions::default())
                    .await
                    .unwrap();
                let results = statement
                    .execute("SELECT 7 AS value", GeneratedKeys::None)
                    .await
                    .unwrap();
                (statement, results)
            };
            let ExecutionResult::ResultSet(mut rows) = results.remove(0) else {
                panic!("必须返回 cursor");
            };
            assert_eq!(rows.known_row_count(), Some(1));
            statement.close().await.unwrap();
            assert!(
                rows.is_closed().unwrap(),
                "Statement close 必须级联关闭已返回的子游标，prepared={prepared}"
            );
            assert_eq!(rows.known_row_count(), None);
            assert!(rows.next().await.is_err());
            statement.close().await.unwrap();
            if cached {
                assert_eq!(pool.state().cached_prepared_statement_count, 1);
                let mut reused = connection
                    .prepare_statement("SELECT 7 AS value", StatementOptions::default())
                    .await
                    .unwrap();
                assert_eq!(pool.state().cached_prepared_statement_hit_count, 1);
                let mut results = reused.execute_prepared().await.unwrap();
                let ExecutionResult::ResultSet(mut fresh_rows) = results.remove(0) else {
                    panic!("必须返回新 cursor");
                };
                assert!(fresh_rows.next().await.unwrap());
                assert_eq!(
                    fresh_rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
                    Value::Int(7)
                );
                assert!(rows.is_closed().unwrap());
                assert_eq!(rows.known_row_count(), None);
                reused.close().await.unwrap();
                assert!(fresh_rows.is_closed().unwrap());
                assert_eq!(pool.state().cached_prepared_statement_count, 1);
            }
        }
        connection.close().await.unwrap();
        pool.close().await;
    }
}

#[tokio::test]
async fn unknown_cursor_summary_preserves_none_and_does_not_call_next() {
    use druid::core::{DruidConnectionHolder, DruidPooledConnection};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use unknown_count_driver::UnknownCountConnection;

    let next_calls = Arc::new(AtomicUsize::new(0));
    let recorder = Arc::new(ExecuteRecorder::default());
    let mut chain = FilterChain::new();
    chain.add_after(recorder.clone());
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(UnknownCountConnection::new(next_calls.clone())),
        94,
        std::time::Duration::ZERO,
        0,
    )
    .await
    .unwrap();
    let mut connection = DruidPooledConnection::with_holder(
        holder,
        "unknown-cursor".to_owned(),
        Some(Arc::new(chain)),
        false,
        Box::new(|_, _| false),
    );
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut results = statement
        .execute("SELECT delayed_value", GeneratedKeys::None)
        .await
        .unwrap();
    assert_eq!(*recorder.row_counts.lock().unwrap(), vec![None]);
    assert_eq!(
        next_calls.load(Ordering::SeqCst),
        0,
        "summary 和 proxy open 不得读取尚未到达的行"
    );
    let ExecutionResult::ResultSet(mut rows) = results.remove(0) else {
        panic!("必须返回 cursor");
    };
    assert_eq!(rows.known_row_count(), None);
    assert!(rows.next().await.unwrap());
    assert_eq!(next_calls.load(Ordering::SeqCst), 1);
    assert_eq!(rows.known_row_count(), None);
    statement.close().await.unwrap();
    assert!(rows.is_closed().unwrap());
    connection.close().await.unwrap();
}

#[tokio::test]
async fn cursor_close_failure_retains_statement_and_retries_release_once() {
    exercise_interrupted_cursor_close(false, false).await;
}

#[tokio::test]
async fn cancelled_cursor_close_retains_statement_and_retries_release_once() {
    exercise_interrupted_cursor_close(true, false).await;
}

#[tokio::test]
async fn returned_prepared_owner_retries_failed_child_close_once() {
    exercise_interrupted_cursor_close(false, true).await;
}

#[tokio::test]
async fn returned_prepared_owner_retries_cancelled_child_close_once() {
    exercise_interrupted_cursor_close(true, true).await;
}

async fn exercise_interrupted_cursor_close(cancel: bool, prepared: bool) {
    use druid::core::{DruidConnectionHolder, DruidPooledConnection};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use unknown_count_driver::{CursorCloseProbe, UnknownCountConnection};

    let probe = Arc::new(CursorCloseProbe::default());
    probe.fail_once.store(!cancel, Ordering::SeqCst);
    probe.block_once.store(cancel, Ordering::SeqCst);
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(UnknownCountConnection::with_close_probe(
            Arc::new(AtomicUsize::new(0)),
            probe.clone(),
        )),
        95,
        std::time::Duration::ZERO,
        0,
    )
    .await
    .unwrap();
    let mut connection = DruidPooledConnection::with_holder(
        holder,
        "close-retry".to_owned(),
        None,
        false,
        Box::new(|_, _| false),
    );
    let (statement, mut results): (Box<dyn druid_rdbc::statement::Statement>, _) = if prepared {
        let mut statement = connection
            .prepare_statement("SELECT delayed_value", StatementOptions::default())
            .await
            .unwrap();
        let results = statement.execute_prepared().await.unwrap();
        (statement, results)
    } else {
        let mut statement = connection
            .create_statement(StatementOptions::default())
            .await
            .unwrap();
        let results = statement
            .execute("SELECT delayed_value", GeneratedKeys::None)
            .await
            .unwrap();
        (statement, results)
    };
    let ExecutionResult::ResultSet(rows) = results.remove(0) else {
        panic!("必须返回 cursor");
    };
    let mut returned = rows.get_statement().unwrap().unwrap();
    let mut retry_owner = rows.get_statement().unwrap().unwrap();
    drop(statement);
    let statement = &mut returned;
    if cancel {
        let mut closing = Box::pin(statement.close());
        tokio::select! {
            result = &mut closing => panic!("必须等待在 raw cursor close，actual={result:?}"),
            () = probe.entered.notified() => {}
        }
        drop(closing);
    } else {
        assert!(
            matches!(statement.close().await, Err(druid_rdbc::error::RdbcError::DriverError(message)) if message == "cursor close failed once")
        );
    }
    assert!(!statement.is_closed().unwrap());
    assert!(!rows.is_closed().unwrap());
    assert_eq!(probe.cursor_close_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(
        probe.statement_closes.load(Ordering::SeqCst),
        0,
        "子资源未结束前不能关闭或归还 raw statement"
    );
    retry_owner.close().await.unwrap();
    assert!(rows.is_closed().unwrap());
    assert!(statement.is_closed().unwrap());
    assert_eq!(probe.cursor_close_attempts.load(Ordering::SeqCst), 2);
    assert_eq!(probe.statement_closes.load(Ordering::SeqCst), 1);
    statement.close().await.unwrap();
    assert_eq!(probe.cursor_close_attempts.load(Ordering::SeqCst), 2);
    assert_eq!(probe.statement_closes.load(Ordering::SeqCst), 1);
    connection.close().await.unwrap();
}

async fn exercise_buffered_execution(prepared: bool) {
    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        let recorder = Arc::new(ExecuteRecorder::default());
        let mut chain = FilterChain::new();
        chain.add_before(recorder.clone());
        chain.add_after(recorder.clone());
        let pool = DruidPool::builder()
            .factory(factory)
            .max_open(1)
            .max_idle(1)
            .filter_chain(Arc::new(chain))
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        let sql = if prepared {
            "SELECT ?1 AS value UNION ALL SELECT 8"
        } else {
            "SELECT 7 AS value UNION ALL SELECT 8"
        };
        let mut statement = if prepared {
            let mut statement = connection
                .prepare_statement(sql, StatementOptions::default())
                .await
                .unwrap();
            statement
                .set_parameter(PreparedParameter {
                    index: ParameterIndex::new(1).unwrap(),
                    value: Value::Int(7),
                })
                .unwrap();
            let results = statement.execute_prepared().await.unwrap();
            (
                statement as Box<dyn druid_rdbc::statement::Statement>,
                results,
            )
        } else {
            let mut statement = connection
                .create_statement(StatementOptions::default())
                .await
                .unwrap();
            let results = statement.execute(sql, GeneratedKeys::None).await.unwrap();
            (statement, results)
        };
        assert_eq!(statement.0.get_update_count().unwrap(), None);
        let params = if prepared {
            vec![Value::Int(7)]
        } else {
            vec![]
        };
        assert_eq!(
            *recorder.events.lock().unwrap(),
            vec![
                (
                    sql.to_owned(),
                    ExecOperation::Execute,
                    params.clone(),
                    true,
                    None
                ),
                (
                    sql.to_owned(),
                    ExecOperation::Execute,
                    params,
                    false,
                    Some(2)
                ),
            ],
            "generic execute 必须只进入/退出 Filter 各一次并保留真实缓冲行数"
        );
        assert_eq!(statement.1.len(), 1);
        let ExecutionResult::ResultSet(mut rows) = statement.1.remove(0) else {
            panic!("SELECT 必须返回 ResultSet");
        };
        assert_eq!(rows.known_row_count(), Some(2));
        let column = ColumnIndex::new(1).unwrap();
        assert!(rows.next().await.unwrap());
        assert_eq!(rows.value(column).unwrap(), Value::Int(7));
        assert!(rows.next().await.unwrap());
        assert_eq!(rows.value(column).unwrap(), Value::Int(8));
        assert!(!rows.next().await.unwrap());
        assert_eq!(rows.known_row_count(), Some(2), "总行数不是剩余行数");
        assert!(!statement
            .0
            .get_more_results(CurrentResultDisposition::CloseCurrent)
            .await
            .unwrap());
        assert!(rows.is_closed().unwrap());
        assert_eq!(rows.known_row_count(), None);
        statement.0.close().await.unwrap();
        connection.close().await.unwrap();
        pool.close().await;
    }
}
