package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;

/**
 * Druid JDBC Agent Protocol v1 请求。
 *
 * @param protocolVersion 协议主版本
 * @param requestId       关联请求标识
 * @param operation       操作名称
 * @param payload         操作参数
 */
public record AgentRequest(int protocolVersion, long requestId, String operation, JsonNode payload) {
}
