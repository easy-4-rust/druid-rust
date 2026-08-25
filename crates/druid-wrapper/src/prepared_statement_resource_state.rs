//! Wrapper-private common state for transitional physical prepared handles.

use druid_rdbc::error::{RdbcError, SqlWarning};
use druid_rdbc::spi::PhysicalStatementOptions;
use std::sync::Mutex;

struct State {
    closed: bool,
    max_field_size: i32,
    max_rows: i32,
    escape_processing: bool,
    query_timeout: i32,
    cursor_name: Option<String>,
    fetch_direction: i32,
    fetch_size: i32,
    close_on_completion: bool,
    warnings: Option<SqlWarning>,
}

pub(crate) struct PreparedStatementResourceState {
    sql: String,
    options: PhysicalStatementOptions,
    state: Mutex<State>,
}

impl PreparedStatementResourceState {
    pub(crate) fn new(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            options: PhysicalStatementOptions::default(),
            state: Mutex::new(State {
                closed: false,
                max_field_size: 0,
                max_rows: 0,
                escape_processing: true,
                query_timeout: 0,
                cursor_name: None,
                fetch_direction: 1000,
                fetch_size: 0,
                close_on_completion: false,
                warnings: None,
            }),
        }
    }

    fn with_open<T>(&self, operation: impl FnOnce(&mut State) -> T) -> Result<T, RdbcError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed {
            return Err(RdbcError::ConnectionDiscarded);
        }
        Ok(operation(&mut state))
    }

    pub(crate) fn sql(&self) -> &str {
        &self.sql
    }
    pub(crate) fn statement_options(&self) -> PhysicalStatementOptions {
        self.options
    }
    pub(crate) fn close(&self) -> Result<(), RdbcError> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed = true;
        Ok(())
    }
    pub(crate) fn is_closed(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed
    }
    pub(crate) fn max_field_size(&self) -> Result<i32, RdbcError> {
        self.with_open(|state| state.max_field_size)
    }
    pub(crate) fn set_max_field_size(&self, value: i32) -> Result<(), RdbcError> {
        if value < 0 {
            return Err(RdbcError::InvalidArgument(
                "max field size must be non-negative".to_owned(),
            ));
        }
        self.with_open(|state| state.max_field_size = value)
    }
    pub(crate) fn max_rows(&self) -> Result<i32, RdbcError> {
        self.with_open(|state| state.max_rows)
    }
    pub(crate) fn set_max_rows(&self, value: i32) -> Result<(), RdbcError> {
        if value < 0 {
            return Err(RdbcError::InvalidArgument(
                "max rows must be non-negative".to_owned(),
            ));
        }
        self.with_open(|state| state.max_rows = value)
    }
    pub(crate) fn set_escape_processing(&self, value: bool) -> Result<(), RdbcError> {
        self.with_open(|state| state.escape_processing = value)
    }
    pub(crate) fn query_timeout(&self) -> Result<i32, RdbcError> {
        self.with_open(|state| state.query_timeout)
    }
    pub(crate) fn set_query_timeout(&self, value: i32) -> Result<(), RdbcError> {
        if value < 0 {
            return Err(RdbcError::InvalidArgument(
                "query timeout must be non-negative".to_owned(),
            ));
        }
        self.with_open(|state| state.query_timeout = value)
    }
    pub(crate) fn cancel(&self) -> Result<(), RdbcError> {
        self.with_open(|_| ())
    }
    pub(crate) fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        self.with_open(|state| state.warnings.clone())
    }
    pub(crate) fn clear_warnings(&self) -> Result<(), RdbcError> {
        self.with_open(|state| state.warnings = None)
    }
    pub(crate) fn set_cursor_name(&self, value: &str) -> Result<(), RdbcError> {
        self.with_open(|state| state.cursor_name = Some(value.to_owned()))
    }
    pub(crate) fn set_fetch_direction(&self, value: i32) -> Result<(), RdbcError> {
        self.with_open(|state| state.fetch_direction = value)
    }
    pub(crate) fn fetch_direction(&self) -> Result<i32, RdbcError> {
        self.with_open(|state| state.fetch_direction)
    }
    pub(crate) fn set_fetch_size(&self, value: i32) -> Result<(), RdbcError> {
        if value < 0 {
            return Err(RdbcError::InvalidArgument(
                "fetch size must be non-negative".to_owned(),
            ));
        }
        self.with_open(|state| state.fetch_size = value)
    }
    pub(crate) fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.with_open(|state| state.fetch_size)
    }
    pub(crate) fn close_on_completion(&self) -> Result<(), RdbcError> {
        self.with_open(|state| state.close_on_completion = true)
    }
    pub(crate) fn is_close_on_completion(&self) -> Result<bool, RdbcError> {
        self.with_open(|state| state.close_on_completion)
    }
}
