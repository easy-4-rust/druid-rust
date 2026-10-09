//! Task 5 review C3：canonical standard proxy 必须保留 Druid Filter/Stat/error policy。

mod standard_test_support;

use druid::core::{
    AfterFilter, BatchExecKind, BeforeFilter, ConnectionEvent, ConnectionRecycleDisposition,
    DruidConnectionHolder, DruidError, DruidPooledConnection, ExceptionSorter,
    ExceptionSorterProperties, ExecContext, ExecResult, FilterChain, ResultSetFilter,
    ResultSetFilterContext, StatementEvent,
};
use druid::pool::config::PoolInnerConfig;
use druid::pool::DruidPool;
use druid::sql::{SqlException, TenantCallBack, TenantStatementType, WallConfig, WallFilter};
use druid::stats::StatFilter;
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, ResultSetMetaData, ResultSetMetaDataSnapshot,
};
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{
    ExecutionResult, GeneratedKeys, PreparedStatement, Statement, StatementOptions,
};
use standard_test_support::{TrackingConnection, TrackingFactory, TrackingState};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct RecordingFilter {
    before: AtomicUsize,
    after: AtomicUsize,
    after_error: AtomicUsize,
    connection_before: AtomicUsize,
    connection_after: AtomicUsize,
    result_open: AtomicUsize,
    result_next_before: AtomicUsize,
    result_next_after: AtomicUsize,
    result_close_before: AtomicUsize,
    result_close_after: AtomicUsize,
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedExecutionObservation {
    sql: String,
    params: Vec<druid_rdbc::types::Value>,
    prepared_parameters: Option<Vec<druid::core::PreparedInputParameter>>,
}

#[derive(Default)]
struct PreparedCreationFilter {
    prepare_inputs: Mutex<Vec<String>>,
    statement_events: Mutex<Vec<StatementEvent>>,
    executions: Mutex<Vec<PreparedExecutionObservation>>,
    batches: Mutex<Vec<Vec<Vec<druid_rdbc::types::Value>>>>,
}

#[async_trait::async_trait]
impl BeforeFilter for PreparedCreationFilter {
    fn name(&self) -> &str {
        "task5-prepared-creation"
    }

    fn prepare_statement_sql(&self, sql: &str) -> Result<String, DruidError> {
        self.prepare_inputs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(sql.to_owned());
        Ok(sql.replace("logical", "prepared_rewritten"))
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        self.executions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(PreparedExecutionObservation {
                sql: context.sql.clone(),
                params: context.params.to_vec(),
                prepared_parameters: context.prepared_parameters.map(<[_]>::to_vec),
            });
        if context.prepared_parameters.is_some() {
            context.sql = "execution_rewrite_must_not_reprepare".to_owned();
        }
        Ok(())
    }

    async fn before_batch(
        &self,
        context: &mut druid::core::BatchExecContext<'_>,
    ) -> Result<(), DruidError> {
        self.batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(context.parameter_sets.to_vec());
        Ok(())
    }

    async fn on_statement_event(&self, event: &StatementEvent) -> Result<(), DruidError> {
        self.statement_events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(event.clone());
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for PreparedCreationFilter {
    fn name(&self) -> &str {
        "task5-prepared-creation"
    }

    async fn after(
        &self,
        _context: &ExecContext<'_>,
        _result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        Ok(())
    }
}

impl ResultSetFilter for PreparedCreationFilter {}

struct RecordingTenantCallBack {
    observed: AtomicUsize,
}

#[derive(Default)]
struct StandardGetterFilter {
    value_before: AtomicUsize,
    value_after: AtomicUsize,
    metadata_before: AtomicUsize,
    metadata_after: AtomicUsize,
    was_null_before: AtomicUsize,
    was_null_after: AtomicUsize,
    reject_value: AtomicBool,
}

impl ResultSetFilter for StandardGetterFilter {
    fn standard_result_set_value_before(
        &self,
        _context: &ResultSetFilterContext,
        _column: ColumnIndex,
    ) -> Result<(), DruidError> {
        self.value_before.fetch_add(1, Ordering::AcqRel);
        if self.reject_value.load(Ordering::Acquire) {
            return Err(DruidError::WallViolation(
                "standard value rejected before raw".to_owned(),
            ));
        }
        Ok(())
    }

    fn standard_result_set_value_after(
        &self,
        _context: &ResultSetFilterContext,
        _column: ColumnIndex,
        result: &mut Result<druid_rdbc::types::Value, DruidError>,
    ) -> Result<(), DruidError> {
        self.value_after.fetch_add(1, Ordering::AcqRel);
        if result.is_ok() {
            *result = Ok(druid_rdbc::types::Value::Int(99));
        }
        Ok(())
    }

    fn standard_result_set_meta_data_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.metadata_before.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn standard_result_set_meta_data_after(
        &self,
        _context: &ResultSetFilterContext,
        result: &mut Result<Arc<dyn ResultSetMetaData>, DruidError>,
    ) -> Result<(), DruidError> {
        self.metadata_after.fetch_add(1, Ordering::AcqRel);
        if result.is_ok() {
            *result = Ok(Arc::new(ResultSetMetaDataSnapshot::new(vec![
                ColumnDescriptor::new("filtered", "BIGINT", ColumnNullability::Nullable),
            ])));
        }
        Ok(())
    }

    fn standard_result_set_was_null_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.was_null_before.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn standard_result_set_was_null_after(
        &self,
        _context: &ResultSetFilterContext,
        result: &mut Result<bool, DruidError>,
    ) -> Result<(), DruidError> {
        self.was_null_after.fetch_add(1, Ordering::AcqRel);
        if result.is_ok() {
            *result = Ok(true);
        }
        Ok(())
    }
}

struct RejectResultSetOpenFilter;

impl ResultSetFilter for RejectResultSetOpenFilter {
    fn result_set_open_after(&self, _context: &ResultSetFilterContext) -> Result<(), DruidError> {
        Err(DruidError::WallViolation(
            "task5 rejected result-set open".to_owned(),
        ))
    }
}

#[derive(Default)]
struct RejectSecondResultSetOpenFilter {
    opens: AtomicUsize,
}

impl ResultSetFilter for RejectSecondResultSetOpenFilter {
    fn result_set_open_after(&self, _context: &ResultSetFilterContext) -> Result<(), DruidError> {
        let open = self.opens.fetch_add(1, Ordering::AcqRel) + 1;
        if open == 2 {
            return Err(DruidError::WallViolation(
                "task5 rejected second result-set open".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Default)]
struct StandardBatchObservationFilter {
    add_inputs: Mutex<Vec<String>>,
    before_statements: Mutex<Vec<Vec<String>>>,
    before_kinds: Mutex<Vec<BatchExecKind>>,
    before_count: AtomicUsize,
    after_count: AtomicUsize,
    after_error_count: AtomicUsize,
}

#[async_trait::async_trait]
impl BeforeFilter for StandardBatchObservationFilter {
    fn name(&self) -> &str {
        "task5-standard-batch-observation"
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }

    fn statement_add_batch_sql(&self, sql: &str) -> Result<String, DruidError> {
        self.add_inputs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(sql.to_owned());
        Ok(sql.replace("logical", "batch_rewritten"))
    }

    async fn before_batch(
        &self,
        context: &mut druid::core::BatchExecContext<'_>,
    ) -> Result<(), DruidError> {
        self.before_count.fetch_add(1, Ordering::AcqRel);
        self.before_statements
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(context.statements.to_vec());
        self.before_kinds
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(context.kind);
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for StandardBatchObservationFilter {
    fn name(&self) -> &str {
        "task5-standard-batch-observation"
    }

    async fn after(
        &self,
        _context: &ExecContext<'_>,
        _result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    async fn after_batch(
        &self,
        _context: &druid::core::BatchExecContext<'_>,
        result: &Result<Vec<i32>, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        self.after_count.fetch_add(1, Ordering::AcqRel);
        if result.is_err() {
            self.after_error_count.fetch_add(1, Ordering::AcqRel);
        }
        Ok(())
    }
}

impl ResultSetFilter for StandardBatchObservationFilter {}

impl TenantCallBack for RecordingTenantCallBack {
    fn tenant_value(
        &self,
        _statement_type: TenantStatementType,
        _table_name: &str,
    ) -> Option<druid_rdbc::types::Value> {
        Some(druid_rdbc::types::Value::Int(7))
    }

    fn tenant_column(
        &self,
        _statement_type: TenantStatementType,
        _table_name: &str,
    ) -> Option<String> {
        Some("tenant_id".to_owned())
    }

    fn hidden_column(&self, _table_name: &str) -> Option<String> {
        Some("tenant_id".to_owned())
    }

    fn filter_resultset_tenant_column(&self, value: &druid_rdbc::types::Value) {
        assert_eq!(value, &druid_rdbc::types::Value::Int(7));
        self.observed.fetch_add(1, Ordering::AcqRel);
    }
}

#[async_trait::async_trait]
impl BeforeFilter for RecordingFilter {
    fn name(&self) -> &str {
        "task5-recording"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        self.before.fetch_add(1, Ordering::AcqRel);
        context.sql = context.sql.replace("logical", "filtered");
        Ok(())
    }

    async fn on_connection_event(&self, _event: &ConnectionEvent) -> Result<(), DruidError> {
        self.connection_before.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for RecordingFilter {
    fn name(&self) -> &str {
        "task5-recording"
    }

    async fn after(
        &self,
        _context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        self.after.fetch_add(1, Ordering::AcqRel);
        if result.is_err() {
            self.after_error.fetch_add(1, Ordering::AcqRel);
        }
        Ok(())
    }

    async fn after_connection_event(
        &self,
        _event: &ConnectionEvent,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        self.connection_after.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}

impl ResultSetFilter for RecordingFilter {
    fn result_set_open_after(&self, _context: &ResultSetFilterContext) -> Result<(), DruidError> {
        self.result_open.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn standard_result_set_next_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.result_next_before.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn standard_result_set_next_after(
        &self,
        _context: &ResultSetFilterContext,
        _result: &Result<bool, DruidError>,
    ) -> Result<(), DruidError> {
        self.result_next_after.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn standard_result_set_close_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.result_close_before.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn standard_result_set_close_after(
        &self,
        _context: &ResultSetFilterContext,
        _result: &Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        self.result_close_after.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}

#[tokio::test]
async fn generic_execute_result_set_is_filtered_counted_and_lease_bound() {
    let state = TrackingState::configured();
    let recording = Arc::new(RecordingFilter::default());
    let stat_collector = Arc::new(druid::stats::StatsCollector::new(
        "task5-standard-filter",
        Duration::from_secs(10),
    ));
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::clone(&recording));
    chain.add_filter(Arc::new(StatFilter::new(Arc::clone(&stat_collector))));
    let pool = DruidPool::new_standard(
        "task5-standard-filter".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("standard pool init");

    let mut connection = pool.get().await.expect("standard logical connection");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("standard statement");
    let mut results = Statement::execute(statement.as_mut(), "SELECT logical", GeneratedKeys::None)
        .await
        .expect("generic execute");
    let mut result_set = match results.remove(0) {
        ExecutionResult::ResultSet(result_set) => result_set,
        ExecutionResult::UpdateCount(count) => panic!("expected result set, got {count}"),
    };
    assert!(result_set.next().await.expect("first row"));
    assert_eq!(recording.before.load(Ordering::Acquire), 1);
    assert_eq!(recording.after.load(Ordering::Acquire), 1);
    assert!(state
        .event_snapshot()
        .contains(&"statement.execute:SELECT filtered".to_owned()));
    assert_eq!(
        pool.stats_collector().execute_count.load(Ordering::Acquire),
        1
    );
    assert_eq!(pool.stats_collector().statement_stat().execute_count(), 1);
    assert_eq!(stat_collector.result_set_stat().open_count(), 1);

    connection.close().await.expect("connection-first close");
    assert!(
        result_set.next().await.is_err(),
        "old cursor crossed leases"
    );
    result_set
        .close()
        .await
        .expect("stale result close is idempotent");
    assert_eq!(state.result_set_next_count.load(Ordering::Acquire), 1);
    assert_eq!(state.result_set_close_count.load(Ordering::Acquire), 1);
    assert_eq!(stat_collector.result_set_stat().close_count(), 1);
    assert_eq!(stat_collector.result_set_stat().fetch_row_count(), 1);

    pool.close().await;
}

#[tokio::test]
async fn wall_rejection_prevents_standard_raw_execution() {
    let state = TrackingState::configured();
    let mut config = WallConfig::default();
    config.update_allow = false;
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(WallFilter::with_config(config)));
    let pool = DruidPool::new_standard(
        "task5-standard-wall".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("standard pool init");
    let mut connection = pool.get().await.expect("standard logical connection");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("standard statement");

    let error = Statement::execute_update(
        statement.as_mut(),
        "UPDATE blocked SET value = 1",
        GeneratedKeys::None,
    )
    .await
    .expect_err("WallFilter must reject before the raw actor");
    assert!(matches!(error, RdbcError::WallViolation(_)));
    assert!(!state
        .event_snapshot()
        .iter()
        .any(|event| event.starts_with("statement.update:")));

    Statement::close(statement.as_mut())
        .await
        .expect("statement close");
    connection.close().await.expect("logical close");
    pool.close().await;
}

#[tokio::test]
async fn statement_prepared_callable_and_result_sets_share_one_filter_stat_path() {
    let state = TrackingState::configured();
    let recording = Arc::new(RecordingFilter::default());
    let stat_collector = Arc::new(druid::stats::StatsCollector::new(
        "task5-standard-families",
        Duration::from_secs(10),
    ));
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::clone(&recording));
    chain.add_filter(Arc::new(StatFilter::new(Arc::clone(&stat_collector))));
    let pool = DruidPool::new_standard(
        "task5-standard-families".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("standard pool init");
    let mut connection = pool.get().await.expect("standard logical connection");

    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("statement");
    let mut statement_result = Statement::execute_query(statement.as_mut(), "SELECT logical")
        .await
        .expect("statement query");
    assert!(statement_result.next().await.expect("statement row"));
    statement_result
        .close()
        .await
        .expect("statement result close");

    let mut prepared = Connection::prepare_statement(
        &mut connection,
        "SELECT logical prepared",
        StatementOptions::default(),
    )
    .await
    .expect("prepared");
    let mut prepared_result = PreparedStatement::execute_prepared_query(prepared.as_mut())
        .await
        .expect("prepared query");
    assert!(prepared_result.next().await.expect("prepared row"));
    prepared_result
        .close()
        .await
        .expect("prepared result close");

    let mut callable = Connection::prepare_call(
        &mut connection,
        "CALL logical_proc()",
        StatementOptions::default(),
    )
    .await
    .expect("callable");
    let mut callable_result = PreparedStatement::execute_prepared_query(callable.as_mut())
        .await
        .expect("callable query");
    assert!(callable_result.next().await.expect("callable row"));
    callable_result
        .close()
        .await
        .expect("callable result close");

    assert_eq!(recording.before.load(Ordering::Acquire), 3);
    assert_eq!(recording.after.load(Ordering::Acquire), 3);
    assert_eq!(recording.after_error.load(Ordering::Acquire), 0);
    assert_eq!(recording.result_open.load(Ordering::Acquire), 3);
    assert_eq!(recording.result_next_before.load(Ordering::Acquire), 3);
    assert_eq!(recording.result_next_after.load(Ordering::Acquire), 3);
    assert_eq!(recording.result_close_before.load(Ordering::Acquire), 3);
    assert_eq!(recording.result_close_after.load(Ordering::Acquire), 3);
    assert_eq!(
        pool.stats_collector().execute_count.load(Ordering::Acquire),
        3
    );
    assert_eq!(pool.stats_collector().statement_stat().execute_count(), 3);
    assert_eq!(stat_collector.result_set_stat().open_count(), 3);
    assert_eq!(stat_collector.result_set_stat().close_count(), 3);
    assert_eq!(stat_collector.result_set_stat().fetch_row_count(), 3);
    assert_eq!(state.result_set_next_count.load(Ordering::Acquire), 3);
    assert_eq!(state.result_set_close_count.load(Ordering::Acquire), 3);

    Statement::close(statement.as_mut())
        .await
        .expect("statement close");
    Statement::close(prepared.as_mut())
        .await
        .expect("prepared close");
    Statement::close(callable.as_mut())
        .await
        .expect("callable close");
    connection.close().await.expect("logical close");
    pool.close().await;
}

#[tokio::test]
async fn fatal_standard_error_runs_after_filter_and_forces_discard() {
    let state = TrackingState::configured();
    state.fail_next("statement_execute_update");
    let recording = Arc::new(RecordingFilter::default());
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::clone(&recording));
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(TrackingConnection::new(Arc::clone(&state))),
        901,
        Duration::ZERO,
        0,
    )
    .await
    .expect("standard holder");
    let returned = Arc::new(Mutex::new(None));
    let returned_observer = Arc::clone(&returned);
    let mut connection = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-standard-fatal".to_owned(),
        Some(Arc::new(chain)),
        false,
        false,
        Box::new(move |holder, disposition| {
            *returned_observer
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((holder, disposition));
            false
        }),
    );
    connection.set_exception_sorter(Arc::new(AllFatalSorter));
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("statement");

    let error = Statement::execute_update(
        statement.as_mut(),
        "UPDATE fatal SET value = 1",
        GeneratedKeys::None,
    )
    .await
    .expect_err("raw SQL failure");
    assert!(matches!(error, RdbcError::SqlException(_)));
    assert_eq!(recording.before.load(Ordering::Acquire), 1);
    assert_eq!(recording.after.load(Ordering::Acquire), 1);
    assert_eq!(recording.after_error.load(Ordering::Acquire), 1);

    connection.close().await.expect("logical discard close");
    let (mut holder, disposition) = returned
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .expect("owner receives discarded holder");
    assert!(matches!(
        disposition,
        ConnectionRecycleDisposition::Discard { .. }
    ));
    holder
        .close_standard_connection()
        .await
        .expect("discard owner closes raw");
}

#[tokio::test]
async fn standard_commit_and_rollback_preserve_filter_and_transaction_stats() {
    let state = TrackingState::configured();
    let recording = Arc::new(RecordingFilter::default());
    let stat_collector = Arc::new(druid::stats::StatsCollector::new(
        "task5-standard-transactions",
        Duration::from_secs(10),
    ));
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::clone(&recording));
    chain.add_filter(Arc::new(StatFilter::new(Arc::clone(&stat_collector))));
    let pool = DruidPool::new_standard(
        "task5-standard-transactions".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("standard pool init");
    let mut connection = pool.get().await.expect("standard logical connection");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("statement");

    connection
        .set_auto_commit(false)
        .await
        .expect("begin commit transaction");
    Statement::execute_update(
        statement.as_mut(),
        "UPDATE commit_path SET value = 1",
        GeneratedKeys::None,
    )
    .await
    .expect("transaction update");
    connection.commit().await.expect("commit");

    connection
        .set_auto_commit(false)
        .await
        .expect("begin rollback transaction");
    Statement::execute_update(
        statement.as_mut(),
        "UPDATE rollback_path SET value = 1",
        GeneratedKeys::None,
    )
    .await
    .expect("transaction update");
    connection.rollback().await.expect("rollback");

    assert_eq!(recording.connection_before.load(Ordering::Acquire), 4);
    assert_eq!(recording.connection_after.load(Ordering::Acquire), 4);
    assert_eq!(recording.before.load(Ordering::Acquire), 2);
    assert_eq!(recording.after.load(Ordering::Acquire), 2);
    assert_eq!(
        pool.stats_collector()
            .start_transaction_count
            .load(Ordering::Acquire),
        2
    );
    assert_eq!(
        pool.stats_collector().commit_count.load(Ordering::Acquire),
        1
    );
    assert_eq!(
        pool.stats_collector()
            .rollback_count
            .load(Ordering::Acquire),
        1
    );

    Statement::close(statement.as_mut())
        .await
        .expect("statement close");
    connection.close().await.expect("logical close");
    pool.close().await;
}

#[tokio::test]
async fn canonical_prepare_rewrite_cache_parameters_batch_events_and_timeout_match_raw() {
    let state = TrackingState::configured();
    let filter = Arc::new(PreparedCreationFilter::default());
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::clone(&filter));
    let pool = DruidPool::new_standard(
        "task5-standard-prepare-policy".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            pool_prepared_statements: true,
            max_pool_prepared_statements_per_connection: 4,
            query_timeout: 13,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("standard pool init");
    let mut connection = pool.get().await.expect("standard connection");
    let mut created = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("created statement");
    assert_eq!(Statement::query_timeout(created.as_ref()).unwrap(), 13);
    Statement::close(created.as_mut())
        .await
        .expect("created close");

    let mut prepared = Connection::prepare_statement(
        &mut connection,
        "SELECT logical ?, ?",
        StatementOptions::default(),
    )
    .await
    .expect("rewritten prepared");
    assert_eq!(Statement::query_timeout(prepared.as_ref()).unwrap(), 13);
    PreparedStatement::set_parameter(
        prepared.as_mut(),
        druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(2).expect("index 2"),
            value: druid_rdbc::types::Value::Int(22),
        },
    )
    .expect("set index 2");
    PreparedStatement::set_parameter(
        prepared.as_mut(),
        druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).expect("index 1"),
            value: druid_rdbc::types::Value::Int(11),
        },
    )
    .expect("set index 1");
    let mut result = PreparedStatement::execute_prepared_query(prepared.as_mut())
        .await
        .expect("prepared query");
    result.close().await.expect("result close");

    Statement::add_batch(prepared.as_mut(), "bound-1".to_owned()).expect("first batch");
    PreparedStatement::set_parameter(
        prepared.as_mut(),
        druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).expect("index 1"),
            value: druid_rdbc::types::Value::Int(33),
        },
    )
    .expect("replace index 1");
    Statement::add_batch(prepared.as_mut(), "bound-2".to_owned()).expect("second batch");
    assert_eq!(
        Statement::execute_batch(prepared.as_mut())
            .await
            .expect("prepared batch"),
        vec![1, 1]
    );
    Statement::close(prepared.as_mut())
        .await
        .expect("prepared cache return");

    let mut cached = Connection::prepare_statement(
        &mut connection,
        "SELECT prepared_rewritten ?, ?",
        StatementOptions::default(),
    )
    .await
    .expect("same effective cache key");
    assert_eq!(Statement::query_timeout(cached.as_ref()).unwrap(), 13);
    Statement::close(cached.as_mut())
        .await
        .expect("cached close");
    let mut callable = Connection::prepare_call(
        &mut connection,
        "CALL logical_proc(?)",
        StatementOptions::default(),
    )
    .await
    .expect("rewritten callable");
    assert_eq!(Statement::query_timeout(callable.as_ref()).unwrap(), 13);
    PreparedStatement::set_parameter(
        callable.as_mut(),
        druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).expect("index 1"),
            value: druid_rdbc::types::Value::Int(44),
        },
    )
    .expect("callable parameter");
    PreparedStatement::execute_prepared_update(callable.as_mut())
        .await
        .expect("callable update");
    Statement::close(callable.as_mut())
        .await
        .expect("callable close");

    let events = state.event_snapshot();
    assert_eq!(
        events
            .iter()
            .filter(|event| event.as_str() == "connection.prepare:SELECT prepared_rewritten ?, ?")
            .count(),
        1,
        "cache key and raw SQL must use the same effective text"
    );
    assert!(events
        .iter()
        .any(|event| event == "connection.prepare_call:CALL prepared_rewritten_proc(?)"));
    let raw_parameters = state
        .prepared_execution_parameters
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(
        raw_parameters[0][0].value,
        druid_rdbc::types::Value::Int(11)
    );
    assert_eq!(
        raw_parameters[0][1].value,
        druid_rdbc::types::Value::Int(22)
    );
    let observations = filter
        .executions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(observations[0].sql, "SELECT prepared_rewritten ?, ?");
    assert_eq!(
        observations[0].params,
        vec![
            druid_rdbc::types::Value::Int(11),
            druid_rdbc::types::Value::Int(22),
        ]
    );
    assert_eq!(
        filter
            .batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[0],
        vec![
            vec![
                druid_rdbc::types::Value::Int(11),
                druid_rdbc::types::Value::Int(22)
            ],
            vec![
                druid_rdbc::types::Value::Int(33),
                druid_rdbc::types::Value::Int(22)
            ],
        ]
    );
    let statement_events = filter
        .statement_events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(
        statement_events
            .iter()
            .filter(|event| matches!(event, StatementEvent::CreateStatement))
            .count(),
        1
    );
    assert_eq!(
        statement_events
            .iter()
            .filter(|event| matches!(event, StatementEvent::PrepareStatement(_)))
            .count(),
        2
    );
    assert_eq!(
        statement_events
            .iter()
            .filter(|event| matches!(event, StatementEvent::PrepareCall(_)))
            .count(),
        1
    );
    assert_eq!(pool.stats_collector().statement_stat().create_count(), 1);
    assert_eq!(pool.stats_collector().statement_stat().prepare_count(), 2);
    assert_eq!(
        pool.stats_collector().statement_stat().prepare_call_count(),
        1
    );
    assert_eq!(
        *state
            .statement_query_timeouts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        // 两次 Prepared 与一次 Callable 关闭均恢复 raw 初始 timeout 为0；重借时应用池配置13。
        vec![13, 13, 0, 13, 0, 13, 0]
    );

    connection.close().await.expect("logical close");
    pool.close().await;
}

#[tokio::test]
async fn canonical_prepared_setter_failure_and_clear_keep_filter_snapshot_truthful() {
    let state = TrackingState::configured();
    let filter = Arc::new(PreparedCreationFilter::default());
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::clone(&filter));
    let pool = DruidPool::new_standard(
        "task5-standard-parameter-state".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig::default(),
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    let mut prepared = Connection::prepare_statement(
        &mut connection,
        "SELECT logical ?",
        StatementOptions::default(),
    )
    .await
    .expect("prepared");
    PreparedStatement::set_parameter(
        prepared.as_mut(),
        druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).expect("index"),
            value: druid_rdbc::types::Value::Int(10),
        },
    )
    .expect("first setter");
    state.fail_next("prepared_set_parameter");
    assert!(PreparedStatement::set_parameter(
        prepared.as_mut(),
        druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).expect("index"),
            value: druid_rdbc::types::Value::Int(99),
        },
    )
    .is_err());
    PreparedStatement::execute_prepared_update(prepared.as_mut())
        .await
        .expect("execute retains successful setter");
    PreparedStatement::clear_parameters(prepared.as_mut()).expect("clear parameters");
    PreparedStatement::execute_prepared_update(prepared.as_mut())
        .await
        .expect("execute after clear");
    let observations = filter
        .executions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(
        observations[0].params,
        vec![druid_rdbc::types::Value::Int(10)]
    );
    assert!(observations[1].params.is_empty());
    let raw = state
        .prepared_execution_parameters
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(raw[0][0].value, druid_rdbc::types::Value::Int(10));
    assert!(raw[1].is_empty());
    Statement::close(prepared.as_mut()).await.expect("close");
    connection.close().await.expect("logical close");
    pool.close().await;
}

#[tokio::test]
async fn canonical_wall_prepare_and_metadata_rejection_prevent_raw_calls() {
    let state = TrackingState::configured();
    let mut config = WallConfig::default();
    config.deny_tables.push("blocked".to_owned());
    config.metadata_allow = false;
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(WallFilter::with_config(config)));
    let pool = DruidPool::new_standard(
        "task5-standard-wall-create".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig::default(),
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    assert!(matches!(
        Connection::prepare_statement(
            &mut connection,
            "SELECT * FROM blocked",
            StatementOptions::default(),
        )
        .await,
        Err(RdbcError::WallViolation(_))
    ));
    assert!(matches!(
        Connection::database_meta_data(&mut connection).await,
        Err(RdbcError::WallViolation(_))
    ));
    let events = state.event_snapshot();
    assert!(!events
        .iter()
        .any(|event| event.starts_with("connection.prepare:")));
    assert!(!events.iter().any(|event| event == "connection.metadata"));
    connection.close().await.expect("logical close");
    pool.close().await;
}

#[tokio::test]
async fn canonical_result_set_open_initializes_wall_tenant_hidden_mapping() {
    let state = TrackingState::configured();
    let tenant = Arc::new(RecordingTenantCallBack {
        observed: AtomicUsize::new(0),
    });
    let config = WallConfig::builder()
        .tenant_column("tenant_id")
        .tenant_call_back(tenant.clone())
        .build();
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(WallFilter::with_config(config)));
    let pool = DruidPool::new_standard(
        "task5-standard-wall-result".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig::default(),
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("statement");
    let mut result = Statement::execute_query(statement.as_mut(), "SELECT tenant_projection")
        .await
        .expect("tenant result");
    assert!(result.next().await.expect("row"));
    assert_eq!(tenant.observed.load(Ordering::Acquire), 1);
    assert_eq!(
        result
            .value(ColumnIndex::new(2).expect("logical column 2"))
            .expect("mapped value"),
        druid_rdbc::types::Value::Int(33)
    );
    result.close().await.expect("result close");
    Statement::close(statement.as_mut())
        .await
        .expect("statement close");
    connection.close().await.expect("logical close");
    pool.close().await;
}

#[tokio::test]
async fn canonical_result_set_open_rejection_closes_unregistered_raw_once() {
    let state = TrackingState::configured();
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::new(RejectResultSetOpenFilter));
    let pool = DruidPool::new_standard(
        "task5-standard-open-reject".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("statement");
    assert!(matches!(
        Statement::execute_query(statement.as_mut(), "SELECT rejected_open").await,
        Err(RdbcError::WallViolation(_))
    ));
    assert_eq!(state.result_set_close_count.load(Ordering::Acquire), 1);
    Statement::close(statement.as_mut())
        .await
        .expect("statement close");
    connection.close().await.expect("connection close");
    pool.close().await;
}

#[tokio::test]
async fn canonical_result_set_value_metadata_was_null_filters_modify_and_reject_once() {
    let state = TrackingState::configured();
    let filter = Arc::new(StandardGetterFilter::default());
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::clone(&filter) as Arc<dyn ResultSetFilter>);
    let pool = DruidPool::new_standard(
        "task5-standard-result-getters".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig::default(),
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("statement");
    let mut result = Statement::execute_query(statement.as_mut(), "SELECT getter_filters")
        .await
        .expect("result");
    assert!(result.next().await.expect("row"));
    assert_eq!(
        result
            .value(ColumnIndex::new(1).expect("column"))
            .expect("modified value"),
        druid_rdbc::types::Value::Int(99)
    );
    let metadata = result.meta_data().expect("modified metadata");
    assert_eq!(
        metadata
            .column(ColumnIndex::new(1).expect("column"))
            .expect("descriptor")
            .label(),
        "filtered"
    );
    assert!(result.was_null().expect("modified was-null"));
    assert_eq!(filter.value_before.load(Ordering::Acquire), 1);
    assert_eq!(filter.value_after.load(Ordering::Acquire), 1);
    assert_eq!(filter.metadata_before.load(Ordering::Acquire), 1);
    assert_eq!(filter.metadata_after.load(Ordering::Acquire), 1);
    assert_eq!(filter.was_null_before.load(Ordering::Acquire), 1);
    assert_eq!(filter.was_null_after.load(Ordering::Acquire), 1);
    assert_eq!(state.result_set_value_count.load(Ordering::Acquire), 1);
    // Proxy-aware open reads raw metadata once; the explicit getter reads it once more.
    assert_eq!(state.result_set_meta_data_count.load(Ordering::Acquire), 2);
    assert_eq!(state.result_set_was_null_count.load(Ordering::Acquire), 1);

    filter.reject_value.store(true, Ordering::Release);
    assert!(matches!(
        result.value(ColumnIndex::new(1).expect("column")),
        Err(RdbcError::WallViolation(_))
    ));
    assert_eq!(filter.value_before.load(Ordering::Acquire), 2);
    assert_eq!(filter.value_after.load(Ordering::Acquire), 1);
    assert_eq!(state.result_set_value_count.load(Ordering::Acquire), 1);
    result.close().await.expect("result close");
    Statement::close(statement.as_mut())
        .await
        .expect("statement close");
    connection.close().await.expect("logical close");
    pool.close().await;
}

#[tokio::test]
async fn canonical_prepared_generic_execute_wraps_registered_result_with_typed_params() {
    let state = TrackingState::configured();
    let prepared_filter = Arc::new(PreparedCreationFilter::default());
    let recording = Arc::new(RecordingFilter::default());
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::clone(&prepared_filter));
    chain.add_filter(Arc::clone(&recording));
    let pool = DruidPool::new_standard(
        "task5-prepared-generic".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig::default(),
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    let mut prepared = Connection::prepare_statement(
        &mut connection,
        "SELECT logical ?",
        StatementOptions::default(),
    )
    .await
    .expect("prepared");
    PreparedStatement::set_parameter(
        prepared.as_mut(),
        druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).expect("index"),
            value: druid_rdbc::types::Value::Int(71),
        },
    )
    .expect("parameter");
    let mut results = PreparedStatement::execute_prepared(prepared.as_mut())
        .await
        .expect("generic execute");
    let mut result = match results.remove(0) {
        ExecutionResult::ResultSet(result) => result,
        ExecutionResult::UpdateCount(count) => panic!("expected ResultSet, got {count}"),
    };
    let observations = prepared_filter
        .executions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].sql, "SELECT prepared_rewritten ?");
    assert_eq!(
        observations[0].params,
        vec![druid_rdbc::types::Value::Int(71)]
    );
    assert_eq!(recording.before.load(Ordering::Acquire), 1);
    assert_eq!(recording.after.load(Ordering::Acquire), 1);
    assert_eq!(recording.result_open.load(Ordering::Acquire), 1);
    assert_eq!(pool.stats_collector().statement_stat().execute_count(), 1);
    assert!(result.next().await.expect("first row"));
    connection.close().await.expect("logical close");
    assert!(result.next().await.is_err(), "generic cursor crossed lease");
    assert_eq!(state.result_set_close_count.load(Ordering::Acquire), 1);
    pool.close().await;
}

