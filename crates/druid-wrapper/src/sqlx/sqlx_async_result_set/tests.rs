use super::SqlxAsyncResultSet;
use crate::sqlx::sqlx_connection_actor::SqlxConnectionActor;
use crate::sqlx::SqlxConnectionAdapter;
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::statement::{FetchDirection, StatementOptions};
use druid_rdbc::types::{Row, Value};
use futures::FutureExt;
use std::cell::Cell;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch};

#[tokio::test]
async fn failed_cursor_worker_is_joined_once_and_repeated_close_does_not_panic() {
    let actor = SqlxConnectionActor::new(
        SqlxConnectionAdapter::connect("sqlite::memory:")
            .await
            .unwrap(),
    );
    let (requests, _) = mpsc::channel(1);
    let (shutdown, _) = watch::channel(false);
    let (_statement_shutdown, statement_stop) = watch::channel(false);
    let worker = tokio::spawn(async { panic!("injected cursor worker failure") });
    let mut cursor = SqlxAsyncResultSet {
        actor: Arc::clone(&actor),
        metadata: None,
        requests: Some(requests),
        pending: None,
        shutdown,
        statement_stop,
        worker: Some(worker),
        current: None,
        was_null: Cell::new(false),
        row_number: 0,
        finished: false,
        closed: false,
        fetch_size: 1,
        direction: FetchDirection::Forward,
        options: StatementOptions::default(),
    };
    let error = cursor.close().await.unwrap_err();
    assert!(error.to_string().contains("cursor worker failed"));
    let repeated = AssertUnwindSafe(cursor.close()).catch_unwind().await;
    assert!(
        repeated.is_ok(),
        "a completed JoinHandle must not be polled a second time"
    );
    repeated.unwrap().unwrap();
    actor.close().await.unwrap();
}

#[tokio::test]
async fn cancelled_next_keeps_the_pending_row_for_the_next_call() {
    let actor = SqlxConnectionActor::new(
        SqlxConnectionAdapter::connect("sqlite::memory:")
            .await
            .unwrap(),
    );
    let (requests, _receiver) = mpsc::channel(1);
    let (shutdown, _) = watch::channel(false);
    let (_statement_shutdown, statement_stop) = watch::channel(false);
    let (reply, pending) = oneshot::channel();
    let worker = tokio::spawn(async {});
    let mut cursor = SqlxAsyncResultSet {
        actor: Arc::clone(&actor),
        metadata: None,
        requests: Some(requests),
        pending: Some(pending),
        shutdown,
        statement_stop,
        worker: Some(worker),
        current: None,
        was_null: Cell::new(false),
        row_number: 0,
        finished: false,
        closed: false,
        fetch_size: 1,
        direction: FetchDirection::Forward,
        options: StatementOptions::default(),
    };

    // 只轮询一次并丢弃 pending future，模拟调用方取消 next。
    assert!(futures::poll!(cursor.next()).is_pending());
    reply
        .send(Ok(Some(Row::new(vec![Value::Int(42)]))))
        .unwrap();

    assert!(cursor.next().await.unwrap());
    assert_eq!(cursor.row().unwrap(), 1);
    assert_eq!(
        cursor.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(42)
    );
    cursor.close().await.unwrap();
    actor.close().await.unwrap();
}
