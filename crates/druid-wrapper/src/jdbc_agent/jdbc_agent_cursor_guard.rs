//! Callable 分页游标在异步执行取消时的远端清理守卫。

use crate::standard_adapter_executor::StandardExecutor;
use druid_rdbc::error::RdbcError;
use serde_json::json;
use std::sync::Arc;

/// 保存当前远端游标身份；Future 被丢弃时安排关闭。
/// 对应 Java: java.sql.ResultSet#close 的会话资源释放。
pub(crate) struct JdbcAgentCursorGuard {
    executor: Arc<dyn StandardExecutor>,
    cursor_id: Option<String>,
    runtime: Option<tokio::runtime::Handle>,
}

impl JdbcAgentCursorGuard {
    pub(crate) fn new(executor: Arc<dyn StandardExecutor>, cursor_id: Option<String>) -> Self {
        Self {
            executor,
            cursor_id,
            runtime: tokio::runtime::Handle::try_current().ok(),
        }
    }

    pub(crate) fn disarm(&mut self) {
        self.cursor_id.take();
    }

    pub(crate) async fn close_now(&mut self) -> Result<(), RdbcError> {
        if let Some(cursor_id) = self.cursor_id.clone() {
            self.executor
                .connection_control("close_cursor", json!({"cursorId": cursor_id}))
                .await?;
            self.cursor_id.take();
        }
        Ok(())
    }
}

impl Drop for JdbcAgentCursorGuard {
    fn drop(&mut self) {
        let Some(cursor_id) = self.cursor_id.take() else {
            return;
        };
        if let Some(runtime) = &self.runtime {
            let executor = Arc::clone(&self.executor);
            runtime.spawn(async move {
                let _ = executor
                    .connection_control("close_cursor", json!({"cursorId": cursor_id}))
                    .await;
            });
        }
    }
}
