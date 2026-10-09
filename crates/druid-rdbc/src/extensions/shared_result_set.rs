use crate::error::RdbcError;
use crate::result_set::ResultSet;
use std::fmt;
use std::sync::Arc;

/// 共享同一个标准结果集后端的具体转发句柄。
///
/// 对应 Java: java.sql.ResultSet；迁移自旧具体句柄 RdbcResultSet。
/// 克隆保持后端身份，互斥借用支持标准可变游标而不增加第二套结果集接口。
/// 本句柄转发关闭调用；驱动负责自身关闭幂等，Druid 所有者负责池化资源清理。
#[derive(Clone)]
pub struct SharedResultSet {
    backend: Arc<SharedResultSetState>,
}

impl SharedResultSet {
    /// 包装指定后端；参数为唯一拥有的标准结果集，返回可克隆的共享句柄。
    /// 对应 Java: java.sql.ResultSet；原 RdbcResultSet#new。
    pub fn new(backend: Box<dyn ResultSet>) -> Self {
        Self {
            backend: Arc::new(SharedResultSetState::new(backend)),
        }
    }

    /// 借用同一个标准后端；无参数，返回可变借用或借用冲突错误。
    /// 借用只暴露接口本身，调用者不能替换拥有的 Box 或改变共享后端身份。
    /// 对应 Java: java.sql.ResultSet；原 RdbcResultSet#physical。
    pub fn try_result_set(&self) -> Result<SharedResultSetBorrow, RdbcError> {
        self.backend.try_borrow()
    }

    /// 等待借用释放后关闭实际后端；无参数，返回原始驱动关闭结果。
    /// 每次调用都转发，不缓存失败或伪造关闭状态。取消时归还原后端，
    /// 驱动关闭自身的业务取消安全仍由后端所有者提供。
    /// 对应 Java: java.sql.ResultSet#close；原 RdbcResultSet#close。
    pub async fn close(&self) -> Result<(), RdbcError> {
        let mut backend = SharedResultSetBorrowFuture::new(Arc::clone(&self.backend)).await?;
        backend.close().await
    }

    /// 查询实际后端是否关闭；无参数，返回驱动状态或借用冲突错误。
    /// 对应 Java: java.sql.ResultSet#isClosed；原 RdbcResultSet#is_closed。
    pub fn is_closed(&self) -> Result<bool, RdbcError> {
        self.try_result_set()?.is_closed()
    }
}

impl fmt::Debug for SharedResultSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("SharedResultSet");
        match self.try_result_set() {
            Ok(backend) => {
                debug.field("backend", &std::ptr::from_ref(&*backend));
                debug.field("closed", &backend.is_closed());
            }
            Err(_) => {
                // 格式化不能等待调用者自己持有的借用，也不能把冲突解释为关闭。
                debug.field("borrowed", &true);
            }
        }
        debug.finish_non_exhaustive()
    }
}

impl PartialEq for SharedResultSet {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.backend, &other.backend)
    }
}

impl Eq for SharedResultSet {}
use super::shared_result_set_borrow::SharedResultSetBorrow;
use super::shared_result_set_borrow_future::SharedResultSetBorrowFuture;
use super::shared_result_set_state::SharedResultSetState;
