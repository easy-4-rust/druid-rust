use super::sqlx_cursor_request::SqlxCursorRequest;
use druid_rdbc::types::{Row, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch};

#[tokio::test(flavor = "current_thread")]
async fn driver_stream_is_polled_only_once_per_accepted_next_request() {
    let polls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&polls);
    // 直接检查生产请求调度器的 driver-stream 边界；不通过计时猜测是否发生预读。
    let rows = futures::stream::poll_fn(move |_| {
        let ordinal = observed.fetch_add(1, Ordering::SeqCst) + 1;
        std::task::Poll::Ready(Some(Ok(Row::new(vec![Value::Int(ordinal as i64)]))))
    });
    let (requests, mut receiver) = mpsc::channel(1);
    let (startup, ready) = oneshot::channel();
    let (stop_tx, mut stop) = watch::channel(false);
    let (parent_stop_tx, mut parent_stop) = watch::channel(false);
    let (statement_stop_tx, mut statement_stop) = watch::channel(false);
    let interrupted = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker = tokio::spawn(async move {
        SqlxCursorRequest::serve(
            Vec::new(),
            &mut Some(startup),
            &mut receiver,
            &mut stop,
            &mut parent_stop,
            &mut statement_stop,
            interrupted,
            rows,
        )
        .await
        .unwrap();
    });
    let _stop_senders = (stop_tx, parent_stop_tx, statement_stop_tx);
    ready.await.unwrap().unwrap();
    assert_eq!(
        polls.load(Ordering::SeqCst),
        0,
        "metadata setup must not advance driver rows"
    );
    for expected in 1..=3 {
        let (reply, response) = oneshot::channel();
        assert!(requests.try_send(SqlxCursorRequest { reply }).is_ok());
        let row = response.await.unwrap().unwrap().unwrap();
        assert_eq!(row.get(0), Some(&Value::Int(expected)));
        assert_eq!(polls.load(Ordering::SeqCst), expected as usize);
        tokio::task::yield_now().await;
        assert_eq!(
            polls.load(Ordering::SeqCst),
            expected as usize,
            "idle scheduler must not prefetch another row"
        );
    }
    drop(requests);
    worker.await.unwrap();
    assert_eq!(polls.load(Ordering::SeqCst), 3);
}
