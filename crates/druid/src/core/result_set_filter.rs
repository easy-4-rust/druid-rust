//! Canonical ResultSet filter contract.

use super::standard_result_set_filter_chain::StandardResultSetFilterChain;
use super::standard_result_set_open_context::StandardResultSetOpenContext;
use super::{DruidError, ResultSetFilterContext, Value};
use druid_rdbc::metadata::ResultSetMetaData;
use druid_rdbc::result_set::ColumnIndex;
use std::sync::Arc;

/// Around-filter contract for the single canonical ResultSet surface.
#[allow(private_interfaces)]
#[async_trait::async_trait]
pub trait ResultSetFilter: Send + Sync {
    /// Observes a successfully registered result set.
    fn result_set_open_after(&self, _context: &ResultSetFilterContext) -> Result<(), DruidError> {
        Ok(())
    }

    /// Observes and may configure the canonical result-set proxy after registration.
    fn standard_result_set_open_after_with_proxy(
        &self,
        context: &mut StandardResultSetOpenContext<'_>,
    ) -> Result<(), DruidError> {
        self.result_set_open_after(context.filter_context())
    }

    /// Runs before `ResultSet::next`.
    fn standard_result_set_next_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs after `ResultSet::next`.
    fn standard_result_set_next_after(
        &self,
        _context: &ResultSetFilterContext,
        _result: &Result<bool, DruidError>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs before `ResultSet::close`.
    fn standard_result_set_close_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs after `ResultSet::close`.
    fn standard_result_set_close_after(
        &self,
        _context: &ResultSetFilterContext,
        _result: &Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs before `ResultSet::value`.
    fn standard_result_set_value_before(
        &self,
        _context: &ResultSetFilterContext,
        _column: ColumnIndex,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs after `ResultSet::value` and may replace the result.
    fn standard_result_set_value_after(
        &self,
        _context: &ResultSetFilterContext,
        _column: ColumnIndex,
        _result: &mut Result<Value, DruidError>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs before `ResultSet::meta_data`.
    fn standard_result_set_meta_data_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs after `ResultSet::meta_data` and may replace the result.
    fn standard_result_set_meta_data_after(
        &self,
        _context: &ResultSetFilterContext,
        _result: &mut Result<Arc<dyn ResultSetMetaData>, DruidError>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs before `ResultSet::was_null`.
    fn standard_result_set_was_null_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Runs after `ResultSet::was_null` and may replace the result.
    fn standard_result_set_was_null_after(
        &self,
        _context: &ResultSetFilterContext,
        _result: &mut Result<bool, DruidError>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Around hook for canonical `ResultSet::next`.
    async fn standard_result_set_next(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
    ) -> Result<bool, DruidError> {
        self.standard_result_set_next_before(chain.context())?;
        let result = chain.result_set_next().await;
        self.standard_result_set_next_after(chain.context(), &result)?;
        result
    }

    /// Around hook for canonical `ResultSet::close`.
    async fn standard_result_set_close(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.standard_result_set_close_before(chain.context())?;
        let result = chain.result_set_close().await;
        self.standard_result_set_close_after(chain.context(), &result)?;
        result
    }

    /// Around hook for canonical `ResultSet::value`.
    fn standard_result_set_value(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        column: ColumnIndex,
    ) -> Result<Value, DruidError> {
        self.standard_result_set_value_before(chain.context(), column)?;
        let mut result = chain.result_set_value(column);
        self.standard_result_set_value_after(chain.context(), column, &mut result)?;
        result
    }

    /// Around hook for canonical `ResultSet::meta_data`.
    fn standard_result_set_meta_data(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
    ) -> Result<Arc<dyn ResultSetMetaData>, DruidError> {
        self.standard_result_set_meta_data_before(chain.context())?;
        let mut result = chain.result_set_meta_data();
        self.standard_result_set_meta_data_after(chain.context(), &mut result)?;
        result
    }

    /// Around hook for canonical `ResultSet::was_null`.
    fn standard_result_set_was_null(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
    ) -> Result<bool, DruidError> {
        self.standard_result_set_was_null_before(chain.context())?;
        let mut result = chain.result_set_was_null();
        self.standard_result_set_was_null_after(chain.context(), &mut result)?;
        result
    }
}