#[tokio::test]
async fn canonical_callable_generic_execute_uses_same_typed_policy_and_wraps_result() {
    let state = TrackingState::configured();
    let prepared_filter = Arc::new(PreparedCreationFilter::default());
    let recording = Arc::new(RecordingFilter::default());
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::clone(&prepared_filter));
    chain.add_filter(Arc::clone(&recording));
    let pool = DruidPool::new_standard(
        "task5-callable-generic".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig::default(),
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    let mut callable = Connection::prepare_call(
        &mut connection,
        "CALL logical_proc(?)",
        StatementOptions::default(),
    )
    .await
    .expect("callable");
    PreparedStatement::set_parameter(
        callable.as_mut(),
        druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).expect("index"),
            value: druid_rdbc::types::Value::Int(81),
        },
    )
    .expect("parameter");
    let mut results = PreparedStatement::execute_prepared(callable.as_mut())
        .await
        .expect("generic callable execute");
    let mut result = match results.remove(0) {
        ExecutionResult::ResultSet(result) => result,
        ExecutionResult::UpdateCount(count) => panic!("expected ResultSet, got {count}"),
    };
    let observations = prepared_filter
        .executions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].sql, "CALL prepared_rewritten_proc(?)");
    assert_eq!(
        observations[0].params,
        vec![druid_rdbc::types::Value::Int(81)]
    );
    assert_eq!(recording.before.load(Ordering::Acquire), 1);
    assert_eq!(recording.after.load(Ordering::Acquire), 1);
    assert_eq!(recording.result_open.load(Ordering::Acquire), 1);
    assert!(result.next().await.expect("first row"));
    connection.close().await.expect("logical close");
    assert!(
        result.next().await.is_err(),
        "callable cursor crossed lease"
    );
    assert_eq!(state.result_set_close_count.load(Ordering::Acquire), 1);
    pool.close().await;
}

