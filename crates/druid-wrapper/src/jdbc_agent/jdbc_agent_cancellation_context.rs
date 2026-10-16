//! 同一 Agent session 的 Callable 取消身份与游标归属表。

use super::jdbc_agent_cancel_sender::JdbcAgentCancelSender;
use super::jdbc_agent_cancellation_slot::JdbcAgentCancellationSlot;
use druid_rdbc::error::RdbcError;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct ContextState {
    statements: HashMap<String, Arc<JdbcAgentCancellationSlot>>,
    cursors: HashMap<String, Arc<JdbcAgentCancellationSlot>>,
}

/// 将 statementId、cursorId 与唯一活动请求 ID 关联，供标准取消句柄使用。
/// 对应 Java: java.sql.Statement#cancel 的 session 资源归属。
pub(crate) struct JdbcAgentCancellationContext {
    session_id: String,
    sender: Arc<dyn JdbcAgentCancelSender>,
    state: Mutex<ContextState>,
}

impl JdbcAgentCancellationContext {
    pub(crate) fn new(session_id: String, sender: Arc<dyn JdbcAgentCancelSender>) -> Arc<Self> {
        Arc::new(Self {
            session_id,
            sender,
            state: Mutex::new(ContextState {
                statements: HashMap::new(),
                cursors: HashMap::new(),
            }),
        })
    }

    pub(crate) fn register(&self, statement_id: &str) -> Arc<JdbcAgentCancellationSlot> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let slot = JdbcAgentCancellationSlot::new();
        state
            .statements
            .insert(statement_id.to_owned(), Arc::clone(&slot));
        slot
    }

    pub(crate) fn slot_for_request(
        &self,
        operation: &str,
        request_key: Option<&str>,
    ) -> Option<Arc<JdbcAgentCancellationSlot>> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match operation {
            "execute_callable" | "next_callable_result" => {
                request_key.and_then(|id| state.statements.get(id)).cloned()
            }
            "fetch_page" => request_key.and_then(|id| state.cursors.get(id)).cloned(),
            _ => None,
        }
    }

    pub(crate) fn observe_response(
        &self,
        operation: &str,
        request_key: Option<&str>,
        response: &JsonValue,
    ) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match operation {
            "execute_callable" | "next_callable_result" => {
                let Some(statement_id) = request_key else {
                    return;
                };
                let Some(slot) = state.statements.get(statement_id).cloned() else {
                    return;
                };
                state
                    .cursors
                    .retain(|_, cursor_slot| !Arc::ptr_eq(cursor_slot, &slot));
                if let Some(cursor_id) = response.get("cursorId").and_then(JsonValue::as_str) {
                    state.cursors.insert(cursor_id.to_owned(), slot);
                }
            }
            "fetch_page" => {
                if response.get("hasMore").and_then(JsonValue::as_bool) == Some(false) {
                    if let Some(cursor_id) = request_key {
                        state.cursors.remove(cursor_id);
                    }
                }
            }
            "close_cursor" => {
                if let Some(cursor_id) = request_key {
                    state.cursors.remove(cursor_id);
                }
            }
            "close_statement" => {
                if let Some(statement_id) = request_key {
                    if let Some(slot) = state.statements.remove(statement_id) {
                        slot.close();
                        state
                            .cursors
                            .retain(|_, cursor_slot| !Arc::ptr_eq(cursor_slot, &slot));
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) async fn cancel_request(&self, request_id: u64) -> Result<(), RdbcError> {
        self.sender
            .cancel_request(&self.session_id, request_id)
            .await
    }

    pub(crate) fn close(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for slot in state.statements.values() {
            slot.close();
        }
        state.statements.clear();
        state.cursors.clear();
    }
}
