package io.github.easy4rust.druid.agent;

import java.sql.SQLException;
import java.util.Objects;

/**
 * JSON-RPC 2.0 错误对象。
 *
 * @param code    JSON-RPC 或 Agent 错误码
 * @param message 对外错误消息
 * @param data    可恢复为 Druid SqlException 的 JDBC 详情
 */
public record AgentRpcError(int code, String message, AgentError data) {

    /** 从执行异常构造协议错误。 */
    public static AgentRpcError from(long requestId, String sessionId, Throwable throwable) {
        Objects.requireNonNull(throwable, "throwable");
        AgentError data = AgentError.from(requestId, sessionId, throwable);
        if (throwable instanceof UnsupportedOperationException) {
            return new AgentRpcError(-32601, Objects.toString(throwable.getMessage(), "method not found"), data);
        }
        if (throwable instanceof IllegalArgumentException || throwable instanceof NullPointerException) {
            return new AgentRpcError(-32602, Objects.toString(throwable.getMessage(), "invalid params"), data);
        }
        int code = throwable instanceof SQLException ? -32001 : -32000;
        return new AgentRpcError(code, data.message(), data);
    }
}
