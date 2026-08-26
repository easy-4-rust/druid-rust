use super::StandardResultSetResource;
use crate::core::RdbcError;
use druid_rdbc::result_set::{ResultSet, ResultSetNavigation};
use std::sync::atomic::Ordering;

impl StandardResultSetResource {
    pub(super) fn access_core<T>(
        &self,
        operation: impl FnOnce(&mut dyn ResultSet) -> Result<T, RdbcError>,
    ) -> Result<T, RdbcError> {
        let mut result_set = self
            .result_set
            .try_lock()
            .map_err(|_| RdbcError::Other("result set is busy".to_owned()))?;
        let raw = result_set.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        self.policy
            .classify(operation(raw.as_mut()), self.filter_context.sql())
    }

    pub(super) async fn navigate_core(
        &self,
        navigation: ResultSetNavigation,
    ) -> Result<bool, RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let mut result_set = self.result_set.lock().await;
        let raw = result_set.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        let result = raw.navigate(navigation).await;
        self.policy.classify(result, self.filter_context.sql())
    }
}
