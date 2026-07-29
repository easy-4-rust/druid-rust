use serde_json::{Map, Value};

/// Java `WebAppStatManager` 与 `SpringStatManager` 的 Rust 管理快照 SPI。
pub trait AdminStatProvider: Send + Sync {
    /// 返回 Web 应用统计列表。
    fn web_app_stats(&self) -> Vec<Map<String, Value>>;
    /// 返回 Web URI 统计列表。
    fn web_uri_stats(&self) -> Vec<Map<String, Value>>;
    /// 返回指定 URI 统计。
    fn web_uri_stat(&self, uri: &str) -> Option<Map<String, Value>>;
    /// 返回 Web session 统计列表。
    fn web_session_stats(&self) -> Vec<Map<String, Value>>;
    /// 返回指定 session 统计。
    fn web_session_stat(&self, session_id: &str) -> Option<Map<String, Value>>;
    /// 返回 Spring/Rust 集成方法统计列表。
    fn method_stats(&self) -> Vec<Map<String, Value>>;
    /// 返回指定类与方法统计。
    fn method_stat(&self, class: &str, method: &str) -> Option<Map<String, Value>>;
}
