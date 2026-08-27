//! Druid prepared/callable Statement-supertrait forwarding.

macro_rules! delegate_statement_state_methods {
    ($base:ident) => {
        fn options(
            &self,
        ) -> Result<druid_rdbc::statement::StatementOptions, druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::options(&self.$base)
        }

        fn max_field_size(&self) -> Result<i32, druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::max_field_size(&self.$base)
        }

        fn set_max_field_size(&mut self, max: i32) -> Result<(), druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::set_max_field_size(&mut self.$base, max)
        }

        fn max_rows(&self) -> Result<i64, druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::max_rows(&self.$base)
        }

        fn set_max_rows(&mut self, max: i64) -> Result<(), druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::set_max_rows(&mut self.$base, max)
        }

        fn set_escape_processing(
            &mut self,
            enabled: bool,
        ) -> Result<(), druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::set_escape_processing(&mut self.$base, enabled)
        }

        fn cancel(&mut self) -> Result<(), druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::cancel(&mut self.$base)
        }

        fn set_cursor_name(&mut self, name: &str) -> Result<(), druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::set_cursor_name(&mut self.$base, name)
        }

        fn set_fetch_direction(
            &mut self,
            direction: druid_rdbc::statement::FetchDirection,
        ) -> Result<(), druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::set_fetch_direction(&mut self.$base, direction)
        }

        fn fetch_direction(
            &self,
        ) -> Result<druid_rdbc::statement::FetchDirection, druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::fetch_direction(&self.$base)
        }

        fn set_fetch_size(&mut self, rows: i32) -> Result<(), druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::set_fetch_size(&mut self.$base, rows)
        }

        fn fetch_size(&self) -> Result<i32, druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::fetch_size(&self.$base)
        }

        fn set_poolable(&mut self, poolable: bool) -> Result<(), druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::set_poolable(&mut self.$base, poolable)
        }

        fn is_poolable(&self) -> Result<bool, druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::is_poolable(&self.$base)
        }

        fn close_on_completion(&mut self) -> Result<(), druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::close_on_completion(&mut self.$base)
        }

        fn is_close_on_completion(&self) -> Result<bool, druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::is_close_on_completion(&self.$base)
        }
    };
}

pub(crate) use delegate_statement_state_methods;
