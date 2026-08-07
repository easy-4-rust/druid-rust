package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;

/**
 * Rust 与 JDBC 之间的显式标量值。
 *
 * @param kind  值类型
 * @param value 类型对应的 JSON 值；SQL NULL 时为空
 */
public record AgentValue(String kind, JsonNode value) {
}
