use super::DruidPooledCallableStatement;

druid_rdbc::impl_standard_wrapper!(DruidPooledCallableStatement => CallableStatement);

impl std::fmt::Debug for DruidPooledCallableStatement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidPooledCallableStatement")
            .field("sql", &self.sql)
            .field("statement_base", &self.statement_base)
            .field("closed", &self.is_closed())
            .finish_non_exhaustive()
    }
}
