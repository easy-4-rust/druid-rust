use super::shared_result_set_borrow::SharedResultSetBorrow;
use super::shared_result_set_state::SharedResultSetState;
use crate::error::RdbcError;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

/// 不绑定执行器的标准结果集借用等待。
/// 对应 Java: java.sql.ResultSet；共享后端互斥访问的异步平台适配。
pub(crate) struct SharedResultSetBorrowFuture {
    state: Arc<SharedResultSetState>,
    waiter: Option<u64>,
}

impl SharedResultSetBorrowFuture {
    pub(crate) fn new(state: Arc<SharedResultSetState>) -> Self {
        Self {
            state,
            waiter: None,
        }
    }
}

impl Future for SharedResultSetBorrowFuture {
    type Output = Result<SharedResultSetBorrow, RdbcError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        this.state.poll_borrow(&mut this.waiter, context)
    }
}

impl Drop for SharedResultSetBorrowFuture {
    fn drop(&mut self) {
        if let Some(waiter) = self.waiter.take() {
            self.state.cancel_waiter(waiter);
        }
    }
}
