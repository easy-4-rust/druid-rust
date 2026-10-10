use super::delegation_counts::DelegationCounts;
use super::delegation_resources::DelegationResources;
use super::lease_meta_data::LeaseMetaData;
use super::lease_statement_probe::LeaseStatementProbe;
use druid_rdbc::connection::{Connection, TransactionIsolation};
use druid_rdbc::error::{RdbcError, SqlWarning};
use druid_rdbc::extensions::ConnectionCapabilities;
use druid_rdbc::metadata::DatabaseMetaData;
use druid_rdbc::resource::{Blob, Clob, NClob};
use druid_rdbc::statement::{
    CallableStatement, ExecResult, GeneratedKeys, PreparedStatement, ResultSetHoldability,
    Statement, StatementOptions,
};
use druid_rdbc::types::{Row, Savepoint, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;
/// 保存固定原案例的连接状态与实际委托资源，验证外部池租约的标准委托合同。
pub(super) struct ContractConnection {
    pub(super) closed: bool,
    pub(super) auto_commit: bool,
    pub(super) read_only: bool,
    pub(super) isolation: TransactionIsolation,
    pub(super) holdability: ResultSetHoldability,
    pub(super) catalog: String,
    pub(super) schema: String,
    pub(super) counts: Arc<DelegationCounts>,
    pub(super) resources: DelegationResources,
}
druid_rdbc::impl_standard_wrapper!(ContractConnection => Connection);
#[async_trait::async_trait]
impl Connection for ContractConnection {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        assert_eq!(sql, "UPDATE contract");
        assert_eq!(params, vec![Value::Int(7)]);
        Ok(ExecResult {
            rows_affected: 1,
            last_insert_id: Some(9),
            row_count: None,
        })
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        assert_eq!(sql, "SELECT contract");
        assert_eq!(params, vec![Value::String("value".to_string())]);
        Ok(vec![Row::new(vec![Value::Bool(true)])])
    }

    async fn exec_batch(
        &mut self,
        batch: Vec<(String, Vec<Value>)>,
    ) -> Result<Vec<i64>, RdbcError> {
        assert_eq!(
            batch,
            vec![("UPDATE delegated batch".to_string(), vec![Value::Int(11)])]
        );
        self.counts.exec_batch.fetch_add(1, Ordering::Relaxed);
        Ok(vec![11])
    }

    async fn begin(&mut self) -> Result<(), RdbcError> {
        self.auto_commit = false;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        self.auto_commit = true;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        self.auto_commit = true;
        Ok(())
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), RdbcError> {
        assert_eq!(savepoint.id, 1);
        Ok(())
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, RdbcError> {
        Ok(Savepoint { id: 1, name: None })
    }

    async fn set_savepoint_named(&mut self, name: &str) -> Result<Savepoint, RdbcError> {
        Ok(Savepoint {
            id: 2,
            name: Some(name.to_string()),
        })
    }

    async fn release_savepoint(&mut self, savepoint: &Savepoint) -> Result<(), RdbcError> {
        assert_eq!(savepoint.name.as_deref(), Some("named"));
        Ok(())
    }

    async fn abort(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }

    async fn create_blob(&mut self) -> Result<Box<dyn Blob>, RdbcError> {
        self.counts.create_blob.fetch_add(1, Ordering::Relaxed);
        Ok(self
            .resources
            .blob
            .take()
            .expect("same preallocated resource transferred once"))
    }

    async fn create_clob(&mut self) -> Result<Box<dyn Clob>, RdbcError> {
        self.counts.create_clob.fetch_add(1, Ordering::Relaxed);
        Ok(self
            .resources
            .clob
            .take()
            .expect("same preallocated resource transferred once"))
    }

    async fn create_n_clob(&mut self) -> Result<Box<dyn NClob>, RdbcError> {
        self.counts.create_n_clob.fetch_add(1, Ordering::Relaxed);
        Ok(self
            .resources
            .n_clob
            .take()
            .expect("same preallocated resource transferred once"))
    }

    fn auto_commit(&self) -> Result<bool, RdbcError> {
        Ok(self.auto_commit)
    }

    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), RdbcError> {
        self.auto_commit = auto_commit;
        Ok(())
    }

    fn read_only(&self) -> Result<bool, RdbcError> {
        Ok(self.read_only)
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), RdbcError> {
        self.read_only = read_only;
        Ok(())
    }

    fn transaction_isolation(&self) -> Result<TransactionIsolation, RdbcError> {
        Ok(self.isolation)
    }

    async fn set_transaction_isolation(
        &mut self,
        level: TransactionIsolation,
    ) -> Result<(), RdbcError> {
        self.isolation = level;
        Ok(())
    }

    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        Ok(self.holdability)
    }

    async fn set_holdability(
        &mut self,
        holdability: ResultSetHoldability,
    ) -> Result<(), RdbcError> {
        self.holdability = holdability;
        Ok(())
    }

    async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        Ok(Some(SqlWarning::new(
            "lease-warning",
            Some("01000".to_string()),
            7,
        )))
    }

    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        Ok(Some(self.catalog.clone()))
    }

    async fn set_catalog(&mut self, catalog: Option<&str>) -> Result<(), RdbcError> {
        self.catalog = catalog.expect("original non-null catalog").to_string();
        Ok(())
    }

    fn schema(&self) -> Result<Option<String>, RdbcError> {
        Ok(Some(self.schema.clone()))
    }

    async fn set_schema(&mut self, schema: Option<&str>) -> Result<(), RdbcError> {
        self.schema = schema.expect("original non-null schema").to_string();
        Ok(())
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("lease-contract".to_owned())
    }
    async fn create_statement(
        &mut self,
        options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        assert_eq!(options, StatementOptions::default());
        self.counts
            .create_physical_statement
            .fetch_add(1, Ordering::Relaxed);
        Ok(self
            .resources
            .statement
            .take()
            .expect("same preallocated statement transferred once"))
    }
    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        assert_eq!(options, StatementOptions::default());
        if sql == "UPDATE delegated prepared" {
            self.counts
                .prepare_physical_statement
                .fetch_add(1, Ordering::Relaxed);
            Ok(self
                .resources
                .prepared_statement
                .take()
                .expect("same preallocated prepared transferred once"))
        } else {
            assert!(matches!(
                sql,
                "UPDATE contract" | "SELECT contract" | "EXECUTE contract"
            ));
            Ok(Box::new(LeaseStatementProbe::new(
                Some(sql.to_owned()),
                options,
                GeneratedKeys::None,
                Arc::clone(&self.counts),
            )))
        }
    }
    async fn prepare_statement_with_generated_keys(
        &mut self,
        sql: &str,
        options: StatementOptions,
        keys: GeneratedKeys,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        assert_eq!(sql, "EXECUTE contract");
        assert_eq!(options, StatementOptions::default());
        assert_eq!(keys, GeneratedKeys::ReturnGeneratedKeys);
        Ok(Box::new(LeaseStatementProbe::new(
            Some(sql.to_owned()),
            options,
            keys,
            Arc::clone(&self.counts),
        )))
    }
    async fn prepare_call(
        &mut self,
        sql: &str,
        options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        assert_eq!(sql, "CALL delegated");
        assert_eq!(options, StatementOptions::default());
        self.counts
            .prepare_physical_call
            .fetch_add(1, Ordering::Relaxed);
        Ok(self
            .resources
            .callable_statement
            .take()
            .expect("same preallocated callable transferred once"))
    }
    async fn database_meta_data(&mut self) -> Result<Box<dyn DatabaseMetaData>, RdbcError> {
        self.counts
            .database_meta_data
            .fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(LeaseMetaData))
    }
    fn capabilities(&self) -> ConnectionCapabilities {
        ConnectionCapabilities {
            transactions: true,
            savepoints: true,
            catalogs: true,
            schemas: true,
            callable_statements: true,
        }
    }
}
