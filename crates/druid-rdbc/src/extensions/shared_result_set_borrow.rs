use super::shared_result_set_state::SharedResultSetState;
use crate::result_set::ResultSet;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// 标准共享结果集的独占拥有式借用。
///
/// 对应 Java: java.sql.ResultSet；原 RdbcResultSet#physical 的受保护后端访问。
/// 只暴露标准接口，不能替换 Box；释放借用时归还原后端，包括异步调用取消路径。
pub struct SharedResultSetBorrow {
    state: Arc<SharedResultSetState>,
    backend: Option<Box<dyn ResultSet>>,
}

impl SharedResultSetBorrow {
    pub(crate) fn new(state: Arc<SharedResultSetState>, backend: Box<dyn ResultSet>) -> Self {
        Self {
            state,
            backend: Some(backend),
        }
    }
}

impl Deref for SharedResultSetBorrow {
    type Target = dyn ResultSet;

    fn deref(&self) -> &Self::Target {
        self.backend
            .as_deref()
            .expect("live result set borrow always owns its backend")
    }
}

impl DerefMut for SharedResultSetBorrow {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.backend
            .as_deref_mut()
            .expect("live result set borrow always owns its backend")
    }
}

impl Drop for SharedResultSetBorrow {
    fn drop(&mut self) {
        if let Some(backend) = self.backend.take() {
            self.state.restore(backend);
        }
    }
}