#[tokio::test]
async fn canonical_prepared_multi_result_open_failure_closes_every_raw_once() {
    let state = TrackingState::configured();
    let reject = Arc::new(RejectSecondResultSetOpenFilter::default());
    let stat_collector = Arc::new(druid::stats::StatsCollector::new(
        "task5-prepared-multi-result-cleanup",
        Duration::from_secs(10),
    ));
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(StatFilter::new(Arc::clone(&stat_collector))));
    chain.add_result_set(Arc::clone(&reject) as Arc<dyn ResultSetFilter>);
    let pool = DruidPool::new_standard(
        "task5-prepared-multi-result-cleanup".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    let mut prepared = Connection::prepare_statement(
        &mut connection,
        "SELECT multi_result",
        StatementOptions::default(),
    )
    .await
    .expect("prepared");

    assert!(matches!(
        PreparedStatement::execute_prepared(prepared.as_mut()).await,
        Err(RdbcError::WallViolation(_))
    ));
    assert_eq!(reject.opens.load(Ordering::Acquire), 2);
    assert_eq!(
        state.result_set_close_attempt_count.load(Ordering::Acquire),
        3
    );
    assert_eq!(state.result_set_close_count.load(Ordering::Acquire), 3);
    assert_eq!(stat_collector.result_set_stat().open_count(), 1);
    assert_eq!(stat_collector.result_set_stat().close_count(), 1);

    connection
        .close()
        .await
        .expect("cleanup leaves no registry entry");
    assert_eq!(
        state.result_set_close_attempt_count.load(Ordering::Acquire),
        3
    );
    let mut replacement = pool.get().await.expect("next lease");
    replacement.close().await.expect("replacement close");
    pool.close().await;
}

