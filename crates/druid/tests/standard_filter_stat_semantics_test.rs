//! Task 5 review C3：canonical standard proxy 必须保留 Druid Filter/Stat/error policy。

mod standard_test_support;

use druid::core::{
    AfterFilter, BeforeFilter, ConnectionEvent, ConnectionRecycleDisposition,
    DruidConnectionHolder, DruidError, DruidPooledConnection, ExceptionSorter,
    ExceptionSorterProperties, ExecContext, ExecResult, FilterChain, ResultSetFilter,
    ResultSetFilterContext,
};
use druid::pool::config::PoolInnerConfig;
use druid::pool::DruidPool;
use druid::sql::{SqlException, WallConfig, WallFilter};
use druid::stats::StatFilter;
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{
    ExecutionResult, GeneratedKeys, PreparedStatement, Statement, StatementOptions,
};
use standard_test_support::{TrackingConnection, TrackingFactory, TrackingState};
use std::sync::atomic::{AtomicUsize, Ordering};
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

struct AllFatalSorter;

impl ExceptionSorter for AllFatalSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        true
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}
