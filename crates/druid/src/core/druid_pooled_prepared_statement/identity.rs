use super::DruidPooledPreparedStatement;

druid_rdbc::impl_standard_wrapper!(DruidPooledPreparedStatement => PreparedStatement);

impl std::fmt::Debug for DruidPooledPreparedStatement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidPooledPreparedStatement")
            .field("sql", &self.sql)
            .field("statement_base", &self.statement_base)
            .field("cached", &self.cache.is_some())
            .field("closed", &self.is_closed())
            .finish_non_exhaustive()
    }
}