#[tokio::test]
async fn canonical_multi_result_cleanup_failure_preserves_primary_and_retries() {
    let state = TrackingState::configured();
    let reject = Arc::new(RejectSecondResultSetOpenFilter::default());
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::clone(&reject) as Arc<dyn ResultSetFilter>);
    let pool = DruidPool::new_standard(
        "task5-multi-result-cleanup-retry".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    let mut prepared = Connection::prepare_statement(
        &mut connection,
        "SELECT multi_result",
        StatementOptions::default(),
    )
    .await
    .expect("prepared");
    state.fail_next("result_set_close");

    assert!(matches!(
        PreparedStatement::execute_prepared(prepared.as_mut()).await,
        Err(RdbcError::WallViolation(_))
    ));
    assert_eq!(
        state.result_set_close_attempt_count.load(Ordering::Acquire),
        3
    );
    assert_eq!(state.result_set_close_count.load(Ordering::Acquire), 2);

    connection
        .close()
        .await
        .expect("logical close retries retained cleanup");
    assert_eq!(
        state.result_set_close_attempt_count.load(Ordering::Acquire),
        4
    );
    assert_eq!(state.result_set_close_count.load(Ordering::Acquire), 3);
    pool.close().await;
}

#[tokio::test]
async fn canonical_callable_multi_result_open_failure_closes_every_raw_once() {
    let state = TrackingState::configured();
    let reject = Arc::new(RejectSecondResultSetOpenFilter::default());
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::clone(&reject) as Arc<dyn ResultSetFilter>);
    let pool = DruidPool::new_standard(
        "task5-callable-multi-result-cleanup".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    let mut callable = Connection::prepare_call(
        &mut connection,
        "CALL multi_result()",
        StatementOptions::default(),
    )
    .await
    .expect("callable");

    assert!(matches!(
        PreparedStatement::execute_prepared(callable.as_mut()).await,
        Err(RdbcError::WallViolation(_))
    ));
    assert_eq!(reject.opens.load(Ordering::Acquire), 2);
    assert_eq!(
        state.result_set_close_attempt_count.load(Ordering::Acquire),
        3
    );
    assert_eq!(state.result_set_close_count.load(Ordering::Acquire), 3);

    connection
        .close()
        .await
        .expect("cleanup leaves no registry entry");
    assert_eq!(
        state.result_set_close_attempt_count.load(Ordering::Acquire),
        3
    );
    let mut replacement = pool.get().await.expect("next lease");
    replacement.close().await.expect("replacement close");
    pool.close().await;
}

