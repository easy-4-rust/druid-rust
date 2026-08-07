package io.github.easy4rust.druid.agent;

import java.sql.BatchUpdateException;
import java.sql.SQLException;
import java.sql.SQLNonTransientConnectionException;
import java.sql.SQLRecoverableException;
import java.sql.SQLTransientException;
import java.sql.SQLTransientConnectionException;
import java.util.ArrayList;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;

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
 * @param assignableTypes 具体异常类到父类/接口的 Java 可赋值类型链
 * @param causes         从直接 cause 到根 cause 的运行时类名
 * @param nextExceptions 独立于 cause 的 SQLException next-exception 链
 * @param updateCounts   批处理失败前驱动返回的更新计数
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
        long requestId,
        List<String> assignableTypes,
        List<String> causes,
        List<AgentError> nextExceptions,
        int[] updateCounts) {

    /** 从任意异常提取最接近的 SQLException 语义和协议关联字段。 */
    public static AgentError from(long requestId, String sessionId, Throwable throwable) {
        Objects.requireNonNull(throwable, "throwable");
        Throwable cursor = throwable;
        while (Objects.nonNull(cursor) && !(cursor instanceof SQLException)) {
            cursor = cursor.getCause();
        }
        if (cursor instanceof SQLException sqlException) {
            return fromSqlException(requestId, sessionId, sqlException, true);
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
                requestId,
                assignableTypes(throwable.getClass()),
                causes(throwable),
                List.of(),
                null);
    }

    private static AgentError fromSqlException(
            long requestId,
            String sessionId,
            SQLException exception,
            boolean includeNextExceptions) {
        boolean recoverable = exception instanceof SQLRecoverableException;
        List<AgentError> nextExceptions = new ArrayList<>();
        if (includeNextExceptions) {
            Set<SQLException> seen = Collections.newSetFromMap(new IdentityHashMap<>());
            seen.add(exception);
            SQLException next = exception.getNextException();
            while (Objects.nonNull(next) && seen.add(next) && nextExceptions.size() < 32) {
                nextExceptions.add(fromSqlException(requestId, sessionId, next, false));
                next = next.getNextException();
            }
        }
        return new AgentError(
                exception.getClass().getName(),
                Objects.toString(exception.getMessage(), exception.getClass().getName()),
                exception.getSQLState(),
                exception.getErrorCode(),
                exception instanceof SQLTransientException,
                recoverable,
                recoverable
                        || exception instanceof SQLNonTransientConnectionException
                        || exception instanceof SQLTransientConnectionException,
                sessionId,
                requestId,
                assignableTypes(exception.getClass()),
                causes(exception),
                List.copyOf(nextExceptions),
                exception instanceof BatchUpdateException batchException
                        ? batchException.getUpdateCounts()
                        : null);
    }

    private static List<String> assignableTypes(Class<?> runtimeType) {
        Set<String> result = new LinkedHashSet<>();
        addAssignableTypes(runtimeType, result);
        return List.copyOf(result);
    }

    private static void addAssignableTypes(Class<?> type, Set<String> result) {
        if (Objects.isNull(type) || !result.add(type.getName())) {
            return;
        }
        for (Class<?> interfaceType : type.getInterfaces()) {
            addAssignableTypes(interfaceType, result);
        }
        addAssignableTypes(type.getSuperclass(), result);
    }

    private static List<String> causes(Throwable throwable) {
        List<String> result = new ArrayList<>();
        Set<Throwable> seen = Collections.newSetFromMap(new IdentityHashMap<>());
        seen.add(throwable);
        Throwable cause = throwable.getCause();
        while (Objects.nonNull(cause) && seen.add(cause) && result.size() < 32) {
            result.add(cause.getClass().getName());
            cause = cause.getCause();
        }
        return List.copyOf(result);
    }
}
