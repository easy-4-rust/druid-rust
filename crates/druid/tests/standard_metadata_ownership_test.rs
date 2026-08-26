mod standard_test_support;

use druid::core::{
    DruidConnectionHolder, DruidError, DruidPooledConnection, FilterChain, RdbcError,
    ResultSetFilter, ResultSetFilterContext,
};
use druid_rdbc::connection::Connection;
use druid_rdbc::metadata::TablesQuery;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{GeneratedKeys, ParameterIndex, Statement, StatementOptions};
use druid_rdbc::types::Value;
use standard_test_support::{TrackingConnection, TrackingState};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

#[tokio::test]
async fn live_metadata_owns_logical_actor_and_snapshots_are_borrow_free() {
    let state = TrackingState::configured();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(TrackingConnection::new(state.clone())),
        61,
        Duration::ZERO,
        0,
    )
    .await
    .expect("standard holder");
    let returned = Arc::new(Mutex::new(None));
    let returned_holder = Arc::clone(&returned);
    let mut connection = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-metadata".to_string(),
        None,
        false,
        false,
        Box::new(move |holder, _disposition| {
            *returned_holder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(holder);
            false
        }),
    );

    let mut metadata = Connection::database_meta_data(&mut connection)
        .await
        .expect("owned live metadata");
    assert_eq!(
        metadata
            .database_product_name()
            .await
            .expect("live product name"),
        "Task5DB"
    );
    let mut tables = metadata
        .tables(TablesQuery::default())
        .await
        .expect("owned metadata result set");
    assert!(tables.next().await.expect("metadata result next"));
    assert_eq!(
        tables
            .value(ColumnIndex::new(1).expect("1-based column"))
            .expect("table name"),
        Value::String("task5_table".to_string())
    );
    let tables_snapshot = tables.meta_data().expect("metadata table snapshot");

    let mut prepared =
        Connection::prepare_statement(&mut connection, "SELECT ?", StatementOptions::default())
            .await
            .expect("prepared statement");
    let parameter_snapshot = prepared
        .parameter_meta_data()
        .expect("immutable parameter metadata");
    let mut result = prepared
        .execute_prepared_query()
        .await
        .expect("prepared result");
    assert!(result.next().await.expect("prepared result next"));
    let result_snapshot = result.meta_data().expect("immutable result metadata");
    let mut stale_statement =
        Connection::create_statement(&mut connection, StatementOptions::default())
            .await
            .expect("owned statement actor");

    prepared.close().await.expect("prepared close");
    Connection::close(&mut connection)
        .await
        .expect("logical connection close");
    let holder = returned
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .expect("first metadata lease returned holder");
    let mut second = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-metadata-second".to_owned(),
        None,
        false,
        false,
        Box::new(|_holder, _disposition| false),
    );
    let mut second_statement =
        Connection::create_statement(&mut second, StatementOptions::default())
            .await
            .expect("second lease proves holder reborrow");
    assert!(metadata.database_product_name().await.is_err());
    assert!(tables.next().await.is_err());
    assert!(tables
        .value(ColumnIndex::new(1).expect("1-based column"))
        .is_err());
    assert!(tables.meta_data().is_err());
    assert!(Statement::is_closed(stale_statement.as_ref()).expect("logical close state"));
    assert!(stale_statement
        .execute_update("UPDATE stale", GeneratedKeys::None)
        .await
        .is_err());
    stale_statement
        .close()
        .await
        .expect("stale statement cleanup");
    assert!(result
        .value(ColumnIndex::new(1).expect("1-based column"))
        .is_err());
    assert!(result.meta_data().is_err());
    assert!(result.next().await.is_err());
    result.close().await.expect("stale result cleanup");
    drop(metadata);
    second_statement
        .close()
        .await
        .expect("second statement close");
    Connection::close(&mut second)
        .await
        .expect("second lease close");

    fn assert_snapshot<T: Send + Sync + 'static + ?Sized>(_snapshot: Arc<T>) {}
    assert_snapshot(result_snapshot.clone());
    assert_snapshot(parameter_snapshot.clone());
    assert_snapshot(tables_snapshot.clone());
    assert_eq!(tables_snapshot.column_count(), 1);
    assert_eq!(result_snapshot.column_count(), 1);
    assert_eq!(parameter_snapshot.parameter_count(), 1);
    assert_eq!(
        parameter_snapshot
            .parameter(ParameterIndex::new(1).expect("1-based parameter"))
            .expect("parameter descriptor")
            .type_name(),
        "BIGINT"
    );
}

struct LateRegistrationBarrier {
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
}