#[tokio::test]
async fn canonical_statement_multi_result_open_failure_closes_every_raw_once() {
    let state = TrackingState::configured();
    let reject = Arc::new(RejectSecondResultSetOpenFilter::default());
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::clone(&reject) as Arc<dyn ResultSetFilter>);
    let pool = DruidPool::new_standard(
        "task5-statement-multi-result-cleanup".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("statement");

    assert!(matches!(
        Statement::execute(
            statement.as_mut(),
            "SELECT multi_result",
            GeneratedKeys::None,
        )
        .await,
        Err(RdbcError::WallViolation(_))
    ));
    assert_eq!(reject.opens.load(Ordering::Acquire), 2);
    assert_eq!(
        state.result_set_close_attempt_count.load(Ordering::Acquire),
        3
    );
    assert_eq!(state.result_set_close_count.load(Ordering::Acquire), 3);

    connection
        .close()
        .await
        .expect("cleanup leaves no registry entry");
    assert_eq!(
        state.result_set_close_attempt_count.load(Ordering::Acquire),
        3
    );
    let mut replacement = pool.get().await.expect("next lease");
    replacement.close().await.expect("replacement close");
    pool.close().await;
}

#[tokio::test]
async fn canonical_prepared_generic_fatal_error_runs_after_once_and_discards() {
    let state = TrackingState::configured();
    let recording = Arc::new(RecordingFilter::default());
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::clone(&recording));
    let pool = DruidPool::new_standard(
        "task5-prepared-generic-fatal".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    connection.set_exception_sorter(Arc::new(AllFatalSorter));
    let mut prepared = Connection::prepare_statement(
        &mut connection,
        "SELECT fatal_generic ?",
        StatementOptions::default(),
    )
    .await
    .expect("prepared");
    PreparedStatement::set_parameter(
        prepared.as_mut(),
        druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).expect("index"),
            value: druid_rdbc::types::Value::Int(91),
        },
    )
    .expect("parameter");
    state.fail_next("prepared_execute_generic");
    assert!(matches!(
        PreparedStatement::execute_prepared(prepared.as_mut()).await,
        Err(RdbcError::SqlException(_))
    ));
    assert_eq!(recording.before.load(Ordering::Acquire), 1);
    assert_eq!(recording.after.load(Ordering::Acquire), 1);
    assert_eq!(recording.after_error.load(Ordering::Acquire), 1);
    assert_eq!(pool.stats_collector().statement_stat().execute_count(), 1);
    connection.close().await.expect("fatal close");
    let mut replacement = pool.get().await.expect("replacement");
    assert_eq!(state.factory_create_count.load(Ordering::Acquire), 2);
    replacement.close().await.expect("replacement close");
    pool.close().await;
}

