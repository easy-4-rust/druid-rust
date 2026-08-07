package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;

/**
 * Druid JDBC Agent Protocol v1 响应。
 *
 * @param protocolVersion 协议主版本
 * @param requestId       关联请求标识
 * @param success         是否成功
 * @param payload         成功载荷
 * @param error           失败详情
 */
public record AgentResponse(
        int protocolVersion,
        long requestId,
        boolean success,
        JsonNode payload,
        AgentError error) {

    /** 创建成功响应。 */
    public static AgentResponse success(long requestId, JsonNode payload) {
        return new AgentResponse(1, requestId, true, payload, null);
    }

    /** 创建失败响应。 */
    public static AgentResponse failure(long requestId, Throwable throwable) {
        return new AgentResponse(1, requestId, false, null, AgentError.from(throwable));
    }
}
