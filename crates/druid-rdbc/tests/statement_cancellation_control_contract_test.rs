use async_trait::async_trait;
use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{Statement, StatementCancellation};
use druid_rdbc::wrapper::Wrapper;
use std::any::TypeId;
use std::cell::Cell;
use std::sync::Arc;
use tokio::sync::watch;

struct CancelControl {
    sender: watch::Sender<bool>,
}

#[async_trait]
impl StatementCancellation for CancelControl {
    async fn cancel(&self) -> Result<(), RdbcError> {
        self.sender.send_replace(true);
        Ok(())
    }
}

struct NonSyncStatement {
    receiver: watch::Receiver<bool>,
    control: Arc<CancelControl>,
    active: Cell<bool>,
}

impl Wrapper for NonSyncStatement {
    fn wrapper_view(&self) -> &dyn Wrapper {
        self
    }

    fn canonical_unwrap(
        &self,
        _target: TypeId,
    ) -> Option<druid_rdbc::wrapper::StandardUnwrapped<'_>> {
        None
    }
}

#[async_trait]
impl Statement for NonSyncStatement {
    fn cancellation_control(&self) -> Result<Arc<dyn StatementCancellation>, RdbcError> {
        Ok(Arc::clone(&self.control) as Arc<dyn StatementCancellation>)
    }

    async fn execute_query(&mut self, _sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.active.set(true);
        let mut receiver = self.receiver.clone();
        receiver
            .changed()
            .await
            .map_err(|_| RdbcError::ConnectionDiscarded)?;
        Err(RdbcError::SqlException(Box::new(SqlException::new(
            0,
            Some("HY008".into()),
            Some("operation cancelled".into()),
        ))))
    }
}

#[tokio::test]
async fn control_acquired_before_execution_can_cancel_a_non_sync_statement_future() {
    let (sender, receiver) = watch::channel(false);
    let control = Arc::new(CancelControl { sender });
    let mut statement = NonSyncStatement {
        receiver,
        control: Arc::clone(&control),
        active: Cell::new(false),
    };
    let cancellation = statement.cancellation_control().unwrap();

    let execution = tokio::spawn(async move { statement.execute_query("select slow()").await });
    tokio::task::yield_now().await;
    cancellation.cancel().await.unwrap();

    let error = match execution.await.unwrap() {
        Err(error) => error,
        Ok(_) => panic!("the cancellation request must terminate the active query"),
    };
    assert!(matches!(error, RdbcError::SqlException(ref sql) if sql.sql_state() == Some("HY008")));
}

struct MinimalStatement;

impl Wrapper for MinimalStatement {
    fn wrapper_view(&self) -> &dyn Wrapper {
        self
    }

    fn canonical_unwrap(
        &self,
        _target: TypeId,
    ) -> Option<druid_rdbc::wrapper::StandardUnwrapped<'_>> {
        None
    }
}

#[async_trait]
impl Statement for MinimalStatement {}

#[test]
fn minimal_external_statement_reports_standard_feature_not_supported() {
    let statement = MinimalStatement;
    assert!(matches!(
        statement.cancellation_control(),
        Err(RdbcError::SqlException(ref error)) if error.sql_state() == Some("0A000")
    ));
}