#[tokio::test]
async fn canonical_statement_batch_rewrites_orders_clears_and_filters_once() {
    let state = TrackingState::configured();
    let batch_filter = Arc::new(StandardBatchObservationFilter::default());
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::clone(&batch_filter));
    let pool = DruidPool::new_standard(
        "task5-statement-batch".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig::default(),
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("statement");
    Statement::add_batch(statement.as_mut(), "UPDATE logical_one SET v=1".to_owned())
        .expect("first add");
    Statement::add_batch(statement.as_mut(), "UPDATE logical_two SET v=2".to_owned())
        .expect("second add");
    assert_eq!(
        Statement::execute_batch(statement.as_mut())
            .await
            .expect("execute batch"),
        vec![1, 1]
    );
    assert_eq!(batch_filter.before_count.load(Ordering::Acquire), 1);
    assert_eq!(batch_filter.after_count.load(Ordering::Acquire), 1);
    assert_eq!(
        batch_filter
            .before_statements
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[0],
        vec![
            "UPDATE batch_rewritten_one SET v=1".to_owned(),
            "UPDATE batch_rewritten_two SET v=2".to_owned(),
        ]
    );
    assert_eq!(
        batch_filter
            .before_kinds
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[0],
        BatchExecKind::Statement
    );
    assert_eq!(
        state
            .statement_batch_executions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[0],
        vec![
            "UPDATE batch_rewritten_one SET v=1".to_owned(),
            "UPDATE batch_rewritten_two SET v=2".to_owned(),
        ]
    );

    Statement::add_batch(
        statement.as_mut(),
        "UPDATE logical_three SET v=3".to_owned(),
    )
    .expect("third add");
    Statement::clear_batch(statement.as_mut()).expect("clear batch");
    assert!(Statement::execute_batch(statement.as_mut())
        .await
        .expect("empty batch")
        .is_empty());
    assert_eq!(state.statement_batch_clear_count.load(Ordering::Acquire), 1);
    assert_eq!(batch_filter.before_count.load(Ordering::Acquire), 2);
    assert_eq!(batch_filter.after_count.load(Ordering::Acquire), 2);
    assert_eq!(pool.stats_collector().statement_stat().execute_count(), 2);
    Statement::close(statement.as_mut())
        .await
        .expect("statement close");
    connection.close().await.expect("connection close");
    pool.close().await;
}

