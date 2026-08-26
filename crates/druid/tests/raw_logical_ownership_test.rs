mod standard_test_support;

use druid::core::{ConnectionRecycleDisposition, DruidConnectionHolder, DruidPooledConnection};
use druid_rdbc::connection::Connection;
use druid_rdbc::statement::{Statement, StatementOptions};
use standard_test_support::{TrackingConnection, TrackingState};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[tokio::test]
async fn logical_close_recycles_once_while_native_eviction_closes_raw_once() {
    let state = TrackingState::configured();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(TrackingConnection::new(state.clone())),
        51,
        Duration::ZERO,
        0,
    )
    .await
    .expect("native holder");
    let recycle_count = Arc::new(AtomicUsize::new(0));
    let returned = Arc::new(Mutex::new(None));
    let recycle_observer = recycle_count.clone();
    let returned_holder = returned.clone();
    let mut logical = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-native".to_string(),
        None,
        false,
        false,
        Box::new(move |holder, disposition| {
            assert_eq!(disposition, ConnectionRecycleDisposition::Reusable);
            recycle_observer.fetch_add(1, Ordering::AcqRel);
            *returned_holder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(holder);
            false
        }),
    );

    Connection::close(&mut logical)
        .await
        .expect("first logical close must recycle");
    Connection::close(&mut logical)
        .await
        .expect("second logical close must be idempotent");
    assert_eq!(recycle_count.load(Ordering::Acquire), 1);
    assert_eq!(state.raw_close_count.load(Ordering::Acquire), 0);

    let mut holder = returned
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .expect("native pool must regain one holder");
    holder
        .close_standard_connection()
        .await
        .expect("eviction owns raw close");
    holder
        .close_standard_connection()
        .await
        .expect("raw close must be take-once");
    assert_eq!(state.raw_close_count.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn external_lease_never_enters_the_native_return_path() {
    let state = TrackingState::configured();
    let external_returns = Arc::new(AtomicUsize::new(0));
    let native_returns = Arc::new(AtomicUsize::new(0));
    let observed_external = external_returns.clone();
    let observed_native = native_returns.clone();
    let mut logical = DruidPooledConnection::with_external_standard_connection(
        Box::new(TrackingConnection::new(state.clone())),
        52,
        "task5-external".to_string(),
        None,
        Box::new(move |_connection, disposition| {
            assert_eq!(disposition, ConnectionRecycleDisposition::Reusable);
            observed_external.fetch_add(1, Ordering::AcqRel);
        }),
        Box::new(move |_holder, _disposition| {
            observed_native.fetch_add(1, Ordering::AcqRel);
            false
        }),
    )
    .await
    .expect("external logical connection");

    let mut prepared = Connection::prepare_statement(
        &mut logical,
        "SELECT external_lease",
        StatementOptions::default(),
    )
    .await
    .expect("external prepared statement");
    Statement::close(prepared.as_mut())
        .await
        .expect("external prepared close");
    assert_eq!(state.prepared_close_count.load(Ordering::Acquire), 1);

    Connection::close(&mut logical)
        .await
        .expect("external logical close");
    Connection::close(&mut logical)
        .await
        .expect("external double close");
    assert_eq!(external_returns.load(Ordering::Acquire), 1);
    assert_eq!(native_returns.load(Ordering::Acquire), 0);
    assert_eq!(state.raw_close_count.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn reset_failure_discards_and_raw_close_remains_exactly_once() {
    let state = TrackingState::configured();
    state.auto_commit.store(false, Ordering::Release);
    state.read_only.store(false, Ordering::Release);
    state.fail_next("set_read_only");
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(TrackingConnection::new(state.clone())),
        53,
        Duration::ZERO,
        0,
    )
    .await
    .expect("holder captures initial state");
    state.read_only.store(true, Ordering::Release);

    let returned = Arc::new(Mutex::new(None));
    let returned_holder = returned.clone();
    let mut logical = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-reset-failure".to_string(),
        None,
        false,
        false,
        Box::new(move |holder, disposition| {
            assert!(matches!(
                disposition,
                ConnectionRecycleDisposition::Discard {
                    recycle_error: Some(_)
                }
            ));
            *returned_holder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(holder);
            false
        }),
    );

    Connection::close(&mut logical)
        .await
        .expect("logical close folds reset error into discard");
    let mut holder = returned
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .expect("discarded holder returned to owner");
    holder
        .close_standard_connection()
        .await
        .expect("discard owner closes raw");
    assert_eq!(state.raw_close_count.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn failed_raw_close_is_not_retried_by_the_owner() {
    let state = TrackingState::configured();
    state.fail_next("close");
    let mut holder = DruidConnectionHolder::with_standard_connection(
        Box::new(TrackingConnection::new(state.clone())),
        54,
        Duration::ZERO,
        0,
    )
    .await
    .expect("native holder");

    assert!(holder.close_standard_connection().await.is_err());
    holder
        .close_standard_connection()
        .await
        .expect("take-once close cannot retry a failed raw handle");
    assert_eq!(state.raw_close_attempt_count.load(Ordering::Acquire), 1);
    assert_eq!(state.raw_close_count.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn standard_recycle_rolls_back_and_restores_the_complete_state() {
    let state = TrackingState::configured();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(TrackingConnection::new(state.clone())),
        55,
        Duration::ZERO,
        0,
    )
    .await
    .expect("native holder captures standard defaults");
    state.auto_commit.store(false, Ordering::Release);
    *state
        .catalog
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some("borrower_catalog".to_string());
    *state
        .schema
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some("borrower_schema".to_string());

    let returned = Arc::new(Mutex::new(None));
    let returned_holder = returned.clone();
    let mut logical = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-reset-success".to_string(),
        None,
        false,
        false,
        Box::new(move |holder, disposition| {
            assert_eq!(disposition, ConnectionRecycleDisposition::Reusable);
            *returned_holder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(holder);
            false
        }),
    );

    Connection::close(&mut logical)
        .await
        .expect("standard logical recycle");
    assert_eq!(
        state.event_snapshot(),
        vec![
            "connection.rollback",
            "connection.set_catalog:catalog_default",
            "connection.set_schema:schema_default",
            "connection.set_auto_commit:true",
            "connection.clear_warnings",
        ]
    );
    let mut holder = returned
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .expect("native owner receives reusable holder");
    holder
        .close_standard_connection()
        .await
        .expect("native owner closes raw once");
}
