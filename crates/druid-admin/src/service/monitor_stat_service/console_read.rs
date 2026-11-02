//! 控制台读取保留原始字段存在性与逐节点失败；Java 兼容聚合保持原语义。
use super::{node_url, HttpUtil, MonitorStatService};
use serde_json::{json, Value};

impl MonitorStatService {
    /// 读取控制台所需的固定管理资源；返回原始节点内容与失败状态。
    /// 参数 path 由版本化 API 常量提供；成功为空与请求失败严格区分。
    pub(crate) async fn console_read(&self, path: &str) -> Value {
        let mut observations = Vec::new();
        let mut failures = Vec::new();
        let nodes = match self.get_all_service_node_map().await {
            Ok(nodes) => nodes,
            Err(_) => return json!({"observations": [], "failures": [{"reason": "服务发现失败"}]}),
        };
        for node in nodes.values() {
            let url = node_url(node, path);
            match HttpUtil::get::<Value>(self.http_client.as_ref(), &url).await {
                Ok(body)
                    if body.get("Content").is_some()
                        && body.get("ResultCode").and_then(Value::as_i64).unwrap_or(0) >= 0 =>
                {
                    observations.push(
                        json!({"serviceId": node.id, "serviceName": node.service_name,
                        "address": node.address, "port": node.port, "content": body["Content"]}),
                    );
                }
                _ => failures.push(
                    json!({"serviceId": node.id, "serviceName": node.service_name,
                    "reason": "节点读取失败或响应无效"}),
                ),
            }
        }
        json!({"observations": observations, "failures": failures})
    }
}
