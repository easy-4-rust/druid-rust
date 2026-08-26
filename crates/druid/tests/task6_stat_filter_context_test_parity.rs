//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Java `StatFilterContext` 与 `StatFilterContextListener` 的监听器及真实 `SQLite` 语义。

use druid::core::{
    Connection, ConnectionFactory, DruidConnectionHolder, DruidError, DruidPooledConnection,
    FilterChain, RdbcError,
};
use druid::stats::{
    StatFilter, StatFilterContext, StatFilterContextListener, StatFilterContextListenerAdapter,
    StatsCollector,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::{
    ExecutionResult, GeneratedKeys, ParameterIndex, PreparedParameter, PreparedStatement,
    Statement, StatementOptions,
};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct RecordingListener {
    label: &'static str,
    events: Arc<Mutex<Vec<String>>>,
    fail_on: Option<&'static str>,
}

impl RecordingListener {
    fn record(&self, event: String) -> Result<(), DruidError> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{}:{event}", self.label));
        if self.fail_on == Some(event.split(':').next().unwrap_or_default()) {
            return Err(DruidError::Other(format!("{} failed", self.label)));
        }
        Ok(())
    }
}

impl StatFilterContextListener for RecordingListener {
    fn add_update_count(&self, update_count: i32) -> Result<(), DruidError> {
        self.record(format!("add_update_count:{update_count}"))
    }

    fn add_fetch_row_count(&self, fetch_row_count: i32) -> Result<(), DruidError> {
        self.record(format!("add_fetch_row_count:{fetch_row_count}"))
    }

    fn execute_before(&self, sql: &str, in_transaction: bool) -> Result<(), DruidError> {
        self.record(format!("execute_before:{sql}:{in_transaction}"))
    }

    fn execute_after(
        &self,
        sql: Option<&str>,
        nano_span: i64,
        error: Option<&DruidError>,
    ) -> Result<(), DruidError> {
        let sql = sql.unwrap_or("null");
        self.record(format!(
            "execute_after:{sql}:{nano_span}:{}",
            error.map_or_else(|| "null".to_string(), ToString::to_string)
        ))
    }

    fn commit(&self) -> Result<(), DruidError> {
        self.record("commit".to_string())
    }

    fn rollback(&self) -> Result<(), DruidError> {
        self.record("rollback".to_string())
    }

    fn pool_connect(&self) -> Result<(), DruidError> {
        self.record("pool_connect".to_string())
    }

    fn pool_close(&self, nanos: i64) -> Result<(), DruidError> {
        self.record(format!("pool_close:{nanos}"))
    }

    fn connection_open(&self) -> Result<(), DruidError> {
        self.record("connection_open".to_string())
    }

    fn connection_close(&self, nanos: i64) -> Result<(), DruidError> {
        self.record(format!("connection_close:{nanos}"))
    }

    fn result_set_open(&self) -> Result<(), DruidError> {
        self.record("result_set_open".to_string())
    }

    fn result_set_close(&self, nanos: i64) -> Result<(), DruidError> {
        self.record(format!("result_set_close:{nanos}"))
    }

    fn clob_open(&self) -> Result<(), DruidError> {
        self.record("clob_open".to_string())
    }

    fn blob_open(&self) -> Result<(), DruidError> {
        self.record("blob_open".to_string())
    }
}

struct AddingListener {
    context: Arc<StatFilterContext>,
    late: Arc<dyn StatFilterContextListener>,
    added: AtomicBool,
    events: Arc<Mutex<Vec<String>>>,
}

impl StatFilterContextListener for AddingListener {
    fn add_update_count(&self, _update_count: i32) -> Result<(), DruidError> {
        Ok(())
    }

    fn add_fetch_row_count(&self, _fetch_row_count: i32) -> Result<(), DruidError> {
        Ok(())
    }

    fn execute_before(&self, _sql: &str, _in_transaction: bool) -> Result<(), DruidError> {
        Ok(())
    }

    fn execute_after(
        &self,
        _sql: Option<&str>,
        _nano_span: i64,
        _error: Option<&DruidError>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    fn commit(&self) -> Result<(), DruidError> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push("adding:commit".to_string());
        if !self.added.swap(true, Ordering::AcqRel) {
            self.context.add_context_listener(Arc::clone(&self.late));
        }
        Ok(())
    }

