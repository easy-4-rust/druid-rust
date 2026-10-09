use super::delegation_counts::DelegationCounts;
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{
    CallableStatement, ExecutionResult, GeneratedKeys, ParameterIndex, PreparedParameter,
    PreparedStatement, Statement, StatementOptions,
};
use druid_rdbc::types::{Row, Value};
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// 保存实际参数、批次及关闭状态的标准语句合同夹具。
/// 对应固定 Java/Rust 迁移来源：physical_connection_lease_test 的 SqlTextStatement/SqlTextPreparedStatement 合同。
pub(super) struct LeaseStatementProbe {
    sql: Option<String>,
    options: StatementOptions,
    generated_keys: GeneratedKeys,
    parameters: BTreeMap<usize, Value>,
    batches: Vec<Vec<PreparedParameter>>,
    closed: bool,
    last_generated_key: Option<i64>,
    counts: Arc<DelegationCounts>,
}
impl LeaseStatementProbe {
    /// 创建实际拥有状态的语句；sql 为预备 SQL，counts 为真实调用计数。
    pub(super) fn new(
        sql: Option<String>,
        options: StatementOptions,
        generated_keys: GeneratedKeys,
        counts: Arc<DelegationCounts>,
    ) -> Self {
        Self {
            sql,
            options,
            generated_keys,
            parameters: BTreeMap::new(),
            batches: Vec::new(),
            closed: false,
            last_generated_key: None,
            counts,
        }
    }
    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.closed {
            Err(RdbcError::Other("statement is closed".into()))
        } else {
            Ok(())
        }
    }
    fn values(&self) -> Vec<Value> {
        self.parameters.values().cloned().collect()
    }
    fn query(&self, sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        let values = self.values();
        let rows = if sql == "UPDATE delegated prepared" {
            assert_eq!(values, vec![Value::Int(13)]);
            self.counts.fetch_prepared.fetch_add(1, Ordering::Relaxed);
            vec![Row::new(vec![Value::Int(13)])]
        } else {
            assert_eq!(sql, "SELECT contract");
            assert_eq!(values, vec![Value::String("value".into())]);
            vec![Row::new(vec![Value::Bool(true)])]
        };
        Ok(Box::new(BufferedRowSet::new(rows)))
    }
    fn update(&self, sql: &str, values: Vec<Value>) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        if sql == "UPDATE delegated prepared" {
            assert_eq!(values, vec![Value::Int(12)]);
            self.counts.exec_prepared.fetch_add(1, Ordering::Relaxed);
            Ok(12)
        } else {
            assert_eq!(sql, "UPDATE contract");
            assert_eq!(values, vec![Value::Int(7)]);
            Ok(1)
        }
    }
    fn generic(
        &mut self,
        sql: &str,
        generated_keys: &GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.ensure_open()?;
        assert_eq!(sql, "EXECUTE contract");
        assert_eq!(self.values(), vec![Value::Int(8)]);
        assert_eq!(generated_keys, &GeneratedKeys::ReturnGeneratedKeys);
        self.last_generated_key = Some(10);
        Ok(vec![ExecutionResult::UpdateCount(3)])
    }
}
druid_rdbc::impl_standard_wrapper!(LeaseStatementProbe => CallableStatement);
#[async_trait::async_trait]
impl Statement for LeaseStatementProbe {
    fn options(&self) -> Result<StatementOptions, RdbcError> {
        self.ensure_open()?;
        Ok(self.options)
    }
    async fn execute_query(&mut self, sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.query(sql)
    }
    async fn execute_update(
        &mut self,
        sql: &str,
        _generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        self.update(sql, self.values())
    }
    async fn execute(
        &mut self,
        sql: &str,
        generated_keys: GeneratedKeys,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        self.generic(sql, &generated_keys)
    }
    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        let rows = self
            .last_generated_key
            .into_iter()
            .map(|key| Row::new(vec![Value::Int(key)]))
            .collect();
        Ok(Box::new(BufferedRowSet::new(rows)))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}
#[async_trait::async_trait]
impl PreparedStatement for LeaseStatementProbe {
    fn sql(&self) -> Result<&str, RdbcError> {
        self.ensure_open()?;
        self.sql
            .as_deref()
            .ok_or_else(|| RdbcError::InvalidArgument("not prepared".into()))
    }
    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        self.ensure_open()?;
        assert_eq!(parameter.index.get(), 1);
        self.parameters
            .insert(parameter.index.get(), parameter.value);
        Ok(())
    }
    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.parameters.clear();
        Ok(())
    }
    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.batches.clear();
        Ok(())
    }
    fn add_parameter_batch(&mut self, parameters: Vec<PreparedParameter>) -> Result<(), RdbcError> {
        self.ensure_open()?;
        assert_eq!(parameters.len(), 1);
        assert_eq!(parameters[0].index, ParameterIndex::new(1).unwrap());
        self.batches.push(parameters);
        Ok(())
    }
    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.query(self.sql()?)
    }
    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        self.update(self.sql()?, self.values())
    }
    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        let sql = self.sql()?.to_owned();
        let generated_keys = self.generated_keys.clone();
        self.generic(&sql, &generated_keys)
    }
    async fn execute_prepared_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.ensure_open()?;
        assert_eq!(self.batches.len(), 1);
        self.batches
            .iter()
            .map(|parameters| {
                self.update(
                    self.sql()?,
                    parameters
                        .iter()
                        .map(|parameter| parameter.value.clone())
                        .collect(),
                )
            })
            .collect()
    }
}
#[async_trait::async_trait]
impl CallableStatement for LeaseStatementProbe {}
