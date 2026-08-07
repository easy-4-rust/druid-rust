package io.github.easy4rust.druid.agent;

import java.io.Closeable;
import java.io.IOException;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.SQLException;
import java.sql.Savepoint;
import java.sql.Statement;
import java.util.ArrayList;
import java.util.Map;
import java.util.Objects;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.locks.ReentrantLock;

/** 共享 Agent 进程中的单个 JDBC 连接会话及其语句、游标资源域。 */
public final class AgentSession implements Closeable {

    private final Connection connection;
    private final String validationQuery;
    private final ReentrantLock operationLock = new ReentrantLock();
    private final Map<String, PreparedStatement> preparedStatements = new ConcurrentHashMap<>();
    private final Map<String, AgentCursor> cursors = new ConcurrentHashMap<>();
    private final Map<Long, Statement> activeStatements = new ConcurrentHashMap<>();
    private final Map<String, Savepoint> savepoints = new ConcurrentHashMap<>();

    /** 创建拥有独占物理 JDBC 连接的会话。 */
    public AgentSession(Connection connection, String validationQuery) {
        this.connection = Objects.requireNonNull(connection, "connection");
        this.validationQuery = validationQuery;
    }

    /** 返回会话独占的物理 JDBC 连接。 */
    public Connection connection() {
        return connection;
    }

    /** 返回可选的连接校验 SQL。 */
    public String validationQuery() {
        return validationQuery;
    }

    /** 返回串行化当前 JDBC Connection 普通操作的锁。 */
    public ReentrantLock operationLock() {
        return operationLock;
    }

    /** 注册远程预编译语句并返回不可猜测标识。 */
    public String registerPreparedStatement(PreparedStatement statement) {
        String statementId = UUID.randomUUID().toString();
        preparedStatements.put(statementId, Objects.requireNonNull(statement, "statement"));
        return statementId;
    }

    /** 获取仍处于会话资源域内的预编译语句。 */
    public PreparedStatement preparedStatement(String statementId) throws SQLException {
        PreparedStatement statement = preparedStatements.get(statementId);
        if (Objects.isNull(statement) || statement.isClosed()) {
            throw new SQLException("JDBC Agent prepared statement is not open: " + statementId);
        }
        return statement;
    }

    /** 关闭远程预编译语句及引用它的游标；重复调用无副作用。 */
    public void closePreparedStatement(String statementId) throws SQLException {
        SQLException failure = null;
        for (Map.Entry<String, AgentCursor> entry : new ArrayList<>(cursors.entrySet())) {
            if (entry.getValue().referencesStatement(statementId)) {
                try {
                    closeCursor(entry.getKey());
                } catch (SQLException exception) {
                    failure = exception;
                }
            }
        }
        PreparedStatement statement = preparedStatements.remove(statementId);
        if (Objects.nonNull(statement)) {
            try {
                statement.close();
            } catch (SQLException exception) {
                failure = exception;
            }
        }
        if (Objects.nonNull(failure)) {
            throw failure;
        }
    }

    /** 注册未耗尽的远程结果集游标。 */
    public void registerCursor(String cursorId, AgentCursor cursor) {
        cursors.put(cursorId, Objects.requireNonNull(cursor, "cursor"));
    }

    /** 获取远程结果集游标。 */
    public AgentCursor cursor(String cursorId) throws SQLException {
        AgentCursor cursor = cursors.get(cursorId);
        if (Objects.isNull(cursor)) {
            throw new SQLException("JDBC Agent cursor is not open: " + cursorId);
        }
        return cursor;
    }

    /** 关闭并移除远程游标；重复调用无副作用。 */
    public void closeCursor(String cursorId) throws SQLException {
        AgentCursor cursor = cursors.remove(cursorId);
        if (Objects.nonNull(cursor)) {
            cursor.close();
        }
    }

    /** 将正在 JDBC 驱动中执行的语句关联到协议请求 ID。 */
    public void activate(long requestId, Statement statement) {
        activeStatements.put(requestId, Objects.requireNonNull(statement, "statement"));
    }

    /** 清除已结束请求对应的活动语句。 */
    public void deactivate(long requestId) {
        activeStatements.remove(requestId);
    }

    /** 取消目标请求正在执行的 JDBC Statement。 */
    public boolean cancel(long requestId) throws SQLException {
        Statement statement = activeStatements.get(requestId);
        if (Objects.isNull(statement)) {
            return false;
        }
        statement.cancel();
        return true;
    }

    /** 取消指定远程预编译语句当前的 JDBC 执行。 */
    public boolean cancelPreparedStatement(String statementId) throws SQLException {
        PreparedStatement statement = preparedStatements.get(statementId);
        if (Objects.isNull(statement) || statement.isClosed()) {
            return false;
        }
        statement.cancel();
        return true;
    }

    /** 注册 JDBC 保存点并返回跨协议稳定标识。 */
    public String registerSavepoint(Savepoint savepoint) {
        String savepointId = UUID.randomUUID().toString();
        savepoints.put(savepointId, Objects.requireNonNull(savepoint, "savepoint"));
        return savepointId;
    }

    /** 获取当前事务中仍有效的保存点。 */
    public Savepoint savepoint(String savepointId) throws SQLException {
        Savepoint savepoint = savepoints.get(savepointId);
        if (Objects.isNull(savepoint)) {
            throw new SQLException("JDBC Agent savepoint is not open: " + savepointId);
        }
        return savepoint;
    }

    /** 释放并移除保存点。 */
    public void releaseSavepoint(String savepointId) throws SQLException {
        Savepoint savepoint = savepoints.remove(savepointId);
        if (Objects.nonNull(savepoint)) {
            connection.releaseSavepoint(savepoint);
        }
    }

    /** 提交或完整回滚后清除已失效保存点。 */
    public void clearSavepoints() {
        savepoints.clear();
    }

    /** 关闭全部游标、语句和物理连接。 */
    @Override
    public void close() throws IOException {
        operationLock.lock();
        try {
            SQLException failure = null;
            for (AgentCursor cursor : cursors.values()) {
                try {
                    cursor.close();
                } catch (SQLException exception) {
                    failure = exception;
                }
            }
            cursors.clear();
            for (PreparedStatement statement : preparedStatements.values()) {
                try {
                    statement.close();
                } catch (SQLException exception) {
                    failure = exception;
                }
            }
            preparedStatements.clear();
            activeStatements.clear();
            savepoints.clear();
            try {
                connection.close();
            } catch (SQLException exception) {
                failure = exception;
            }
            if (Objects.nonNull(failure)) {
                throw new IOException("failed to close JDBC Agent session", failure);
            }
        } finally {
            operationLock.unlock();
        }
    }
}
