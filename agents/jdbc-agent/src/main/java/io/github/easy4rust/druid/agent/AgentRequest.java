package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;

/**
 * JDBC Agent JSON-RPC 2.0 请求。
 *
 * @param jsonrpc 固定为 2.0
 * @param id      关联请求标识
 * @param method  方法名
 * @param params  方法参数
 */
public record AgentRequest(String jsonrpc, long id, String method, JsonNode params) {
}
