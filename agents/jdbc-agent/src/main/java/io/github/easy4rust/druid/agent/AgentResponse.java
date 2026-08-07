package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;

/**
 * JDBC Agent JSON-RPC 2.0 响应。
 *
 * @param jsonrpc 固定为 2.0
 * @param id      关联请求标识
 * @param result  成功结果
 * @param error   失败详情
 */
public record AgentResponse(String jsonrpc, long id, JsonNode result, AgentRpcError error) {

    /** 创建成功响应。 */
    public static AgentResponse success(long id, JsonNode result) {
        return new AgentResponse("2.0", id, result, null);
    }

    /** 创建失败响应。 */
    public static AgentResponse failure(long id, Throwable throwable) {
        return new AgentResponse("2.0", id, null, AgentRpcError.from(throwable));
    }
}