    fn rollback(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn pool_connect(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn pool_close(&self, _nanos: i64) -> Result<(), DruidError> {
        Ok(())
    }

    fn connection_open(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn connection_close(&self, _nanos: i64) -> Result<(), DruidError> {
        Ok(())
    }

    fn result_set_open(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn result_set_close(&self, _nanos: i64) -> Result<(), DruidError> {
        Ok(())
    }

    fn clob_open(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn blob_open(&self) -> Result<(), DruidError> {
        Ok(())
    }
}

fn events(events: &Mutex<Vec<String>>) -> Vec<String> {
    events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

#[test]
fn task6_stat_filter_context_test_context_dispatches_all_java_events_in_registration_order() {
    let default_context = StatFilterContext::default();
    assert!(default_context.listeners().is_empty());

    let context = StatFilterContext::new();
    let event_log = Arc::new(Mutex::new(Vec::new()));
    let first: Arc<dyn StatFilterContextListener> = Arc::new(RecordingListener {
        label: "first",
        events: Arc::clone(&event_log),
        fail_on: None,
    });
    let second: Arc<dyn StatFilterContextListener> = Arc::new(RecordingListener {
        label: "second",
        events: Arc::clone(&event_log),
        fail_on: None,
    });
    context.add_context_listener(Arc::clone(&first));
    context.add_context_listener(Arc::clone(&second));

    context.add_update_count(2).unwrap();
    context.add_fetch_row_count(3).unwrap();
    context.execute_before("SELECT 1", true).unwrap();
    let error = DruidError::DriverError("boom".to_string());
    context
        .execute_after(Some("SELECT 1"), 7, Some(&error))
        .unwrap();
    context.execute_after(Some("SELECT 2"), 8, None).unwrap();
    context.commit().unwrap();
    context.rollback().unwrap();
    context.pool_connection_open().unwrap();
    context.pool_connection_close(9).unwrap();
    context.connection_open().unwrap();
    context.connection_close(10).unwrap();
    context.result_set_open().unwrap();
    context.result_set_close(11).unwrap();
    context.clob_open().unwrap();
    context.blob_open().unwrap();

    let recorded = events(&event_log);
    assert_eq!(recorded.len(), 30);
    for pair in recorded.chunks_exact(2) {
        assert!(pair[0].starts_with("first:"));
        assert_eq!(
            pair[0].strip_prefix("first:"),
            pair[1].strip_prefix("second:")
        );
    }
}

#[test]
fn task6_stat_filter_context_test_listener_adapter_is_a_complete_no_op_java_object() {
    let adapter = StatFilterContextListenerAdapter::new();
    let cloned = adapter;
    let defaulted = StatFilterContextListenerAdapter;
    assert!(format!("{adapter:?}").contains("StatFilterContextListenerAdapter"));

    adapter.add_update_count(i32::MIN).unwrap();
    adapter.add_fetch_row_count(i32::MAX).unwrap();
    adapter.execute_before("SELECT 1", true).unwrap();
    adapter
        .execute_after(
            Some("SELECT 1"),
            i64::MAX,
            Some(&DruidError::DriverError("boom".to_string())),
        )
        .unwrap();
    adapter
        .execute_after(Some("SELECT 2"), i64::MIN, None)
        .unwrap();
    adapter.commit().unwrap();
    adapter.rollback().unwrap();
    adapter.pool_connect().unwrap();
    adapter.pool_close(i64::MAX).unwrap();
    cloned.connection_open().unwrap();
    cloned.connection_close(i64::MIN).unwrap();
    defaulted.result_set_open().unwrap();
    defaulted.result_set_close(0).unwrap();
    defaulted.clob_open().unwrap();
    defaulted.blob_open().unwrap();
}

#[test]
fn task6_stat_filter_context_test_duplicate_remove_indexed_mutation_and_error_short_circuit_match_java_list(
) {
    let duplicate_context = StatFilterContext::new();
    let duplicate_log = Arc::new(Mutex::new(Vec::new()));
    let duplicate: Arc<dyn StatFilterContextListener> = Arc::new(RecordingListener {
        label: "duplicate",
        events: Arc::clone(&duplicate_log),
        fail_on: None,
    });
    duplicate_context.add_context_listener(Arc::clone(&duplicate));
    duplicate_context.add_context_listener(Arc::clone(&duplicate));
    assert_eq!(duplicate_context.listeners().len(), 2);
    assert!(duplicate_context.remove_context_listener(&duplicate));
    assert_eq!(duplicate_context.listeners().len(), 1);
    duplicate_context.commit().unwrap();
    assert_eq!(events(&duplicate_log), ["duplicate:commit"]);
    assert!(duplicate_context.remove_context_listener(&duplicate));
    assert!(!duplicate_context.remove_context_listener(&duplicate));

    let snapshot_context = Arc::new(StatFilterContext::new());
    let snapshot_log = Arc::new(Mutex::new(Vec::new()));
    let late: Arc<dyn StatFilterContextListener> = Arc::new(RecordingListener {
        label: "late",
        events: Arc::clone(&snapshot_log),
        fail_on: None,
    });
    let adding: Arc<dyn StatFilterContextListener> = Arc::new(AddingListener {
        context: Arc::clone(&snapshot_context),
        late,
        added: AtomicBool::new(false),
        events: Arc::clone(&snapshot_log),
    });
    snapshot_context.add_context_listener(adding);
    snapshot_context.commit().unwrap();
    assert_eq!(events(&snapshot_log), ["adding:commit", "late:commit"]);
    snapshot_context.commit().unwrap();
    assert_eq!(
        events(&snapshot_log),
        [
            "adding:commit",
            "late:commit",
            "adding:commit",
            "late:commit"
        ]
    );

    let error_context = StatFilterContext::new();
    let error_log = Arc::new(Mutex::new(Vec::new()));
    for (label, fail_on) in [("first", None), ("second", Some("commit")), ("third", None)] {
        error_context.add_context_listener(Arc::new(RecordingListener {
            label,
            events: Arc::clone(&error_log),
            fail_on,
        }));
    }
    assert_eq!(
        error_context.commit(),
        Err(DruidError::Other("second failed".to_string()))
    );
    assert_eq!(events(&error_log), ["first:commit", "second:commit"]);
}

#[tokio::test]
async fn task6_stat_filter_context_test_global_context_is_singleton_and_stat_filter_emits_real_sqlite_events(
) {
    assert!(std::ptr::eq(
        StatFilterContext::global(),
        StatFilterContext::global()
    ));

    let event_log = Arc::new(Mutex::new(Vec::new()));
    let listener: Arc<dyn StatFilterContextListener> = Arc::new(RecordingListener {
        label: "sqlite",
        events: Arc::clone(&event_log),
        fail_on: None,
    });

    let collector = Arc::new(StatsCollector::new(
        "context-sqlite",
        Duration::from_secs(1),
    ));
    let stat_filter = Arc::new(StatFilter::new(Arc::clone(&collector)));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_filter(stat_filter);
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let holder = DruidConnectionHolder::with_standard_connection(physical, 73, Duration::ZERO, 0)
        .await
        .unwrap();
    let mut connection = DruidPooledConnection::with_standard_holder(
        holder,
        "context-sqlite".to_string(),
        Some(Arc::new(filter_chain)),
        false,
        false,
        Box::new(|_, _| true),
    );
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement
        .execute_update(
            "CREATE TABLE context_event(id INTEGER PRIMARY KEY, value TEXT)",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    StatFilterContext::global().add_context_listener(Arc::clone(&listener));

    let query_sql = "SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3";
    let mut result_set = statement.execute_query(query_sql).await.unwrap();
    assert!(result_set.next().await.unwrap());
    assert!(result_set.next().await.unwrap());
    result_set.close().await.unwrap();

    let recorded = events(&event_log);
    assert_eq!(
        recorded[0],
        format!("sqlite:execute_before:{query_sql}:false")
    );
    assert!(recorded[1].starts_with(&format!("sqlite:execute_after:{query_sql}:")));
    assert!(recorded[1].ends_with(":null"));
    assert_eq!(recorded[2], "sqlite:result_set_open");
    assert_eq!(recorded[3], "sqlite:add_fetch_row_count:2");
    assert!(recorded[4].starts_with("sqlite:result_set_close:"));

    let batch_sql_1 = "INSERT INTO context_event(id, value) VALUES (10, 'batch-1')";
    let batch_sql_2 = "INSERT INTO context_event(id, value) VALUES (11, 'batch-2')";
    statement.add_batch(batch_sql_1.to_owned()).unwrap();
    statement.add_batch(batch_sql_2.to_owned()).unwrap();
    assert_eq!(statement.execute_batch().await.unwrap(), [1, 1]);
    assert_eq!(statement.get_update_count().unwrap(), None);
    statement.clear_batch().unwrap();

    let prepared_batch_sql = "INSERT INTO context_event(id, value) VALUES (?1, ?2)";
    let mut prepared_statement = connection
        .prepare_statement(prepared_batch_sql, StatementOptions::default())
        .await
        .unwrap();
    prepared_statement
        .add_parameter_batch(vec![
            PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: druid::core::Value::Int(30),
            },
            PreparedParameter {
                index: ParameterIndex::new(2).unwrap(),
                value: druid::core::Value::String("prepared-1".to_string()),
            },
        ])
        .unwrap();
    prepared_statement
        .add_parameter_batch(vec![
            PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: druid::core::Value::Int(31),
            },
            PreparedParameter {
                index: ParameterIndex::new(2).unwrap(),
                value: druid::core::Value::String("prepared-2".to_string()),
            },
        ])
        .unwrap();
    prepared_statement.clear_parameters().unwrap();
    assert_eq!(
        prepared_statement.execute_prepared_batch().await.unwrap(),
        [1, 1]
    );
    prepared_statement.close().await.unwrap();

    let generic_query_sql = "SELECT 41";
    let mut generic_results = statement
        .execute(generic_query_sql, GeneratedKeys::None)
        .await
        .unwrap();
    let ExecutionResult::ResultSet(mut generic_result_set) = generic_results.remove(0) else {
        panic!("generic query 必须产生当前 ResultSet");
    };
    assert!(generic_result_set.next().await.unwrap());
    generic_result_set.close().await.unwrap();

    let generic_update_sql = "INSERT INTO context_event(id, value) VALUES (40, 'generic-update')";
    let generic_update_results = statement
        .execute(generic_update_sql, GeneratedKeys::None)
        .await
        .unwrap();
    assert!(matches!(
        generic_update_results.first(),
        Some(ExecutionResult::UpdateCount(1))
    ));
    assert_eq!(statement.get_update_count().unwrap(), Some(1));

    let partial_sql = "INSERT INTO context_event(id, value) VALUES (12, 'partial')";
    let invalid_batch_sql = "INSERT INTO missing_batch_table VALUES (1)";
    statement.add_batch(partial_sql.to_owned()).unwrap();
    statement.add_batch(invalid_batch_sql.to_owned()).unwrap();
    let batch_error = statement.execute_batch().await.unwrap_err();
    assert_eq!(
        batch_error
            .batch_update_counts()
            .map(druid_rdbc::error::BatchUpdateCounts::large),
        Some([1_i64].as_slice())
    );
    assert!(batch_error.sql_exception().is_some());
    statement.clear_batch().unwrap();

    let batch_after_failure: Arc<dyn StatFilterContextListener> = Arc::new(RecordingListener {
        label: "batch-after-failure",
        events: Arc::clone(&event_log),
        fail_on: Some("execute_after"),
    });
    StatFilterContext::global().add_context_listener(Arc::clone(&batch_after_failure));
    statement
        .add_batch("INSERT INTO context_event(id, value) VALUES (20, 'batch-after-1')".to_owned())
        .unwrap();
    statement
        .add_batch("INSERT INTO context_event(id, value) VALUES (21, 'batch-after-2')".to_owned())
        .unwrap();
    let batch_after_error = statement.execute_batch().await.unwrap_err();
    assert_eq!(
        batch_after_error,
        RdbcError::Other("batch-after-failure failed".to_string())
    );
    assert!(StatFilterContext::global().remove_context_listener(&batch_after_failure));
    statement.clear_batch().unwrap();

    connection.begin().await.unwrap();
    statement
        .execute_update(
            "INSERT INTO context_event(id, value) VALUES (1, 'commit')",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    connection.commit().await.unwrap();

    connection.begin().await.unwrap();
    statement
        .execute_update(
            "INSERT INTO context_event(id, value) VALUES (2, 'rollback')",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    let savepoint = connection
        .set_savepoint_named("before_rollback")
        .await
        .unwrap();
    statement
        .execute_update(
            "UPDATE context_event SET value = 'changed' WHERE id = 2",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    connection.rollback_to(&savepoint).await.unwrap();
    connection.rollback().await.unwrap();

    let invalid_sql = "INSERT INTO missing_context_table VALUES (1)";
    let invalid_error = statement
        .execute_update(invalid_sql, GeneratedKeys::None)
        .await
        .unwrap_err();
    assert!(matches!(
        invalid_error,
        RdbcError::SqlException(_) | RdbcError::DriverError(_)
    ));

    assert!(StatFilterContext::global().remove_context_listener(&listener));
    let recorded = events(&event_log);
    let merged_batch_sql = format!("{batch_sql_1}\n;\n{batch_sql_2}");
    let batch_before = recorded
        .iter()
        .position(|event| event == &format!("sqlite:execute_before:{merged_batch_sql}:false"))
        .unwrap();
    assert_eq!(recorded[batch_before + 1], "sqlite:add_update_count:1");
    assert_eq!(recorded[batch_before + 2], "sqlite:add_update_count:1");
    assert!(recorded[batch_before + 3].starts_with("sqlite:execute_after:null:"));
    assert!(recorded[batch_before + 3].ends_with(":null"));
    let merged_error_batch_sql = format!("{partial_sql}\n;\n{invalid_batch_sql}");
    assert_eq!(
        recorded
            .iter()
            .filter(|event| {
                event.starts_with(&format!("sqlite:execute_before:{merged_error_batch_sql}:"))
            })
            .count(),
        1
    );
    let batch_error_after = recorded
        .iter()
        .find(|event| event.starts_with(&format!("sqlite:execute_after:{merged_error_batch_sql}:")))
        .unwrap();
    assert!(!batch_error_after.ends_with(":null"));
    let prepared_batch_before = recorded
        .iter()
        .position(|event| event == &format!("sqlite:execute_before:{prepared_batch_sql}:false"))
        .unwrap();
    assert_eq!(
        recorded[prepared_batch_before + 1],
        "sqlite:add_update_count:1"
    );
    assert_eq!(
        recorded[prepared_batch_before + 2],
        "sqlite:add_update_count:1"
    );
    assert!(recorded[prepared_batch_before + 3]
        .starts_with(&format!("sqlite:execute_after:{prepared_batch_sql}:")));
    assert!(recorded[prepared_batch_before + 3].ends_with(":null"));
    let generic_query_before = recorded
        .iter()
        .position(|event| event == &format!("sqlite:execute_before:{generic_query_sql}:false"))
        .unwrap();
    assert!(recorded[generic_query_before + 1]
        .starts_with(&format!("sqlite:execute_after:{generic_query_sql}:")));
    assert_eq!(recorded[generic_query_before + 2], "sqlite:result_set_open");
    let generic_update_before = recorded
        .iter()
        .position(|event| event == &format!("sqlite:execute_before:{generic_update_sql}:false"))
        .unwrap();
    assert!(
        recorded[generic_update_before + 1]
            .starts_with(&format!("sqlite:execute_after:{generic_update_sql}:")),
        "Java StatFilter generic execute 更新分支不发送全局 addUpdateCount"
    );
    assert_eq!(collector.execute_batch_count(), 4);
    assert_eq!(collector.execute_batch_size_total(), 6);
    let committed_before = recorded
        .iter()
        .position(|event| {
            event
                == "sqlite:execute_before:INSERT INTO context_event(id, value) VALUES (1, 'commit'):true"
        })
        .unwrap();
    assert_eq!(recorded[committed_before + 1], "sqlite:add_update_count:1");
    assert!(recorded[committed_before + 2].starts_with(
        "sqlite:execute_after:INSERT INTO context_event(id, value) VALUES (1, 'commit'):"
    ));
    assert_eq!(recorded[committed_before + 3], "sqlite:commit");
    assert_eq!(
        recorded
            .iter()
            .filter(|event| event.as_str() == "sqlite:rollback")
            .count(),
        1,
        "savepoint rollback 不得发送全局 rollback"
    );
    let invalid_after = recorded
        .iter()
        .find(|event| event.starts_with(&format!("sqlite:execute_after:{invalid_sql}:")))
        .unwrap();
    assert!(!invalid_after.ends_with(":null"));

    let before_failure: Arc<dyn StatFilterContextListener> = Arc::new(RecordingListener {
        label: "before-failure",
        events: Arc::clone(&event_log),
        fail_on: Some("execute_before"),
    });
    StatFilterContext::global().add_context_listener(Arc::clone(&before_failure));
    let before_error = statement
        .execute_update(
            "INSERT INTO context_event(id, value) VALUES (3, 'blocked')",
            GeneratedKeys::None,
        )
        .await
        .unwrap_err();
    assert!(StatFilterContext::global().remove_context_listener(&before_failure));
    assert_eq!(
        before_error,
        RdbcError::Other("before-failure failed".to_string())
    );

    let after_failure: Arc<dyn StatFilterContextListener> = Arc::new(RecordingListener {
        label: "after-failure",
        events: Arc::clone(&event_log),
        fail_on: Some("execute_after"),
    });
    StatFilterContext::global().add_context_listener(Arc::clone(&after_failure));
    let after_error = statement
        .execute_update(
            "INSERT INTO context_event(id, value) VALUES (4, 'physical-first')",
            GeneratedKeys::None,
        )
        .await
        .unwrap_err();
    assert!(StatFilterContext::global().remove_context_listener(&after_failure));
    assert_eq!(
        after_error,
        RdbcError::Other("after-failure failed".to_string())
    );

    connection.begin().await.unwrap();
    statement
        .execute_update(
            "INSERT INTO context_event(id, value) VALUES (5, 'commit-first')",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    let commit_failure: Arc<dyn StatFilterContextListener> = Arc::new(RecordingListener {
        label: "commit-failure",
        events: Arc::clone(&event_log),
        fail_on: Some("commit"),
    });
    StatFilterContext::global().add_context_listener(Arc::clone(&commit_failure));
    let commit_error = connection.commit().await.unwrap_err();
    assert!(StatFilterContext::global().remove_context_listener(&commit_failure));
    assert_eq!(
        commit_error,
        RdbcError::Other("commit-failure failed".to_string())
    );

    connection.begin().await.unwrap();
    statement
        .execute_update(
            "INSERT INTO context_event(id, value) VALUES (6, 'rollback-first')",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    let rollback_failure: Arc<dyn StatFilterContextListener> = Arc::new(RecordingListener {
        label: "rollback-failure",
        events: Arc::clone(&event_log),
        fail_on: Some("rollback"),
    });
    StatFilterContext::global().add_context_listener(Arc::clone(&rollback_failure));
    let rollback_error = connection.rollback().await.unwrap_err();
    assert!(StatFilterContext::global().remove_context_listener(&rollback_failure));
    assert_eq!(
        rollback_error,
        RdbcError::Other("rollback-failure failed".to_string())
    );

    let mut rows = statement
        .execute_query("SELECT id FROM context_event ORDER BY id")
        .await
        .unwrap();
    let mut ids = Vec::new();
    while rows.next().await.unwrap() {
        ids.push(rows.value(ColumnIndex::new(1).unwrap()).unwrap());
    }
    rows.close().await.unwrap();
    assert_eq!(
        ids,
        [
            druid::core::Value::Int(1),
            druid::core::Value::Int(4),
            druid::core::Value::Int(5),
            druid::core::Value::Int(10),
            druid::core::Value::Int(11),
            druid::core::Value::Int(12),
            druid::core::Value::Int(20),
            druid::core::Value::Int(21),
            druid::core::Value::Int(30),
            druid::core::Value::Int(31),
            druid::core::Value::Int(40)
        ],
        "before 失败不得执行；after/commit 失败发生在物理副作用后；rollback 失败发生在物理回滚后"
    );

    let failing: Arc<dyn StatFilterContextListener> = Arc::new(RecordingListener {
        label: "failing",
        events: Arc::clone(&event_log),
        fail_on: Some("result_set_open"),
    });
    StatFilterContext::global().add_context_listener(Arc::clone(&failing));
    let error = match statement.execute_query("SELECT 4").await {
        Ok(_) => panic!("listener 异常必须中止 ResultSet 构造"),
        Err(error) => error,
    };
    assert!(StatFilterContext::global().remove_context_listener(&failing));
    assert_eq!(error, RdbcError::Other("failing failed".to_string()));
    assert_eq!(collector.result_set_stat().opening_count(), 1);
}