impl ResultSetFilter for LateRegistrationBarrier {
    fn result_set_open_after(&self, _context: &ResultSetFilterContext) -> Result<(), DruidError> {
        self.entered.wait();
        self.release.wait();
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn late_metadata_result_registration_is_closed_before_next_lease() {
    let state = TrackingState::configured();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(TrackingConnection::new(Arc::clone(&state))),
        62,
        Duration::ZERO,
        0,
    )
    .await
    .expect("standard holder");
    let returned = Arc::new(Mutex::new(None));
    let returned_holder = Arc::clone(&returned);
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(LateRegistrationBarrier {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    }));
    let mut first = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-late-metadata-first".to_owned(),
        Some(Arc::new(filter_chain)),
        false,
        false,
        Box::new(move |holder, _disposition| {
            *returned_holder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(holder);
            false
        }),
    );
    let mut metadata = Connection::database_meta_data(&mut first)
        .await
        .expect("metadata proxy");
    let query = tokio::spawn(async move { metadata.tables(TablesQuery::default()).await });
    // Raw metadata.tables 已返回 ResultSet，proxy-aware open hook 在 registry.register
    // 之前阻塞，精确覆盖 snapshot-gap 竞态。
    entered.wait();

    let (close_started, close_started_rx) = tokio::sync::oneshot::channel();
    let mut close = tokio::spawn(async move {
        let _ = close_started.send(());
        Connection::close(&mut first).await
    });
    close_started_rx.await.expect("close task started");
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut close)
            .await
            .is_err(),
        "logical close returned while raw admission was pending"
    );
    assert!(
        returned
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none(),
        "holder reopened before late raw cleanup"
    );
    release.wait();
    assert!(matches!(
        query.await.expect("metadata query task"),
        Err(RdbcError::ConnectionDiscarded)
    ));
    close
        .await
        .expect("close task")
        .expect("first lease seals registry");
    let holder = returned
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .expect("holder returned");
    let mut second = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-late-metadata-second".to_owned(),
        None,
        false,
        false,
        Box::new(|_holder, _disposition| false),
    );
    assert_eq!(state.result_set_close_count.load(Ordering::Acquire), 1);

    let mut second_statement =
        Connection::create_statement(&mut second, StatementOptions::default())
            .await
            .expect("next lease remains usable");
    assert_eq!(
        second_statement
            .execute_update("UPDATE next_lease", GeneratedKeys::None)
            .await
            .expect("next lease update"),
        1
    );
    second_statement.close().await.expect("statement close");
    Connection::close(&mut second).await.expect("second close");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn late_result_close_failure_discards_old_generation_without_reopen() {
    let state = TrackingState::configured();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(TrackingConnection::new(Arc::clone(&state))),
        63,
        Duration::ZERO,
        0,
    )
    .await
    .expect("standard holder");
    let returned = Arc::new(Mutex::new(None));
    let returned_holder = Arc::clone(&returned);
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(LateRegistrationBarrier {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    }));
    let mut first = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-late-close-failure".to_owned(),
        Some(Arc::new(filter_chain)),
        false,
        false,
        Box::new(move |holder, disposition| {
            *returned_holder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((holder, disposition));
            false
        }),
    );
    let mut metadata = Connection::database_meta_data(&mut first)
        .await
        .expect("metadata proxy");
    let query = tokio::spawn(async move { metadata.tables(TablesQuery::default()).await });
    entered.wait();
    let (close_started, close_started_rx) = tokio::sync::oneshot::channel();
    let mut close = tokio::spawn(async move {
        let _ = close_started.send(());
        Connection::close(&mut first).await
    });
    close_started_rx.await.expect("close task started");
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut close)
            .await
            .is_err(),
        "logical close returned before late close failure resolved"
    );
    state.fail_next("result_set_close");
    release.wait();
    assert!(matches!(
        query.await.expect("metadata query task"),
        Err(RdbcError::Other(_))
    ));
    close
        .await
        .expect("close task")
        .expect("logical close reports disposition through callback");
    let (mut holder, disposition) = returned
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .expect("discarded holder returned to owner");
    assert!(disposition.has_recycle_error());
    assert!(holder.is_discard());
    assert_eq!(
        state.result_set_close_attempt_count.load(Ordering::Acquire),
        1
    );
    holder
        .close_standard_connection()
        .await
        .expect("old generation retry and raw shutdown");
    assert_eq!(
        state.result_set_close_attempt_count.load(Ordering::Acquire),
        2
    );
    assert_eq!(state.result_set_close_count.load(Ordering::Acquire), 1);
}
