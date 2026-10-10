//! Wrapper-private Statement state shared by ordinary and prepared handles.

use druid_rdbc::error::{RdbcError, SqlWarning};
use druid_rdbc::statement::{FetchDirection, StatementOptions};

/// Canonical JDBC statement properties and last-result state.
pub(crate) struct StandardStatementState {
    pub(crate) options: StatementOptions,
    pub(crate) max_field_size: i32,
    pub(crate) max_rows: i64,
    pub(crate) query_timeout: i32,
    pub(crate) fetch_direction: FetchDirection,
    pub(crate) fetch_size: i32,
    pub(crate) poolable: bool,
    pub(crate) close_on_completion: bool,
    pub(crate) _cursor_name: Option<String>,
    pub(crate) _escape_processing: bool,
    pub(crate) statement_batch: Vec<String>,
    pub(crate) last_generated_key: Option<i64>,
    pub(crate) last_update_count: Option<i64>,
    pub(crate) warnings: Option<SqlWarning>,
}

impl StandardStatementState {
    pub(crate) fn new(options: StatementOptions) -> Self {
        Self {
            options,
            max_field_size: 0,
            max_rows: 0,
            query_timeout: 0,
            fetch_direction: FetchDirection::Unknown,
            fetch_size: 0,
            poolable: false,
            close_on_completion: false,
            _cursor_name: None,
            _escape_processing: true,
            statement_batch: Vec::new(),
            last_generated_key: None,
            last_update_count: None,
            warnings: None,
        }
    }
}

pub(crate) fn validate_non_negative(value: i64, name: &str) -> Result<(), RdbcError> {
    if value < 0 {
        Err(RdbcError::InvalidArgument(format!(
            "{name} must be non-negative"
        )))
    } else {
        Ok(())
    }
}

macro_rules! delegate_standard_statement_state_methods {
    () => {
        fn options(
            &self,
        ) -> Result<druid_rdbc::statement::StatementOptions, druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            Ok(self.state.options)
        }

        fn max_field_size(&self) -> Result<i32, druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            Ok(self.state.max_field_size)
        }

        fn set_max_field_size(&mut self, max: i32) -> Result<(), druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            $crate::standard_statement_state::validate_non_negative(
                i64::from(max),
                "max field size",
            )?;
            self.state.max_field_size = max;
            Ok(())
        }

        fn max_rows(&self) -> Result<i64, druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            Ok(self.state.max_rows)
        }

        fn set_max_rows(&mut self, max: i64) -> Result<(), druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            $crate::standard_statement_state::validate_non_negative(max, "max rows")?;
            self.state.max_rows = max;
            Ok(())
        }

        fn set_escape_processing(
            &mut self,
            enabled: bool,
        ) -> Result<(), druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            self.state._escape_processing = enabled;
            Ok(())
        }

        fn query_timeout(&self) -> Result<i32, druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            Ok(self.state.query_timeout)
        }

        fn set_query_timeout(&mut self, seconds: i32) -> Result<(), druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            $crate::standard_statement_state::validate_non_negative(
                i64::from(seconds),
                "query timeout",
            )?;
            self.state.query_timeout = seconds;
            Ok(())
        }

        fn cancel(&mut self) -> Result<(), druid_rdbc::error::RdbcError> {
            self.executor_core()?.cancel_running()
        }

        fn set_cursor_name(&mut self, name: &str) -> Result<(), druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            self.state._cursor_name = Some(name.to_owned());
            Ok(())
        }

        fn set_fetch_direction(
            &mut self,
            direction: druid_rdbc::statement::FetchDirection,
        ) -> Result<(), druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            self.state.fetch_direction = direction;
            Ok(())
        }

        fn fetch_direction(
            &self,
        ) -> Result<druid_rdbc::statement::FetchDirection, druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            Ok(self.state.fetch_direction)
        }

        fn set_fetch_size(&mut self, rows: i32) -> Result<(), druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            $crate::standard_statement_state::validate_non_negative(i64::from(rows), "fetch size")?;
            self.state.fetch_size = rows;
            Ok(())
        }

        fn fetch_size(&self) -> Result<i32, druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            Ok(self.state.fetch_size)
        }

        fn is_poolable(&self) -> Result<bool, druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            Ok(self.state.poolable)
        }

        fn set_poolable(&mut self, poolable: bool) -> Result<(), druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            self.state.poolable = poolable;
            Ok(())
        }

        fn close_on_completion(&mut self) -> Result<(), druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            self.state.close_on_completion = true;
            Ok(())
        }

        fn is_close_on_completion(&self) -> Result<bool, druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            Ok(self.state.close_on_completion)
        }

        fn warnings(
            &self,
        ) -> Result<Option<druid_rdbc::error::SqlWarning>, druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            Ok(self.state.warnings.clone())
        }

        fn clear_warnings(&mut self) -> Result<(), druid_rdbc::error::RdbcError> {
            self.executor_core()?;
            self.state.warnings = None;
            Ok(())
        }
    };
}

pub(crate) use delegate_standard_statement_state_methods;
