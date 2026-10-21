use super::*;

pub(crate) struct SqliteExecutionCleanupGuard<'a> {
    pub(crate) connection: &'a mut SqlxConnectionAdapter,
    pub(crate) confirmed: bool,
}

pub(crate) enum CleanupFailure {
    Error(RdbcError),
    TimedOut,
}

impl<'a> SqliteExecutionCleanupGuard<'a> {
    pub(crate) fn new(connection: &'a mut SqlxConnectionAdapter) -> Self {
        Self {
            connection,
            confirmed: false,
        }
    }

    pub(crate) fn connection(&mut self) -> &mut SqlxConnectionAdapter {
        self.connection
    }

    pub(crate) async fn clear(&mut self, timeout: Duration) -> Result<(), RdbcError> {
        let outcome =
            match tokio::time::timeout(timeout, self.connection.clear_sqlite_progress_deadline())
                .await
            {
                Ok(Ok(())) => Ok(()),
                Ok(Err(error)) => Err(CleanupFailure::Error(error)),
                Err(_) => Err(CleanupFailure::TimedOut),
            };
        self.finish(outcome)
    }

    pub(crate) fn finish(&mut self, outcome: Result<(), CleanupFailure>) -> Result<(), RdbcError> {
        match outcome {
            Ok(()) => {
                self.confirmed = true;
                Ok(())
            }
            Err(CleanupFailure::Error(error)) => {
                self.connection.discard_after_unconfirmed_interrupt();
                Err(error)
            }
            Err(CleanupFailure::TimedOut) => {
                self.connection.discard_after_unconfirmed_interrupt();
                Err(RdbcError::ConnectionDiscarded)
            }
        }
    }
}

impl Drop for SqliteExecutionCleanupGuard<'_> {
    fn drop(&mut self) {
        if !self.confirmed {
            self.connection.discard_after_unconfirmed_interrupt();
        }
    }
}
