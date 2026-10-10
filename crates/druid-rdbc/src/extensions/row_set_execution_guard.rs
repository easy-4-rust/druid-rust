use super::row_set_state::RowSetState;
use std::sync::{Arc, Mutex};

/// 执行操作 token 的同步取消收尾；不执行数据库回滚或关闭调用者连接。
/// 对应 Java: RowSet#execute 与 RowSetWriter#writeData 的 Rust future Drop 适配。
pub(crate) struct RowSetExecutionGuard {
    pub(crate) owner: Arc<Mutex<RowSetState>>,
    pub(crate) token: u64,
}
impl Drop for RowSetExecutionGuard {
    fn drop(&mut self) {
        let mut state = self
            .owner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.busy == Some(self.token) {
            state.busy = None;
        }
    }
}
