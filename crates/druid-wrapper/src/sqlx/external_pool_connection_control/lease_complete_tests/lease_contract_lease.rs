use druid_rdbc::connection::Connection;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
/// 拥有一个标准连接并在真实租约析构时增加一次观察计数；对应原 ContractLease。
pub(super) struct ContractLease {
    pub(super) connection: Box<dyn Connection>,
    pub(super) drop_count: Arc<AtomicUsize>,
}
impl Deref for ContractLease {
    type Target = Box<dyn Connection>;
    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}
impl DerefMut for ContractLease {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}
impl Drop for ContractLease {
    fn drop(&mut self) {
        self.drop_count.fetch_add(1, Ordering::Relaxed);
    }
}
