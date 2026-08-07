package io.github.easy4rust.druid.agent;

import java.sql.Connection;
import java.util.Objects;

/**
 * 共享 Agent 进程中的单个 JDBC 连接会话。
 *
 * @param connection      独占物理 JDBC 连接
 * @param validationQuery 可选校验 SQL
 */
public record AgentSession(Connection connection, String validationQuery) {

    /** 校验并创建会话。 */
    public AgentSession {
        Objects.requireNonNull(connection, "connection");
    }
}