#[tokio::test]
async fn canonical_statement_batch_denial_and_partial_fatal_error_are_classified_once() {
    let state = TrackingState::configured();
    let batch_filter = Arc::new(StandardBatchObservationFilter::default());
    let mut wall_config = WallConfig::default();
    wall_config.deny_tables.push("blocked".to_owned());
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::clone(&batch_filter));
    chain.add_filter(Arc::new(WallFilter::with_config(wall_config)));
    let pool = DruidPool::new_standard(
        "task5-statement-batch-error".to_owned(),
        "tracking".to_owned(),
        Arc::new(TrackingFactory::new(Arc::clone(&state))),
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_secs(2),
            ..PoolInnerConfig::default()
        },
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    connection.set_exception_sorter(Arc::new(AllFatalSorter));
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("statement");
    Statement::add_batch(statement.as_mut(), "UPDATE logical_ok SET v=1".to_owned())
        .expect("allowed add");
    assert!(matches!(
        Statement::add_batch(
            statement.as_mut(),
            "DELETE FROM blocked WHERE id=1".to_owned()
        ),
        Err(RdbcError::WallViolation(_))
    ));
    assert_eq!(
        state
            .statement_batch_add_attempt_count
            .load(Ordering::Acquire),
        1,
        "Wall denial must precede raw add"
    );
    Statement::add_batch(
        statement.as_mut(),
        "UPDATE logical_after SET v=2".to_owned(),
    )
    .expect("second allowed add");
    state.fail_next("statement_execute_batch_partial");
    let error = Statement::execute_batch(statement.as_mut())
        .await
        .expect_err("partial fatal batch");
    assert_eq!(
        error.batch_update_counts().expect("partial counts").large(),
        &[1]
    );
    assert_eq!(batch_filter.before_count.load(Ordering::Acquire), 1);
    assert_eq!(batch_filter.after_count.load(Ordering::Acquire), 1);
    assert_eq!(batch_filter.after_error_count.load(Ordering::Acquire), 1);
    assert_eq!(
        state.statement_batch_execute_count.load(Ordering::Acquire),
        1
    );
    connection.close().await.expect("fatal close");
    let mut replacement = pool.get().await.expect("replacement connection");
    assert_eq!(state.factory_create_count.load(Ordering::Acquire), 2);
    replacement.close().await.expect("replacement close");
    pool.close().await;
}

