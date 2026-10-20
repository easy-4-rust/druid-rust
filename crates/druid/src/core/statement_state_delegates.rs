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

        fn cancellation_control(
            &self,
        ) -> Result<
            std::sync::Arc<dyn druid_rdbc::statement::StatementCancellation>,
            druid_rdbc::error::RdbcError,
        > {
            druid_rdbc::statement::Statement::cancellation_control(&self.$base)
        }

        fn set_cursor_name(&mut self, name: &str) -> Result<(), druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::set_cursor_name(&mut self.$base, name)
        }

        fn set_cursor_name_nullable(
            &mut self,
            name: Option<&str>,
        ) -> Result<(), druid_rdbc::error::RdbcError> {
            druid_rdbc::statement::Statement::set_cursor_name_nullable(&mut self.$base, name)
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

#[cfg(test)]
mod cancellation_control_tests {
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::statement::{Statement, StatementCancellation};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct CancellationCounter(Arc<AtomicUsize>);

    #[async_trait::async_trait]
    impl StatementCancellation for CancellationCounter {
        async fn cancel(&self) -> Result<(), RdbcError> {
            self.0.fetch_add(1, Ordering::AcqRel);
            Ok(())
        }
    }

    struct CancellationTarget(Arc<AtomicUsize>);

    druid_rdbc::impl_standard_wrapper!(CancellationTarget => Statement);

    #[async_trait::async_trait]
    impl Statement for CancellationTarget {
        fn cancellation_control(&self) -> Result<Arc<dyn StatementCancellation>, RdbcError> {
            Ok(Arc::new(CancellationCounter(Arc::clone(&self.0))))
        }
    }

    struct DelegatingStatement {
        statement_base: CancellationTarget,
    }

    druid_rdbc::impl_standard_wrapper!(DelegatingStatement => Statement);

    #[async_trait::async_trait]
    impl Statement for DelegatingStatement {
        delegate_statement_state_methods!(statement_base);
    }

    #[tokio::test]
    async fn prepared_and_callable_state_delegate_macro_forwards_cancellation_control() {
        let calls = Arc::new(AtomicUsize::new(0));
        let statement = DelegatingStatement {
            statement_base: CancellationTarget(Arc::clone(&calls)),
        };

        let control = Statement::cancellation_control(&statement)
            .expect("pooled prepared/callable facade must expose its target control");

        control.cancel().await.unwrap();
        assert_eq!(calls.load(Ordering::Acquire), 1);
    }
}
