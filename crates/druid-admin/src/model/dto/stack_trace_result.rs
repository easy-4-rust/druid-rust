use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 活跃连接堆栈响应。
///
/// 对应 Java: `com.alibaba.druid.admin.model.dto`（`activeConnectionStackTrace-{id}.json`
/// 路由的返回信封；Java 侧是裸 `List<List<String>>`，Admin 代理时统一包成
/// `{"ResultCode":int,"Content":...}`）。
///
/// `Content` 成功时是堆栈字符串数组（多条活跃连接各一条），
/// 失败时是原因字符串（如节点未开启 `removeAbandoned`），
/// 两种形状共存于一个字段，因此保留 `Value` 由调用方按 `result_code` 区分。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StackTraceResult {
    #[serde(rename = "ResultCode")]
    pub result_code: i32,
    #[serde(rename = "Content", default)]
    pub content: Value,
}