#[tokio::test]
async fn fatal_standard_create_prepare_call_and_metadata_errors_force_discard() {
    for operation in [
        "create_statement",
        "prepare_statement",
        "prepare_call",
        "database_meta_data",
    ] {
        let state = TrackingState::configured();
        state.fail_next(operation);
        let holder = DruidConnectionHolder::with_standard_connection(
            Box::new(TrackingConnection::new(Arc::clone(&state))),
            950,
            Duration::ZERO,
            0,
        )
        .await
        .expect("holder");
        let returned = Arc::new(Mutex::new(None));
        let returned_observer = Arc::clone(&returned);
        let mut connection = DruidPooledConnection::with_standard_holder(
            holder,
            format!("task5-fatal-{operation}"),
            None,
            false,
            false,
            Box::new(move |holder, disposition| {
                *returned_observer
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some((holder, disposition));
                false
            }),
        );
        connection.set_exception_sorter(Arc::new(AllFatalSorter));
        let result = match operation {
            "create_statement" => {
                Connection::create_statement(&mut connection, StatementOptions::default())
                    .await
                    .map(|_| ())
            }
            "prepare_statement" => Connection::prepare_statement(
                &mut connection,
                "SELECT fatal_prepare",
                StatementOptions::default(),
            )
            .await
            .map(|_| ()),
            "prepare_call" => Connection::prepare_call(
                &mut connection,
                "CALL fatal_prepare()",
                StatementOptions::default(),
            )
            .await
            .map(|_| ()),
            "database_meta_data" => Connection::database_meta_data(&mut connection)
                .await
                .map(|_| ()),
            _ => unreachable!("fixed operation list"),
        };
        assert!(matches!(result, Err(RdbcError::SqlException(_))));
        assert!(
            returned
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_none(),
            "fatal notification must not transfer the holder before close"
        );
        assert!(connection.is_closed().expect("fatal lease is closed"));
        assert!(!connection.is_recycled());
        connection.close().await.expect("fatal logical close");
        let (mut holder, disposition) = returned
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .expect("fatal close transfers the holder exactly once");
        assert!(matches!(
            disposition,
            ConnectionRecycleDisposition::Discard { .. }
        ));
        holder
            .close_standard_connection()
            .await
            .expect("close discarded raw");
    }
}

struct AllFatalSorter;

impl ExceptionSorter for AllFatalSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        true
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}
