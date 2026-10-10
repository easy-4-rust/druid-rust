use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{
    Blob, BlobPattern, RdbcInputStream, RdbcOutputStream, ResourceHandleFactory, ResourceOperations,
};
use std::future::Future;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::task::Poll;

#[derive(Debug)]
struct ScriptedBlob {
    calls: Arc<AtomicUsize>,
    reads: Arc<AtomicUsize>,
    retry_fails: bool,
}
#[async_trait::async_trait]
impl Blob for ScriptedBlob {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ | ResourceOperations::FREE
    }
    async fn length(&mut self) -> Result<i64, RdbcError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(7)
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        let attempt = self.calls.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            futures::future::pending::<()>().await;
        }
        if attempt == 1 && self.retry_fails {
            Err(RdbcError::DriverError(
                "retry outcome remains unknown".to_owned(),
            ))
        } else {
            Ok(())
        }
    }
    async fn get_bytes(&mut self, _: i64, _: i32) -> Result<Vec<u8>, RdbcError> {
        Err(RdbcError::feature_not_supported("fixture get_bytes"))
    }
    async fn get_binary_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "fixture get_binary_stream",
        ))
    }
    async fn position_bytes(&mut self, _: &[u8], _: i64) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::feature_not_supported("fixture position_bytes"))
    }
    async fn position_blob(
        &mut self,
        _: BlobPattern<'_>,
        _: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::feature_not_supported("fixture position_blob"))
    }
    async fn set_bytes(&mut self, _: i64, _: &[u8]) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported("fixture set_bytes"))
    }
    async fn set_bytes_range(
        &mut self,
        _: i64,
        _: &[u8],
        _: i32,
        _: i32,
    ) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported("fixture set_bytes_range"))
    }
    async fn set_binary_stream(&mut self, _: i64) -> Result<RdbcOutputStream, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "fixture set_binary_stream",
        ))
    }
    async fn truncate(&mut self, _: i64) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("fixture truncate"))
    }
    async fn get_binary_stream_range(
        &mut self,
        _: i64,
        _: i64,
    ) -> Result<RdbcInputStream, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "fixture get_binary_stream_range",
        ))
    }
}
fn handle(
    start: usize,
    retry_fails: bool,
) -> (
    druid_rdbc::resource::RdbcBlob,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
) {
    let calls = Arc::new(AtomicUsize::new(start));
    let reads = Arc::new(AtomicUsize::new(0));
    (
        ResourceHandleFactory::blob(Box::new(ScriptedBlob {
            calls: Arc::clone(&calls),
            reads: Arc::clone(&reads),
            retry_fails,
        })),
        calls,
        reads,
    )
}
async fn cancel_started_release(blob: &druid_rdbc::resource::RdbcBlob) {
    let mut release = Box::pin(blob.free());
    assert!(
        futures::future::poll_fn(|cx| Poll::Ready(release.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    drop(release);
}
#[test]
fn failed_retry_does_not_reopen_unconfirmed_resource() {
    futures::executor::block_on(async {
        let (blob, calls, reads) = handle(0, true);
        cancel_started_release(&blob).await;
        assert!(blob.length().await.is_err());
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert!(blob.free().await.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(
            blob.length().await.is_err(),
            "failed retry cannot turn an unknown release outcome back into an open resource"
        );
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        blob.free().await.unwrap();
        assert!(blob.is_freed());
    });
}
#[test]
fn initial_known_failure_preserves_usable_resource() {
    futures::executor::block_on(async {
        let (blob, _, reads) = handle(1, true);
        assert!(blob.free().await.is_err());
        assert_eq!(blob.length().await.unwrap(), 7);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
    });
}
#[test]
fn successful_retry_confirms_release_without_repeating_driver_free() {
    futures::executor::block_on(async {
        let (blob, calls, reads) = handle(0, false);
        cancel_started_release(&blob).await;
        blob.free().await.unwrap();
        blob.free().await.unwrap();
        assert!(blob.is_freed());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(blob.length().await.is_err());
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn queued_failed_retry_keeps_unconfirmed_state_observed_after_lock() {
    futures::executor::block_on(async {
        let (blob, calls, reads) = handle(0, true);
        let alias = blob.clone();
        let mut first = Box::pin(blob.free());
        assert!(
            futures::future::poll_fn(|cx| Poll::Ready(first.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        let mut retry = Box::pin(alias.free());
        assert!(
            futures::future::poll_fn(|cx| Poll::Ready(retry.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        drop(first);
        assert!(retry.await.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(
            blob.length().await.is_err(),
            "queued retry must retain the unconfirmed state after acquiring the lock"
        );
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn reads_are_rejected_during_release_and_after_cancellation() {
    futures::executor::block_on(async {
        let (blob, calls, reads) = handle(0, false);
        let alias = blob.clone();
        let mut release = Box::pin(blob.free());
        assert!(
            futures::future::poll_fn(|cx| Poll::Ready(release.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        let mut read = Box::pin(alias.length());
        let result = futures::future::poll_fn(|cx| Poll::Ready(read.as_mut().poll(cx))).await;
        assert!(matches!(result, Poll::Ready(Err(_))));
        drop(read);
        drop(release);
        assert!(blob.length().await.is_err());
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        blob.free().await.unwrap();
        assert!(blob.is_freed());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    });
}
