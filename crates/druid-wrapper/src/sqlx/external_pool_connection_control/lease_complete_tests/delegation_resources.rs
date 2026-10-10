use super::delegation_counts::DelegationCounts;
use super::lease_blob_access::LeaseBlobAccess;
use super::lease_clob_access::LeaseClobAccess;
use super::lease_statement_probe::LeaseStatementProbe;
use druid_rdbc::resource::{Blob, Clob, NClob};
use druid_rdbc::statement::{
    CallableStatement, GeneratedKeys, PreparedStatement, Statement, StatementOptions,
};
use std::sync::Arc;
/// 保存待移动的原对象与可克隆同身份 LOB；对应原 DelegationResources。
pub(super) struct DelegationResources {
    pub(super) statement: Option<Box<dyn Statement>>,
    pub(super) prepared_statement: Option<Box<dyn PreparedStatement>>,
    pub(super) callable_statement: Option<Box<dyn CallableStatement>>,
    pub(super) blob: Option<Box<dyn Blob>>,
    pub(super) clob: Option<Box<dyn Clob>>,
    pub(super) n_clob: Option<Box<dyn NClob>>,
}
impl DelegationResources {
    /// 预先分配三个实际 owned 对象；返回可只移动一次的真实资源集合。
    pub(super) fn new(counts: Arc<DelegationCounts>) -> Self {
        Self {
            statement: Some(Box::new(LeaseStatementProbe::new(
                None,
                StatementOptions::default(),
                GeneratedKeys::None,
                Arc::clone(&counts),
            ))),
            prepared_statement: Some(Box::new(LeaseStatementProbe::new(
                Some("UPDATE delegated prepared".into()),
                StatementOptions::default(),
                GeneratedKeys::None,
                Arc::clone(&counts),
            ))),
            callable_statement: Some(Box::new(LeaseStatementProbe::new(
                Some("CALL delegated".into()),
                StatementOptions::default(),
                GeneratedKeys::None,
                counts,
            ))),
            blob: Some(Box::new(LeaseBlobAccess::default())),
            clob: Some(Box::new(LeaseClobAccess::default())),
            n_clob: Some(Box::new(LeaseClobAccess::default())),
        }
    }
    /// 返回预分配 Statement 的实际数据地址，用于观察移动前后身份。
    pub(super) fn statement_address(&self) -> usize {
        std::ptr::from_ref::<dyn Statement>(self.statement.as_deref().unwrap()).cast::<()>()
            as usize
    }
    /// 返回预分配 PreparedStatement 的实际数据地址。
    pub(super) fn prepared_address(&self) -> usize {
        std::ptr::from_ref::<dyn PreparedStatement>(self.prepared_statement.as_deref().unwrap())
            .cast::<()>() as usize
    }
    /// 返回预分配 CallableStatement 的实际数据地址。
    pub(super) fn callable_address(&self) -> usize {
        std::ptr::from_ref::<dyn CallableStatement>(self.callable_statement.as_deref().unwrap())
            .cast::<()>() as usize
    }
}
