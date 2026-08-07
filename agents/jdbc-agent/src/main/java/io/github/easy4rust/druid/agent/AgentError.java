package io.github.easy4rust.druid.agent;

import java.sql.SQLException;
import java.sql.SQLRecoverableException;
import java.util.Objects;

/**
 * 可由 Rust 侧恢复为 Druid SqlException 的 JDBC 异常描述。
 *
 * @param className  Java 异常类名
 * @param message    异常消息
 * @param sqlState   JDBC SQLState
 * @param errorCode  vendor 错误码
 * @param recoverable 是否可恢复
 */
public record AgentError(
        String className,
        String message,
        String sqlState,
        int errorCode,
        boolean recoverable) {

    /**
     * 从任意异常提取最接近的 SQLException 语义。
     *
     * @param throwable 原异常
     * @return 结构化协议错误
     */
    public static AgentError from(Throwable throwable) {
        Objects.requireNonNull(throwable, "throwable");
        Throwable cursor = throwable;
        while (Objects.nonNull(cursor) && !(cursor instanceof SQLException)) {
            cursor = cursor.getCause();
        }
        if (cursor instanceof SQLException sqlException) {
            return new AgentError(
                    sqlException.getClass().getName(),
                    Objects.toString(sqlException.getMessage(), sqlException.getClass().getName()),
                    sqlException.getSQLState(),
                    sqlException.getErrorCode(),
                    sqlException instanceof SQLRecoverableException);
        }
        return new AgentError(
                throwable.getClass().getName(),
                Objects.toString(throwable.getMessage(), throwable.getClass().getName()),
                null,
                0,
                false);
    }
}
