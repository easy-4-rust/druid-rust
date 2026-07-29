use serde_json::{Map, Value};

use super::AdminStatProvider;

/// 未接入 Web/集成层统计时的空实现。
#[derive(Clone, Copy, Debug, Default)]
pub struct EmptyAdminStatProvider;

impl AdminStatProvider for EmptyAdminStatProvider {
    fn web_app_stats(&self) -> Vec<Map<String, Value>> {
        Vec::new()
    }

    fn web_uri_stats(&self) -> Vec<Map<String, Value>> {
        Vec::new()
    }

    fn web_uri_stat(&self, _uri: &str) -> Option<Map<String, Value>> {
        None
    }

    fn web_session_stats(&self) -> Vec<Map<String, Value>> {
        Vec::new()
    }

    fn web_session_stat(&self, _session_id: &str) -> Option<Map<String, Value>> {
        None
    }

    fn method_stats(&self) -> Vec<Map<String, Value>> {
        Vec::new()
    }

    fn method_stat(&self, _class: &str, _method: &str) -> Option<Map<String, Value>> {
        None
    }
}
