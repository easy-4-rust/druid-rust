package io.github.easy4rust.druid.agent;

import java.sql.SQLException;
import java.sql.SQLNonTransientConnectionException;
import java.sql.SQLRecoverableException;
import java.sql.SQLTransientException;
import java.util.Objects;

/**
 * 可由 Rust 侧恢复为 Druid SqlException 的 JDBC 异常描述。
 *
 * @param exceptionClass Java 异常类名
 * @param message        异常消息
 * @param sqlState       JDBC SQLState
 * @param vendorCode     vendor 错误码
 * @param transientError 是否属于 JDBC transient 异常
 * @param recoverable    是否可恢复
 * @param fatal          是否应立即丢弃物理连接
 * @param sessionId      发生错误的可选会话
 * @param requestId      发生错误的协议请求
 */
public record AgentError(
        String exceptionClass,
        String message,
        String sqlState,
        int vendorCode,
        boolean transientError,
        boolean recoverable,
        boolean fatal,
        String sessionId,
        long requestId) {

    /** 从任意异常提取最接近的 SQLException 语义和协议关联字段。 */
    public static AgentError from(long requestId, String sessionId, Throwable throwable) {
        Objects.requireNonNull(throwable, "throwable");
        Throwable cursor = throwable;
        while (Objects.nonNull(cursor) && !(cursor instanceof SQLException)) {
            cursor = cursor.getCause();
        }
        if (cursor instanceof SQLException sqlException) {
            boolean recoverable = sqlException instanceof SQLRecoverableException;
            return new AgentError(
                    sqlException.getClass().getName(),
                    Objects.toString(sqlException.getMessage(), sqlException.getClass().getName()),
                    sqlException.getSQLState(),
                    sqlException.getErrorCode(),
                    sqlException instanceof SQLTransientException,
                    recoverable,
                    recoverable || sqlException instanceof SQLNonTransientConnectionException,
                    sessionId,
                    requestId);
        }
        return new AgentError(
                throwable.getClass().getName(),
                Objects.toString(throwable.getMessage(), throwable.getClass().getName()),
                null,
                0,
                false,
                false,
                false,
                sessionId,
                requestId);
    }
}
