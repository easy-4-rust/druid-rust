package io.github.easy4rust.druid.agent;

import java.io.Closeable;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.sql.Array;
import java.sql.Blob;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.SQLException;
import java.sql.Savepoint;
import java.sql.Statement;
import javax.sql.XAConnection;
import javax.transaction.xa.XAResource;
import java.util.ArrayList;
import java.util.Map;
import java.util.Objects;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.locks.ReentrantLock;

/** 共享 Agent 进程中的单个 JDBC 连接会话及其语句、游标资源域。 */
public final class AgentSession implements Closeable {

    private Connection connection;
    private final XAConnection xaConnection;
    private final XAResource xaResource;
    private final String validationQuery;
    private final ReentrantLock operationLock = new ReentrantLock();
    private final Map<String, PreparedStatement> preparedStatements = new ConcurrentHashMap<>();
    private final Map<String, AgentCursor> cursors = new ConcurrentHashMap<>();
    private final Map<String, Array> arrays = new ConcurrentHashMap<>();
    private final Map<String, Blob> blobs = new ConcurrentHashMap<>();
    private final Map<String, BlobStreamHandle> blobStreams = new ConcurrentHashMap<>();
    private final Map<Long, Statement> activeStatements = new ConcurrentHashMap<>();
    private final Map<String, Savepoint> savepoints = new ConcurrentHashMap<>();

    /** 创建拥有独占物理 JDBC 连接的会话。 */
    public AgentSession(Connection connection, String validationQuery) {
        this.connection = Objects.requireNonNull(connection, "connection");
        this.xaConnection = null;
        this.xaResource = null;
        this.validationQuery = validationQuery;
    }

    /** 创建由 XAConnection owner 管理的会话及其当前逻辑 Connection。 */
    public AgentSession(XAConnection xaConnection, String validationQuery) throws SQLException {
        this.xaConnection = Objects.requireNonNull(xaConnection, "xaConnection");
        this.connection = xaConnection.getConnection();
        this.xaResource = xaConnection.getXAResource();
        this.validationQuery = validationQuery;
    }

    /** 返回会话独占的物理 JDBC 连接。 */
    public Connection connection() throws SQLException {
        if (Objects.isNull(connection)) {
            throw new SQLException("JDBC Agent logical connection is closed");
        }
        return connection;
    }

    /** 判断该会话是否由真正的 XADataSource 创建。 */
    public boolean supportsXa() {
        return Objects.nonNull(xaConnection) && Objects.nonNull(xaResource);
    }

    /** 返回 owner 管理的 XAResource；普通 JDBC 会话 fail closed。 */
    public XAResource xaResource() throws SQLException {
        if (!supportsXa()) {
            throw new java.sql.SQLFeatureNotSupportedException(
                    "JDBC Agent session was not created from XADataSource", "0A000");
        }
        return xaResource;
    }

    /** 关闭逻辑 JDBC 句柄但保留 XAConnection owner。 */
    public void closeLogicalConnection() throws SQLException {
        if (Objects.isNull(connection)) {
            return;
        }
        Connection current = connection;
        connection = null;
        SQLException failure = closeLogicalResources();
        try {
            current.close();
        } catch (SQLException exception) {
            failure = appendFailure(failure, exception);
        }
        if (Objects.nonNull(failure)) {
            throw failure;
        }
    }

    /** 在同一 XAConnection owner 上重新取得逻辑 JDBC 句柄。 */
    public void reopenLogicalConnection() throws SQLException {
        if (Objects.isNull(xaConnection) || Objects.isNull(xaResource)) {
            throw new java.sql.SQLFeatureNotSupportedException(
                    "logical connection reopen requires XAConnection owner", "0A000");
        }
        if (Objects.nonNull(connection) && !connection.isClosed()) {
            throw new SQLException("JDBC Agent logical connection is already open");
        }
        connection = xaConnection.getConnection();
    }

