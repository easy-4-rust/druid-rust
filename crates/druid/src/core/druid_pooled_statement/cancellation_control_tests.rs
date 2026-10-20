use super::*;
use crate::core::owner_disposition_control::OwnerDispositionControl;
use druid_rdbc::statement::{StatementCancellation, StatementOptions};
use std::sync::atomic::AtomicUsize;

struct CancelCounter(Arc<AtomicUsize>);

#[async_trait::async_trait]
impl StatementCancellation for CancelCounter {
    async fn cancel(&self) -> Result<(), RdbcError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}

struct CancellationProbe {
    calls: Arc<AtomicUsize>,
}

druid_rdbc::impl_standard_wrapper!(CancellationProbe => Statement);

#[async_trait::async_trait]
impl Statement for CancellationProbe {
    fn cancellation_control(&self) -> Result<Arc<dyn StatementCancellation>, RdbcError> {
        Ok(Arc::new(CancelCounter(Arc::clone(&self.calls))))
    }
}

#[tokio::test]
async fn pooled_cancellation_delegates_only_while_original_lease_is_active() {
    let calls = Arc::new(AtomicUsize::new(0));
    let resources = Arc::new(StandardResourceRegistry::default());
    let generation = resources.current_generation();
    let lease_active = Arc::new(AtomicBool::new(true));
    let policy = StandardLogicalPolicy::new(
        7,
        "cancel-test".to_owned(),
        Arc::clone(&lease_active),
        Arc::new(AtomicBool::new(false)),
        None,
        Some(true),
        generation,
        Arc::new(OwnerDispositionControl::new(Box::new(|_| {}))),
    );
    let mut statement = DruidPooledStatement::new_standard(
        Box::new(CancellationProbe {
            calls: Arc::clone(&calls),
        }),
        StatementOptions::default(),
        Arc::clone(&resources),
        policy,
        7,
        11,
        Arc::new(std::sync::atomic::AtomicU64::new(1)),
        Arc::new(std::sync::atomic::AtomicU64::new(1)),
        Arc::clone(&lease_active),
        None,
        None,
    );
    assert!(matches!(
        Statement::cancel(&mut statement),
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_cancel"
        })
    ));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    let admission = resources.begin_admission(generation).unwrap();
    resources
        .register(&admission, Arc::clone(&statement.inner))
        .await
        .unwrap();
    drop(admission);

    let cancellation = statement.cancellation_control().unwrap();
    cancellation.cancel().await.unwrap();
    assert_eq!(calls.load(Ordering::Acquire), 1);

    lease_active.store(false, Ordering::Release);
    assert!(matches!(
        cancellation.cancel().await,
        Err(RdbcError::ConnectionDiscarded)
    ));
    assert_eq!(calls.load(Ordering::Acquire), 1);
}
