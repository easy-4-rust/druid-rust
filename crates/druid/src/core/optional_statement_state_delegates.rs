macro_rules! delegate_optional_statement_state_methods {
    ($first:ident $(.$rest:ident)*, $busy:literal) => {
        fn options(&self) -> Result<druid_rdbc::statement::StatementOptions, druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_ref().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.options()
        }
        fn result_set_type(&self) -> Result<druid_rdbc::statement::ResultSetType, druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_ref().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.result_set_type()
        }

        fn result_set_concurrency(&self) -> Result<druid_rdbc::statement::ResultSetConcurrency, druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_ref().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.result_set_concurrency()
        }

        fn result_set_holdability(&self) -> Result<druid_rdbc::statement::ResultSetHoldability, druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_ref().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.result_set_holdability()
        }

        fn max_field_size(&self) -> Result<i32, druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_ref().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.max_field_size()
        }
        fn set_max_field_size(&mut self, max: i32) -> Result<(), druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_mut().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.set_max_field_size(max)
        }
        fn max_rows(&self) -> Result<i64, druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_ref().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.max_rows()
        }
        fn set_max_rows(&mut self, max: i64) -> Result<(), druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_mut().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.set_max_rows(max)
        }
        fn set_escape_processing(&mut self, enabled: bool) -> Result<(), druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_mut().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.set_escape_processing(enabled)
        }
        fn cancel(&mut self) -> Result<(), druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_mut().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.cancel()
        }
        fn set_cursor_name(&mut self, name: &str) -> Result<(), druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_mut().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.set_cursor_name(name)
        }
        fn set_cursor_name_nullable(&mut self, name: Option<&str>) -> Result<(), druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_mut().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.set_cursor_name_nullable(name)
        }
        fn set_fetch_direction(&mut self, direction: druid_rdbc::statement::FetchDirection) -> Result<(), druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_mut().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.set_fetch_direction(direction)
        }
        fn fetch_direction(&self) -> Result<druid_rdbc::statement::FetchDirection, druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_ref().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.fetch_direction()
        }
        fn set_fetch_size(&mut self, rows: i32) -> Result<(), druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_mut().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.set_fetch_size(rows)
        }
        fn fetch_size(&self) -> Result<i32, druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_ref().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.fetch_size()
        }
        fn set_poolable(&mut self, poolable: bool) -> Result<(), druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_mut().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.set_poolable(poolable)
        }
        fn is_poolable(&self) -> Result<bool, druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_ref().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.is_poolable()
        }
        fn close_on_completion(&mut self) -> Result<(), druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_mut().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.close_on_completion()
        }
        fn is_close_on_completion(&self) -> Result<bool, druid_rdbc::error::RdbcError> {
            self.ensure_open()?;
            self.$first$(.$rest)*.try_lock().map_err(|_| druid_rdbc::error::RdbcError::Other($busy.to_owned()))?
                .as_ref().ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?.is_close_on_completion()
        }
    };
}

pub(crate) use delegate_optional_statement_state_methods;