    /** 判断逻辑连接当前是否开放。 */
    public boolean hasOpenLogicalConnection() throws SQLException {
        return Objects.nonNull(connection) && !connection.isClosed();
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
                    failure = appendFailure(failure, exception);
                }
            }
        }
        PreparedStatement statement = preparedStatements.remove(statementId);
        if (Objects.nonNull(statement)) {
            try {
                statement.close();
            } catch (SQLException exception) {
                failure = appendFailure(failure, exception);
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

    /** 将驱动 Array 注册到当前逻辑连接资源域并返回不可猜测句柄。 */
    public String registerArray(Array array) {
        String arrayId = UUID.randomUUID().toString();
        arrays.put(arrayId, Objects.requireNonNull(array, "array"));
        return arrayId;
    }

    /** 按当前会话句柄取得仍有效的驱动 Array。 */
    public Array array(String arrayId) throws SQLException {
        Array array = arrays.get(arrayId);
        if (Objects.isNull(array)) {
            throw new SQLException("JDBC Agent array is not open: " + arrayId);
        }
        return array;
    }

    /** 释放驱动 Array；失败时保留句柄以允许重试，成功后重复释放无副作用。 */
    public void freeArray(String arrayId) throws SQLException {
        Array array = arrays.get(arrayId);
        if (Objects.nonNull(array)) {
            SQLException failure = null;
            for (Map.Entry<String, AgentCursor> entry : new ArrayList<>(cursors.entrySet())) {
                if (entry.getValue().referencesArray(arrayId)) {
                    try {
                        closeCursor(entry.getKey());
                    } catch (SQLException exception) {
                        failure = appendFailure(failure, exception);
                    }
                }
            }
            try {
                array.free();
                arrays.remove(arrayId, array);
            } catch (SQLException exception) {
                failure = appendFailure(failure, exception);
            }
            if (Objects.nonNull(failure)) {
                throw failure;
            }
        }
    }

    /** 将驱动 Blob 注册到当前逻辑连接资源域并返回不可猜测句柄。 */
    public String registerBlob(Blob blob) {
        String blobId = UUID.randomUUID().toString();
        blobs.put(blobId, Objects.requireNonNull(blob, "blob"));
        return blobId;
    }

    /** 按当前会话句柄取得仍有效的驱动 Blob。 */
    public Blob blob(String blobId) throws SQLException {
        Blob blob = blobs.get(blobId);
        if (Objects.isNull(blob)) {
            throw new SQLException("JDBC Agent blob is not open: " + blobId);
        }
        return blob;
    }

    /** 注册 Blob 子输入流，并保留其父资源身份。 */
    public String registerBlobInputStream(String blobId, InputStream input) throws SQLException {
        blob(blobId);
        return registerBlobStream(blobId, Objects.requireNonNull(input, "input"));
    }

    /** 注册 Blob 子输出流，并保留其父资源身份。 */
    public String registerBlobOutputStream(String blobId, OutputStream output) throws SQLException {
        blob(blobId);
        return registerBlobStream(blobId, Objects.requireNonNull(output, "output"));
    }

    private String registerBlobStream(String blobId, Closeable stream) {
        String streamId = UUID.randomUUID().toString();
        blobStreams.put(streamId, new BlobStreamHandle(blobId, stream));
        return streamId;
    }

    /** 获取仍在会话内且类型正确的 Blob 输入流。 */
    public InputStream blobInputStream(String streamId) throws SQLException {
        BlobStreamHandle handle = blobStream(streamId);
        if (!(handle.stream instanceof InputStream)) {
            throw new SQLException("JDBC Agent blob stream is not an input stream: " + streamId);
        }
        return (InputStream) handle.stream;
    }

    /** 获取仍在会话内且类型正确的 Blob 输出流。 */
    public OutputStream blobOutputStream(String streamId) throws SQLException {
        BlobStreamHandle handle = blobStream(streamId);
        if (!(handle.stream instanceof OutputStream)) {
            throw new SQLException("JDBC Agent blob stream is not an output stream: " + streamId);
        }
        return (OutputStream) handle.stream;
    }

    private BlobStreamHandle blobStream(String streamId) throws SQLException {
        BlobStreamHandle handle = blobStreams.get(streamId);
        if (Objects.isNull(handle)) {
            throw new SQLException("JDBC Agent blob stream is not open: " + streamId);
        }
        blob(handle.blobId);
        return handle;
    }

    /** 关闭并移除 Blob 子流；失败保留原始 I/O cause，重复关闭无副作用。 */
    public void closeBlobStream(String streamId) throws SQLException {
        BlobStreamHandle handle = blobStreams.remove(streamId);
        if (Objects.nonNull(handle)) {
            try {
                handle.stream.close();
            } catch (IOException exception) {
                throw new SQLException("JDBC Agent blob stream close failed", exception);
            }
        }
    }

    /** 释放驱动 Blob；失败时保留句柄以允许重试，成功后重复释放无副作用。 */
    public void freeBlob(String blobId) throws SQLException {
        Blob blob = blobs.get(blobId);
        if (Objects.nonNull(blob)) {
            SQLException failure = null;
            for (Map.Entry<String, BlobStreamHandle> entry : new ArrayList<>(blobStreams.entrySet())) {
                if (Objects.equals(entry.getValue().blobId, blobId)) {
                    try {
                        closeBlobStream(entry.getKey());
                    } catch (SQLException exception) {
                        failure = appendFailure(failure, exception);
                    }
                }
            }
            try {
                blob.free();
                blobs.remove(blobId, blob);
            } catch (SQLException exception) {
                failure = appendFailure(failure, exception);
            }
            if (Objects.nonNull(failure)) {
                throw failure;
            }
        }
    }

    private static final class BlobStreamHandle {
        private final String blobId;
        private final Closeable stream;

        private BlobStreamHandle(String blobId, Closeable stream) {
            this.blobId = blobId;
            this.stream = stream;
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
            SQLException failure = closeLogicalResources();
            if (Objects.nonNull(connection)) {
                try {
                    connection.close();
                } catch (SQLException exception) {
                    failure = appendFailure(failure, exception);
                }
                connection = null;
            }
            if (Objects.nonNull(xaConnection)) {
                try {
                    xaConnection.close();
                } catch (SQLException exception) {
                    failure = appendFailure(failure, exception);
                }
            }
            if (Objects.nonNull(failure)) {
                throw new IOException("failed to close JDBC Agent session", failure);
            }
        } finally {
            operationLock.unlock();
        }
    }

    private SQLException closeLogicalResources() {
        SQLException failure = null;
        for (String cursorId : new ArrayList<>(cursors.keySet())) {
            try {
                closeCursor(cursorId);
            } catch (SQLException exception) {
                failure = appendFailure(failure, exception);
            }
        }
        for (String statementId : new ArrayList<>(preparedStatements.keySet())) {
            try {
                closePreparedStatement(statementId);
            } catch (SQLException exception) {
                failure = appendFailure(failure, exception);
            }
        }
        for (String arrayId : new ArrayList<>(arrays.keySet())) {
            try {
                freeArray(arrayId);
            } catch (SQLException exception) {
                failure = appendFailure(failure, exception);
            }
        }
        for (String blobId : new ArrayList<>(blobs.keySet())) {
            try {
                freeBlob(blobId);
            } catch (SQLException exception) {
                failure = appendFailure(failure, exception);
            }
        }
        activeStatements.clear();
        savepoints.clear();
        return failure;
    }

    private SQLException appendFailure(SQLException previous, SQLException next) {
        if (Objects.isNull(previous)) {
            return next;
        }
        previous.addSuppressed(next);
        return previous;
    }
}
