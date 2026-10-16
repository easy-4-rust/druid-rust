package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.JsonNodeFactory;
import com.fasterxml.jackson.databind.node.ObjectNode;
import lombok.extern.slf4j.Slf4j;

import java.io.BufferedInputStream;
import java.io.BufferedWriter;
import java.io.ByteArrayOutputStream;
import java.io.Closeable;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.io.OutputStreamWriter;
import java.io.Reader;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.sql.Connection;
import java.sql.Array;
import java.sql.Blob;
import java.sql.Clob;
import java.sql.DatabaseMetaData;
import java.sql.Date;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.SQLFeatureNotSupportedException;
import java.sql.Statement;
import java.sql.Time;
import java.sql.Timestamp;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.Base64;
import java.util.Arrays;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Properties;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import javax.sql.XAConnection;
import javax.sql.XADataSource;
import javax.transaction.xa.XAResource;

/** JSON-RPC 2.0 NDJSON 服务端；一个共享进程可隔离承载多个 JDBC session。 */
@Slf4j
public final class AgentServer implements Closeable {

    private static final int PROTOCOL_VERSION = 1;
    private static final int MAX_FRAME_BYTES = 16 * 1024 * 1024;
    private static final int DEFAULT_PAGE_SIZE = 500;
    private static final int MAX_PAGE_SIZE = 10_000;
    private static final int MAX_BLOB_BYTES = 8 * 1024 * 1024;
    private static final int MAX_CLOB_UNITS = 4 * 1024 * 1024;
    private static final int MAX_CLOB_STREAM_CHUNK_UNITS = 32 * 1024;
    private static final int MAX_BLOB_STREAM_CHUNK_BYTES = 64 * 1024;
    private static final int DEFAULT_RESPONSE_BYTES = 8 * 1024 * 1024;
    private static final int MIN_RESPONSE_BYTES = 1024;
    private static final List<String> AGENT_CAPABILITIES = List.of(
            "multi-session",
            "structured-errors",
            "tagged-values",
            "concurrent-requests",
            "cursor-paging",
            "cancel",
            "remote-prepare",
            "native-prepared-batch",
            "array-values",
            "array-result-set",
            "xa-resource");

    private final InputStream input;
    private final BufferedWriter output;
    private boolean skipLineFeed;
    private final ObjectMapper objectMapper;
    private final Map<String, AgentSession> sessions = new ConcurrentHashMap<>();
    private final ExecutorService requests = Executors.newCachedThreadPool();
    private final AtomicBoolean shuttingDown = new AtomicBoolean();
    private final AtomicBoolean closed = new AtomicBoolean();
    private final AtomicBoolean contractFaultInjection = new AtomicBoolean();

    /**
     * 创建使用指定字节流的服务端。
     *
     * @param input  JSON-RPC NDJSON 输入流
     * @param output JSON-RPC NDJSON 输出流
     */
    public AgentServer(InputStream input, OutputStream output) {
        this.input = new BufferedInputStream(Objects.requireNonNull(input, "input"));
        this.output = new BufferedWriter(new OutputStreamWriter(
                Objects.requireNonNull(output, "output"), StandardCharsets.UTF_8));
        this.objectMapper = new ObjectMapper();
    }

    /** 发送 ready 通知并并发调度 JSON-RPC 请求，直到 EOF 或 shutdown。 */
    public void run() throws IOException {
        ObjectNode ready = JsonNodeFactory.instance.objectNode();
        ready.put("jsonrpc", "2.0");
        ready.put("method", "ready");
        ObjectNode readyParams = ready.putObject("params");
        readyParams.put("protocolVersion", PROTOCOL_VERSION);
        readyParams.put("agentVersion", "0.0.0-design");
        ArrayNode readyCapabilities = readyParams.putArray("capabilities");
        AGENT_CAPABILITIES.forEach(readyCapabilities::add);
        writeLine(ready);

        try {
            String line;
            while (!shuttingDown.get() && Objects.nonNull(line = readFrame())) {
                String frame = line;
                requests.submit(() -> handleFrame(frame));
            }
        } catch (IOException exception) {
            if (!shuttingDown.get()) {
                throw exception;
            }
        } finally {
            requests.shutdown();
            awaitRequests();
        }
    }

    /** 在读取阶段限制原始 UTF-8 帧字节数，避免无换行输入先占满内存。 */
    private String readFrame() throws IOException {
        ByteArrayOutputStream frame = new ByteArrayOutputStream();
        int value;
        while ((value = input.read()) != -1) {
            if (skipLineFeed) {
                skipLineFeed = false;
                if (value == '\n') {
                    continue;
                }
            }
            if (value == '\n') {
                return frame.toString(StandardCharsets.UTF_8);
            }
            if (value == '\r') {
                skipLineFeed = true;
                return frame.toString(StandardCharsets.UTF_8);
            }
            if (frame.size() >= MAX_FRAME_BYTES) {
                throw new IOException("JSON-RPC request exceeds maximum frame size");
            }
            frame.write(value);
        }
        return frame.size() == 0 ? null : frame.toString(StandardCharsets.UTF_8);
    }

    private void handleFrame(String line) {
        AgentRequest request = null;
        String sessionId = null;
        try {
            if (line.getBytes(StandardCharsets.UTF_8).length > MAX_FRAME_BYTES) {
                throw new IOException("JSON-RPC request exceeds maximum frame size");
            }
            request = objectMapper.readValue(line, AgentRequest.class);
            validateRequest(request);
            sessionId = nullableText(request.params(), "sessionId");
            JsonNode result = dispatch(request);
            writeLine(objectMapper.valueToTree(AgentResponse.success(request.id(), result)));
            if ("shutdown".equals(request.method())) {
                requestShutdown();
            }
        } catch (Exception exception) {
            long requestId = Objects.isNull(request) ? 0 : request.id();
            Throwable exposedException = Objects.isNull(request)
                    ? new IllegalArgumentException("invalid JSON-RPC request")
                    : exception;
            try {
                writeLine(objectMapper.valueToTree(
                        AgentResponse.failure(requestId, sessionId, exposedException)));
            } catch (IOException writeException) {
                log.error("Unable to write JDBC Agent error response", writeException);
                requestShutdown();
            }
            if (Objects.isNull(request)) {
                log.debug("JDBC Agent rejected malformed request: requestId={}, errorType={}",
                        requestId, exception.getClass().getSimpleName());
            } else {
                log.debug("JDBC operation failed: requestId={}", requestId, exception);
            }
        }
    }

    private void validateRequest(AgentRequest request) {
        Objects.requireNonNull(request, "request");
        Objects.requireNonNull(request.method(), "method");
        Objects.requireNonNull(request.params(), "params");
        if (!"2.0".equals(request.jsonrpc())) {
            throw new IllegalArgumentException("unsupported JSON-RPC version");
        }
    }

    private JsonNode dispatch(AgentRequest request) throws Exception {
        String method = request.method();
        JsonNode params = request.params();
        return switch (method) {
            case "handshake" -> handshake(params);
            case "open_session", "session.open" -> openSession(params);
            case "close_session", "session.close" -> closeSession(params);
            case "cancel" -> cancel(params);
            case "shutdown" -> JsonNodeFactory.instance.objectNode();
            case "diagnostic_crash" -> diagnosticCrash();
            case "diagnostic_protocol_failure" -> diagnosticProtocolFailure();
            default -> withSession(params, session -> dispatchSession(
                    normalizeMethod(method), request.id(), session, params));
        };
    }

    private String normalizeMethod(String method) {
        String operation = method.startsWith("session.")
                ? method.substring("session.".length())
                : method;
        return switch (operation) {
            case "exec" -> "execute_update";
            case "fetch" -> "execute_query";
            case "ping" -> "validate_connection";
            default -> operation;
        };
    }

    private JsonNode dispatchSession(
            String method,
            long requestId,
            AgentSession session,
            JsonNode params) throws Exception {
        return switch (method) {
            case "validate_connection" -> validateConnection(session, params);
            case "execute_update" -> executeUpdate(requestId, session, params);
            case "execute_query" -> executeQuery(requestId, session, params);
            case "execute" -> execute(requestId, session, params);
            case "prepare" -> prepare(session, params);
            case "execute_prepared" -> executePrepared(requestId, session, params);
            case "execute_prepared_batch" -> executePreparedBatch(requestId, session, params);
            case "close_statement" -> closeStatement(session, params);
            case "fetch_page" -> fetchPage(requestId, session, params);
            case "close_cursor" -> closeCursor(session, params);
            case "begin" -> begin(session);
            case "commit" -> commit(session);
            case "rollback" -> rollback(session);
            case "set_savepoint" -> setSavepoint(session, params);
            case "rollback_to_savepoint" -> rollbackToSavepoint(session, params);
            case "release_savepoint" -> releaseSavepoint(session, params);
            case "set_auto_commit" -> setAutoCommit(session, params);
            case "get_auto_commit" -> value(session.connection().getAutoCommit());
            case "set_read_only" -> setReadOnly(session, params);
            case "get_read_only" -> value(session.connection().isReadOnly());
            case "set_transaction_isolation" -> setTransactionIsolation(session, params);
            case "get_transaction_isolation" -> value(session.connection().getTransactionIsolation());
            case "set_catalog" -> setCatalog(session, params);
            case "get_catalog" -> nullableValue(session.connection().getCatalog());
            case "set_schema" -> setSchema(session, params);
            case "get_schema" -> nullableValue(session.connection().getSchema());
            case "list_catalogs" -> listCatalogs(session);
            case "list_schemas" -> listSchemas(session);
            case "database_metadata" -> databaseMetadata(session);
            case "create_array" -> createArray(session, params);
            case "array_metadata" -> arrayMetadata(session, params);
            case "array_values" -> arrayValues(session, params);
            case "array_result_set" -> arrayResultSet(session, params);
            case "free_array" -> freeArray(session, params);
            case "create_blob" -> createBlob(session);
            case "blob_length" -> blobLength(session, params);
            case "blob_get_bytes" -> blobGetBytes(session, params);
            case "blob_set_bytes" -> blobSetBytes(session, params);
            case "blob_position_bytes" -> blobPositionBytes(session, params);
            case "blob_position_blob" -> throw new SQLFeatureNotSupportedException(
                    "Blob#position(Blob,long) is not supported", "0A000");
            case "blob_truncate" -> blobTruncate(session, params);
            case "blob_open_input_stream" -> blobOpenInputStream(session, params);
            case "blob_read_input_stream" -> blobReadInputStream(requestId, session, params);
            case "blob_open_output_stream" -> blobOpenOutputStream(session, params);
            case "blob_write_output_stream" -> blobWriteOutputStream(session, params);
            case "blob_flush_stream" -> blobFlushStream(session, params);
            case "blob_close_stream" -> blobCloseStream(session, params);
            case "free_blob" -> freeBlob(session, params);
            case "create_clob" -> createClob(session);
            case "clob_length" -> clobLength(session, params);
            case "clob_get_sub_string" -> clobGetSubString(session, params);
            case "clob_set_string" -> clobSetString(session, params);
            case "clob_position_string" -> clobPositionString(session, params);
            case "clob_position_clob" -> throw new SQLFeatureNotSupportedException(
                    "Clob#position(Clob,long) is not supported", "0A000");
            case "clob_truncate" -> clobTruncate(session, params);
            case "clob_open_reader" -> clobOpenReader(session, params);
            case "clob_read_reader" -> clobReadReader(session, params);
            case "clob_close_stream" -> clobCloseStream(session, params);
            case "free_clob" -> freeClob(session, params);
            case "close_logical_connection" -> closeLogicalConnection(session);
            case "reopen_logical_connection" -> reopenLogicalConnection(session);
            case "xa_start" -> xaStart(session, params);
            case "xa_end" -> xaEnd(session, params);
            case "xa_prepare" -> xaPrepare(session, params);
            case "xa_commit" -> xaCommit(session, params);
            case "xa_rollback" -> xaRollback(session, params);
            case "xa_forget" -> xaForget(session, params);
            case "xa_recover" -> xaRecover(session, params);
            case "xa_set_timeout" -> xaSetTimeout(session, params);
            case "xa_get_timeout" -> value(session.xaResource().getTransactionTimeout());
            case "xa_is_same_rm" -> xaIsSameRm(session, params);
            default -> throw new UnsupportedOperationException("unsupported method " + method);
        };
    }

    private JsonNode handshake(JsonNode params) {
        if (params.path("protocolVersion").asInt() != PROTOCOL_VERSION) {
            throw new IllegalArgumentException("unsupported JDBC Agent protocol version");
        }
        JsonNode requestedCapabilities = params.path("capabilities");
        if (!requestedCapabilities.isArray()) {
            throw new IllegalArgumentException("handshake capabilities must be an array");
        }
        for (JsonNode capability : requestedCapabilities) {
            String name = capability.asText();
            if (!AGENT_CAPABILITIES.contains(name)) {
                throw new UnsupportedOperationException("unsupported JDBC Agent capability");
            }
        }
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        contractFaultInjection.set(params.path("contractFaultInjection").asBoolean(false));
        result.put("protocolVersion", PROTOCOL_VERSION);
        result.put("agentVersion", "0.0.0-design");
        result.put("driverArtifactVersion", params.path("driverArtifactVersion").asText("unmanaged"));
        ArrayNode capabilities = result.putArray("capabilities");
        AGENT_CAPABILITIES.forEach(capabilities::add);
        result.put("defaultPageSize", DEFAULT_PAGE_SIZE);
        result.put("maxResponseBytes", DEFAULT_RESPONSE_BYTES);
        return result;
    }

    private JsonNode diagnosticCrash() {
        requireContractFaultInjection();
        Runtime.getRuntime().halt(91);
        throw new IllegalStateException("JVM halt unexpectedly returned");
    }

    private JsonNode diagnosticProtocolFailure() throws IOException {
        requireContractFaultInjection();
        synchronized (this) {
            output.write("{invalid-json-rpc-frame");
            output.newLine();
            output.flush();
        }
        requestShutdown();
        return JsonNodeFactory.instance.objectNode();
    }

    private void requireContractFaultInjection() {
        if (!contractFaultInjection.get()) {
            throw new SecurityException("contract fault injection is disabled");
        }
    }

    private JsonNode openSession(JsonNode payload) throws SQLException {
        Objects.requireNonNull(payload, "connect payload");
        Properties properties = new Properties();
        JsonNode propertyNode = payload.get("properties");
        if (Objects.nonNull(propertyNode) && propertyNode.isObject()) {
            Iterator<Map.Entry<String, JsonNode>> fields = propertyNode.fields();
            while (fields.hasNext()) {
                Map.Entry<String, JsonNode> field = fields.next();
                properties.setProperty(field.getKey(), field.getValue().asText());
            }
        }
        String validationQuery = nullableText(payload, "validationQuery");
        String xaClassName = nullableText(payload, "xaDataSourceClassName");
        Connection connection = null;
        XAConnection xaConnection = null;
        try {
            AgentSession session;
            if (Objects.nonNull(xaClassName)) {
                XADataSource dataSource = XaDataSourceFactory.create(
                        xaClassName, payload.get("xaDataSourceProperties"));
                xaConnection = dataSource.getXAConnection();
                session = new AgentSession(xaConnection, validationQuery);
                connection = session.connection();
            } else {
                connection = DriverManager.getConnection(requiredText(payload, "url"), properties);
                session = new AgentSession(connection, validationQuery);
            }
            DatabaseMetaData metadata = connection.getMetaData();
            String sessionId = UUID.randomUUID().toString();
            sessions.put(sessionId, session);
            ObjectNode result = JsonNodeFactory.instance.objectNode();
            result.put("sessionId", sessionId);
            result.put("agentVersion", "0.0.0-design");
            result.put("driverName", metadata.getDriverName());
            result.put("driverVersion", metadata.getDriverVersion());
            result.put("databaseProductName", metadata.getDatabaseProductName());
            result.put("databaseProductVersion", metadata.getDatabaseProductVersion());
            result.put("autoCommit", connection.getAutoCommit());
            result.put("readOnly", connection.isReadOnly());
            result.put("transactionIsolation", connection.getTransactionIsolation());
            putNullable(result, "catalog", safeCatalog(connection));
            putNullable(result, "schema", safeSchema(connection));
            ObjectNode capabilities = result.putObject("capabilities");
            capabilities.put("transactions", metadata.supportsTransactions());
            capabilities.put("savepoints", metadata.supportsSavepoints());
            capabilities.put("autoCommit", true);
            capabilities.put("readOnly", true);
            capabilities.put("transactionIsolation", true);
            capabilities.put("catalog", supportsCatalog(metadata));
            capabilities.put("schema", supportsSchema(metadata));
            capabilities.put("xa", session.supportsXa());
            return result;
        } catch (SQLException | RuntimeException exception) {
            if (Objects.nonNull(xaConnection)) {
                xaConnection.close();
            } else if (Objects.nonNull(connection)) {
                connection.close();
            }
            throw exception;
        }
    }

    private JsonNode closeSession(JsonNode payload) throws IOException {
        AgentSession session = sessions.remove(requiredText(payload, "sessionId"));
        if (Objects.nonNull(session)) {
            session.operationLock().lock();
            try {
                session.close();
            } finally {
                session.operationLock().unlock();
            }
        }
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode withSession(JsonNode payload, SessionOperation operation)
            throws Exception {
        String sessionId = requiredText(payload, "sessionId");
        AgentSession session = sessions.get(sessionId);
        if (Objects.isNull(session)) {
            throw new SQLException("JDBC Agent session is not open: " + sessionId);
        }
        session.operationLock().lock();
        try {
            return operation.execute(session);
        } finally {
            session.operationLock().unlock();
        }
    }

    @FunctionalInterface
    private interface SessionOperation {
        JsonNode execute(AgentSession session) throws Exception;
    }

    private JsonNode closeLogicalConnection(AgentSession session) throws SQLException {
        session.closeLogicalConnection();
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode reopenLogicalConnection(AgentSession session) throws SQLException {
        session.reopenLogicalConnection();
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode xaStart(AgentSession session, JsonNode params) throws Exception {
        session.xaResource().start(AgentXid.fromJson(params.get("xid")), requiredInt(params, "flags"));
        return JsonNodeFactory.instance.objectNode().put("started", 1);
    }

    private JsonNode xaEnd(AgentSession session, JsonNode params) throws Exception {
        session.xaResource().end(AgentXid.fromJson(params.get("xid")), requiredInt(params, "flags"));
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode xaPrepare(AgentSession session, JsonNode params) throws Exception {
        return JsonNodeFactory.instance.objectNode().put(
                "vote", session.xaResource().prepare(AgentXid.fromJson(params.get("xid"))));
    }

    private JsonNode xaCommit(AgentSession session, JsonNode params) throws Exception {
        session.xaResource().commit(
                AgentXid.fromJson(params.get("xid")), params.path("onePhase").asBoolean());
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode xaRollback(AgentSession session, JsonNode params) throws Exception {
        session.xaResource().rollback(AgentXid.fromJson(params.get("xid")));
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode xaForget(AgentSession session, JsonNode params) throws Exception {
        session.xaResource().forget(AgentXid.fromJson(params.get("xid")));
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode xaRecover(AgentSession session, JsonNode params) throws Exception {
        ArrayNode xids = JsonNodeFactory.instance.arrayNode();
        for (javax.transaction.xa.Xid xid : session.xaResource().recover(requiredInt(params, "flags"))) {
            xids.add(new AgentXid(
                    xid.getFormatId(), xid.getGlobalTransactionId(), xid.getBranchQualifier()).toJson());
        }
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        result.set("xids", xids);
        return result;
    }

    private JsonNode xaSetTimeout(AgentSession session, JsonNode params) throws Exception {
        return JsonNodeFactory.instance.objectNode().put(
                "applied", session.xaResource().setTransactionTimeout(requiredInt(params, "seconds")));
    }

    private JsonNode xaIsSameRm(AgentSession session, JsonNode params) throws Exception {
        AgentSession other = sessions.get(requiredText(params, "otherSessionId"));
        if (Objects.isNull(other)) {
            throw new SQLException("JDBC Agent comparison session is not open");
        }
        return JsonNodeFactory.instance.objectNode().put(
                "sameResourceManager", session.xaResource().isSameRM(other.xaResource()));
    }

    private int requiredInt(JsonNode payload, String field) {
        JsonNode value = Objects.requireNonNull(payload.get(field), field);
        if (!value.canConvertToInt()) {
            throw new IllegalArgumentException(field + " must be an integer");
        }
        return value.intValue();
    }

    private long requiredLong(JsonNode payload, String field) {
        JsonNode value = Objects.requireNonNull(payload.get(field), field);
        if (!value.canConvertToLong()) {
            throw new IllegalArgumentException(field + " must be a long integer");
        }
        return value.longValue();
    }

    /** 创建由当前 Agent session 独占的驱动 Array 句柄。 */
    private JsonNode createArray(AgentSession session, JsonNode payload) throws SQLException, IOException {
        JsonNode elements = payload.get("elements");
        Object[] values = null;
        if (Objects.nonNull(elements) && !elements.isNull()) {
            if (!elements.isArray()) {
                throw new IllegalArgumentException("elements must be an array or null");
            }
            values = new Object[elements.size()];
            for (int index = 0; index < elements.size(); index++) {
                AgentValue value = objectMapper.treeToValue(elements.get(index), AgentValue.class);
                values[index] = value.toJdbcObject();
            }
        }
        Array array = session.connection().createArrayOf(nullableText(payload, "typeName"), values);
        return JsonNodeFactory.instance.objectNode().put("arrayId", session.registerArray(array));
    }

    /** 从原驱动 Array 读取 SQL 基础类型，保留驱动异常。 */
    private JsonNode arrayMetadata(AgentSession session, JsonNode payload) throws SQLException {
        Array array = session.array(requiredText(payload, "arrayId"));
        return JsonNodeFactory.instance.objectNode()
                .put("baseTypeName", array.getBaseTypeName())
                .put("baseType", array.getBaseType());
    }

    /** 按 JDBC 1-based 范围读取元素；未实现的 type map 重载明确拒绝。 */
    private JsonNode arrayValues(AgentSession session, JsonNode payload) throws SQLException {
        if (payload.has("typeMap")) {
            throw new SQLFeatureNotSupportedException("Array#getArray(Map) is not supported", "0A000");
        }
        Array array = session.array(requiredText(payload, "arrayId"));
        boolean hasIndex = payload.has("index");
        boolean hasCount = payload.has("count");
        if (hasIndex != hasCount) {
            throw new IllegalArgumentException("index and count must be supplied together");
        }
        Object raw = hasIndex
                ? array.getArray(requiredLong(payload, "index"), requiredInt(payload, "count"))
                : array.getArray();
        if (Objects.isNull(raw) || !raw.getClass().isArray()) {
            throw new SQLException("JDBC Array#getArray returned a non-array value");
        }
        int size = java.lang.reflect.Array.getLength(raw);
        if (size > MAX_PAGE_SIZE) {
            throw new SQLException("JDBC Array response exceeds maximum element count; use index/count");
        }
        int baseType = array.getBaseType();
        ArrayNode values = JsonNodeFactory.instance.arrayNode();
        for (int index = 0; index < size; index++) {
            values.add(objectMapper.valueToTree(AgentValue.fromArrayElement(
                    java.lang.reflect.Array.get(raw, index), baseType)));
        }
        return JsonNodeFactory.instance.objectNode().set("values", values);
    }

    /** 释放远程 Array；失败保留会话句柄供下一次请求重试。 */
    private JsonNode freeArray(AgentSession session, JsonNode payload) throws SQLException {
        session.freeArray(requiredText(payload, "arrayId"));
        return JsonNodeFactory.instance.objectNode();
    }

    /** 创建由当前 Agent session 独占的驱动 Blob 句柄。 */
    private JsonNode createBlob(AgentSession session) throws SQLException {
        Blob blob = session.connection().createBlob();
        return JsonNodeFactory.instance.objectNode().put("blobId", session.registerBlob(blob));
    }

    /** 返回驱动 Blob 的实际字节长度。 */
    private JsonNode blobLength(AgentSession session, JsonNode payload) throws SQLException {
        return JsonNodeFactory.instance.objectNode()
                .put("length", session.blob(requiredText(payload, "blobId")).length());
    }

    /** 按 JDBC 的 1-based 位置读取有界字节范围，保留驱动异常。 */
    private JsonNode blobGetBytes(AgentSession session, JsonNode payload) throws SQLException {
        Blob blob = session.blob(requiredText(payload, "blobId"));
        int length = requiredInt(payload, "length");
        if (length > MAX_BLOB_BYTES) {
            throw new IllegalArgumentException("Blob read length exceeds maximum request size");
        }
        byte[] bytes = blob.getBytes(requiredLong(payload, "position"), length);
        if (bytes.length > MAX_BLOB_BYTES) {
            throw new SQLException("JDBC Blob#getBytes returned too many bytes");
        }
        return JsonNodeFactory.instance.objectNode().put("bytes", Base64.getEncoder().encodeToString(bytes));
    }

    /** 委托两种 Blob#setBytes 重载；offset 和 length 必须成对提供。 */
    private JsonNode blobSetBytes(AgentSession session, JsonNode payload) throws SQLException {
        Blob blob = session.blob(requiredText(payload, "blobId"));
        byte[] bytes = blobRequestBytes(payload, "bytes");
        boolean hasOffset = payload.has("offset");
        boolean hasLength = payload.has("length");
        if (hasOffset != hasLength) {
            throw new IllegalArgumentException("offset and length must be supplied together");
        }
        long position = requiredLong(payload, "position");
        int written = hasOffset
                ? blob.setBytes(position, bytes, requiredInt(payload, "offset"), requiredInt(payload, "length"))
                : blob.setBytes(position, bytes);
        return JsonNodeFactory.instance.objectNode().put("written", written);
    }

    /** 用字节模式搜索 Blob，直接返回 JDBC 的 1-based 位置或 -1。 */
    private JsonNode blobPositionBytes(AgentSession session, JsonNode payload) throws SQLException {
        Blob blob = session.blob(requiredText(payload, "blobId"));
        return JsonNodeFactory.instance.objectNode().put("position",
                blob.position(blobRequestBytes(payload, "pattern"), requiredLong(payload, "start")));
    }

    /** 将 Blob 截断到驱动指定长度。 */
    private JsonNode blobTruncate(AgentSession session, JsonNode payload) throws SQLException {
        session.blob(requiredText(payload, "blobId")).truncate(requiredLong(payload, "length"));
        return JsonNodeFactory.instance.objectNode();
    }

    /** 释放远程 Blob；失败时会话保留句柄供重试。 */
    private JsonNode freeBlob(AgentSession session, JsonNode payload) throws SQLException {
        session.freeBlob(requiredText(payload, "blobId"));
        return JsonNodeFactory.instance.objectNode();
    }

    /** 创建由当前 Agent session 独占的驱动 Clob 句柄。 */
    private JsonNode createClob(AgentSession session) throws SQLException {
        Clob clob = session.connection().createClob();
        return JsonNodeFactory.instance.objectNode().put("clobId", session.registerClob(clob));
    }

    /** 返回以 Java UTF-16 code unit 计数的驱动 Clob 长度。 */
    private JsonNode clobLength(AgentSession session, JsonNode payload) throws SQLException {
        return JsonNodeFactory.instance.objectNode()
                .put("length", session.clob(requiredText(payload, "clobId")).length());
    }

    /** 按 JDBC 1-based 位置读取有界字符范围，使用原始 UTF-16 code unit 编码。 */
    private JsonNode clobGetSubString(AgentSession session, JsonNode payload) throws SQLException {
        Clob clob = session.clob(requiredText(payload, "clobId"));
        int length = requiredInt(payload, "length");
        if (length > MAX_CLOB_UNITS) {
            throw new IllegalArgumentException("Clob read length exceeds maximum request size");
        }
        String value = clob.getSubString(requiredLong(payload, "position"), length);
        if (value.length() > MAX_CLOB_UNITS) {
            throw new SQLException("JDBC Clob#getSubString returned too many characters");
        }
        return JsonNodeFactory.instance.objectNode().put("utf16", encodeUtf16(value));
    }

    /** 委托两种 Clob#setString 重载；输入 offset/length 均按 UTF-16 unit 计数。 */
    private JsonNode clobSetString(AgentSession session, JsonNode payload) throws SQLException {
        Clob clob = session.clob(requiredText(payload, "clobId"));
        String value = decodeUtf16(payload, "utf16");
        boolean hasOffset = payload.has("offset");
        boolean hasLength = payload.has("length");
        if (hasOffset != hasLength) {
            throw new IllegalArgumentException("offset and length must be supplied together");
        }
        long position = requiredLong(payload, "position");
        int written = hasOffset
                ? clob.setString(position, value, requiredInt(payload, "offset"), requiredInt(payload, "length"))
                : clob.setString(position, value);
        return JsonNodeFactory.instance.objectNode().put("written", written);
    }

    /** 以 UTF-16 字符模式搜索 Clob，直接返回驱动的 1-based 位置或 -1。 */
    private JsonNode clobPositionString(AgentSession session, JsonNode payload) throws SQLException {
        Clob clob = session.clob(requiredText(payload, "clobId"));
        return JsonNodeFactory.instance.objectNode().put("position",
                clob.position(decodeUtf16(payload, "utf16"), requiredLong(payload, "start")));
    }

    /** 将 Clob 截断到驱动指定的 UTF-16 单元长度。 */
    private JsonNode clobTruncate(AgentSession session, JsonNode payload) throws SQLException {
        session.clob(requiredText(payload, "clobId")).truncate(requiredLong(payload, "length"));
        return JsonNodeFactory.instance.objectNode();
    }

    /** 释放远程 Clob；失败时会话保留句柄供重试。 */
    private JsonNode freeClob(AgentSession session, JsonNode payload) throws SQLException {
        session.freeClob(requiredText(payload, "clobId"));
        return JsonNodeFactory.instance.objectNode();
    }

    /** 打开完整或 JDBC 1-based 范围字符读取器，注册到父 Clob 资源域。 */
    private JsonNode clobOpenReader(AgentSession session, JsonNode payload) throws SQLException {
        String clobId = requiredText(payload, "clobId");
        Clob clob = session.clob(clobId);
        boolean hasPosition = payload.has("position");
        boolean hasLength = payload.has("length");
        if (hasPosition != hasLength) {
            throw new IllegalArgumentException("position and length must be supplied together");
        }
        Reader reader = hasPosition
                ? clob.getCharacterStream(requiredLong(payload, "position"), requiredLong(payload, "length"))
                : clob.getCharacterStream();
        try {
            return JsonNodeFactory.instance.objectNode()
                    .put("streamId", session.registerClobReader(clobId, reader));
        } catch (SQLException exception) {
            try {
                reader.close();
            } catch (IOException closeFailure) {
                exception.addSuppressed(closeFailure);
            }
            throw exception;
        }
    }

    /** 每次只读取至多 32K 个 UTF-16 单元，并显式报告 EOF。 */
    private JsonNode clobReadReader(AgentSession session, JsonNode payload) throws SQLException {
        Reader reader = session.clobReader(requiredText(payload, "streamId"));
        int maxUnits = requiredInt(payload, "maxUnits");
        if (maxUnits < 1 || maxUnits > MAX_CLOB_STREAM_CHUNK_UNITS) {
            throw new IllegalArgumentException("maxUnits must be between 1 and 32768");
        }
        char[] buffer = new char[maxUnits];
        int count;
        try {
            count = reader.read(buffer);
        } catch (IOException exception) {
            throw new SQLException("JDBC Agent clob reader failed", exception);
        }
        return JsonNodeFactory.instance.objectNode()
                .put("utf16", encodeUtf16(count < 0 ? "" : new String(buffer, 0, count)))
                .put("eof", count < 0);
    }

    /** 关闭 Clob 子流句柄；父 Clob 释放后的重复关闭无副作用。 */
    private JsonNode clobCloseStream(AgentSession session, JsonNode payload) throws SQLException {
        session.closeClobStream(requiredText(payload, "streamId"));
        return JsonNodeFactory.instance.objectNode();
    }

    private String decodeUtf16(JsonNode payload, String field) {
        JsonNode node = Objects.requireNonNull(payload.get(field), field);
        if (!node.isTextual()) {
            throw new IllegalArgumentException(field + " must be base64 UTF-16 text");
        }
        byte[] bytes = Base64.getDecoder().decode(node.asText());
        if ((bytes.length & 1) != 0 || bytes.length / 2 > MAX_CLOB_UNITS) {
            throw new IllegalArgumentException(field + " must contain bounded UTF-16 units");
        }
        char[] units = new char[bytes.length / 2];
        for (int index = 0; index < units.length; index++) {
            units[index] = (char) (((bytes[index * 2] & 0xff) << 8) | (bytes[index * 2 + 1] & 0xff));
        }
        return new String(units);
    }

    private String encodeUtf16(String value) {
        byte[] bytes = new byte[value.length() * 2];
        for (int index = 0; index < value.length(); index++) {
            char unit = value.charAt(index);
            bytes[index * 2] = (byte) (unit >>> 8);
            bytes[index * 2 + 1] = (byte) unit;
        }
        return Base64.getEncoder().encodeToString(bytes);
    }

    /** 打开 Blob 的完整或 JDBC 1-based 范围输入流，注册到父 Blob 资源域。 */
    private JsonNode blobOpenInputStream(AgentSession session, JsonNode payload) throws SQLException {
        String blobId = requiredText(payload, "blobId");
        Blob blob = session.blob(blobId);
        boolean hasPosition = payload.has("position");
        boolean hasLength = payload.has("length");
        if (hasPosition != hasLength) {
            throw new IllegalArgumentException("position and length must be supplied together");
        }
        InputStream input = hasPosition
                ? blob.getBinaryStream(requiredLong(payload, "position"), requiredLong(payload, "length"))
                : blob.getBinaryStream();
        try {
            return JsonNodeFactory.instance.objectNode()
                    .put("streamId", session.registerBlobInputStream(blobId, input));
        } catch (SQLException exception) {
            try {
                input.close();
            } catch (IOException closeFailure) {
                exception.addSuppressed(closeFailure);
            }
            throw exception;
        }
    }

    /** 单次只读取至多 64 KiB，并显式返回 EOF 状态。 */
    private JsonNode blobReadInputStream(long requestId, AgentSession session, JsonNode payload)
            throws SQLException {
        String streamId = requiredText(payload, "streamId");
        InputStream input = session.blobInputStream(streamId);
        int maxBytes = requiredInt(payload, "maxBytes");
        if (maxBytes < 1 || maxBytes > MAX_BLOB_STREAM_CHUNK_BYTES) {
            throw new IllegalArgumentException("maxBytes must be between 1 and 65536");
        }
        byte[] buffer = new byte[maxBytes];
        int count = 0;
        IOException readFailure = null;
        boolean cancelled;
        session.activateBlobRead(requestId, streamId);
        try {
            count = input.read(buffer);
        } catch (IOException exception) {
            readFailure = exception;
        } finally {
            cancelled = session.finishBlobRead(requestId);
        }
        if (cancelled) {
            throw new SQLException("JDBC Agent blob stream read cancelled", "HY008", readFailure);
        }
        if (Objects.nonNull(readFailure)) {
            throw new SQLException("JDBC Agent blob stream read failed", readFailure);
        }
        return JsonNodeFactory.instance.objectNode()
                .put("bytes", Base64.getEncoder().encodeToString(
                        count < 0 ? new byte[0] : Arrays.copyOf(buffer, count)))
                .put("eof", count < 0);
    }

    /** 关闭 Blob 子流句柄，重复关闭无副作用。 */
    private JsonNode blobCloseStream(AgentSession session, JsonNode payload) throws SQLException {
        session.closeBlobStream(requiredText(payload, "streamId"));
        return JsonNodeFactory.instance.objectNode();
    }

    /** 在 JDBC 1-based 位置打开 Blob 子输出流。 */
    private JsonNode blobOpenOutputStream(AgentSession session, JsonNode payload) throws SQLException {
        String blobId = requiredText(payload, "blobId");
        OutputStream output = session.blob(blobId).setBinaryStream(requiredLong(payload, "position"));
        try {
            return JsonNodeFactory.instance.objectNode()
                    .put("streamId", session.registerBlobOutputStream(blobId, output));
        } catch (SQLException exception) {
            try {
                output.close();
            } catch (IOException closeFailure) {
                exception.addSuppressed(closeFailure);
            }
            throw exception;
        }
    }

    /** 单次向驱动 Blob 流写入至多 64 KiB。 */
    private JsonNode blobWriteOutputStream(AgentSession session, JsonNode payload) throws SQLException {
        OutputStream output = session.blobOutputStream(requiredText(payload, "streamId"));
        byte[] bytes = blobStreamRequestBytes(payload);
        try {
            output.write(bytes);
        } catch (IOException exception) {
            throw new SQLException("JDBC Agent blob stream write failed", exception);
        }
        return JsonNodeFactory.instance.objectNode().put("written", bytes.length);
    }

    /** 按 JDBC 流语义将缓冲的 Blob 字节刷新到底层驱动。 */
    private JsonNode blobFlushStream(AgentSession session, JsonNode payload) throws SQLException {
        OutputStream output = session.blobOutputStream(requiredText(payload, "streamId"));
        try {
            output.flush();
        } catch (IOException exception) {
            throw new SQLException("JDBC Agent blob stream flush failed", exception);
        }
        return JsonNodeFactory.instance.objectNode();
    }

    private byte[] blobStreamRequestBytes(JsonNode payload) {
        String encoded = requiredText(payload, "bytes");
        if (encoded.length() > ((MAX_BLOB_STREAM_CHUNK_BYTES + 2L) / 3L) * 4L) {
            throw new IllegalArgumentException("Blob stream write exceeds maximum chunk size");
        }
        byte[] bytes = Base64.getDecoder().decode(encoded);
        if (bytes.length > MAX_BLOB_STREAM_CHUNK_BYTES) {
            throw new IllegalArgumentException("Blob stream write exceeds maximum chunk size");
        }
        return bytes;
    }

    private byte[] blobRequestBytes(JsonNode payload, String field) {
        String encoded = requiredText(payload, field);
        if (encoded.length() > ((MAX_BLOB_BYTES + 2L) / 3L) * 4L) {
            throw new IllegalArgumentException(field + " exceeds maximum request size");
        }
        byte[] bytes = Base64.getDecoder().decode(encoded);
        if (bytes.length > MAX_BLOB_BYTES) {
            throw new IllegalArgumentException(field + " exceeds maximum request size");
        }
        return bytes;
    }

    /** 将驱动 Array#getResultSet 作为独立子游标分页输出。 */
    private JsonNode arrayResultSet(AgentSession session, JsonNode payload)
            throws SQLException, IOException {
        if (payload.has("typeMap")) {
            throw new SQLFeatureNotSupportedException("Array#getResultSet(Map) is not supported", "0A000");
        }
        String arrayId = requiredText(payload, "arrayId");
        Array array = session.array(arrayId);
        boolean hasIndex = payload.has("index");
        boolean hasCount = payload.has("count");
        if (hasIndex != hasCount) {
            throw new IllegalArgumentException("index and count must be supplied together");
        }
        ResultSet resultSet = hasIndex
                ? array.getResultSet(requiredLong(payload, "index"), requiredInt(payload, "count"))
                : array.getResultSet();
        AgentCursor cursor;
        try {
            cursor = new AgentCursor(resultSet, arrayId, objectMapper);
        } catch (SQLException | RuntimeException exception) {
            try {
                resultSet.close();
            } catch (SQLException closeFailure) {
                exception.addSuppressed(closeFailure);
            }
            throw exception;
        }
        return firstPage(session, cursor, payload);
    }

    private JsonNode executeUpdate(long requestId, AgentSession session, JsonNode payload)
            throws SQLException, IOException {
        try (PreparedStatement statement = session.connection().prepareStatement(
                requiredText(payload, "sql"), Statement.RETURN_GENERATED_KEYS)) {
            bind(statement, payload.path("params"));
            session.activate(requestId, statement);
            try {
                return updateResult(statement, Math.max(0, statement.executeUpdate()), true);
            } finally {
                session.deactivate(requestId);
            }
        }
    }

    private JsonNode executeQuery(long requestId, AgentSession session, JsonNode payload)
            throws SQLException, IOException {
        PreparedStatement statement = session.connection().prepareStatement(requiredText(payload, "sql"));
        try {
            bind(statement, payload.path("params"));
            session.activate(requestId, statement);
            ResultSet resultSet;
            try {
                resultSet = statement.executeQuery();
            } finally {
                session.deactivate(requestId);
            }
            return firstPage(session, new AgentCursor(
                    statement, resultSet, true, null, objectMapper), payload);
        } catch (SQLException | IOException | RuntimeException exception) {
            statement.close();
            throw exception;
        }
    }

    private JsonNode execute(long requestId, AgentSession session, JsonNode payload)
            throws SQLException, IOException {
        PreparedStatement statement = prepareForExecute(
                session.connection(), requiredText(payload, "sql"), payload.path("generatedKeys"));
        try {
            bind(statement, payload.path("params"));
            session.activate(requestId, statement);
            boolean query;
            try {
                query = statement.execute();
            } finally {
                session.deactivate(requestId);
            }
            if (query) {
                return firstPage(session, new AgentCursor(
                        statement, statement.getResultSet(), true, null, objectMapper), payload);
            }
            ObjectNode result = updateResult(
                    statement,
                    Math.max(0, statement.getUpdateCount()),
                    generatedKeysRequested(payload));
            statement.close();
            return result;
        } catch (SQLException | IOException | RuntimeException exception) {
            statement.close();
            throw exception;
        }
    }

    private JsonNode prepare(AgentSession session, JsonNode payload) throws SQLException {
        PreparedStatement statement = prepareForExecute(
                session.connection(), requiredText(payload, "sql"), payload.path("generatedKeys"));
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        result.put("statementId", session.registerPreparedStatement(statement));
        return result;
    }

    private JsonNode executePrepared(
            long requestId,
            AgentSession session,
            JsonNode payload) throws SQLException, IOException {
        String statementId = requiredText(payload, "statementId");
        PreparedStatement statement = session.preparedStatement(statementId);
        statement.clearParameters();
        statement.setQueryTimeout(Math.max(0, payload.path("queryTimeoutSeconds").asInt(0)));
        bind(statement, payload.path("params"));
        String mode = payload.path("mode").asText("execute");
        session.activate(requestId, statement);
        try {
            return switch (mode) {
                case "query" -> firstPage(session, new AgentCursor(
                        statement, statement.executeQuery(), false, statementId, objectMapper), payload);
                case "update" -> updateResult(
                        statement,
                        Math.max(0, statement.executeUpdate()),
                        generatedKeysRequested(payload));
                case "execute" -> executePreparedAny(session, statementId, statement, payload);
                default -> throw new IllegalArgumentException("unsupported execute_prepared mode " + mode);
            };
        } finally {
            session.deactivate(requestId);
        }
    }

    private JsonNode executePreparedAny(
            AgentSession session,
            String statementId,
            PreparedStatement statement,
            JsonNode payload) throws SQLException, IOException {
        if (statement.execute()) {
            return firstPage(session, new AgentCursor(
                    statement, statement.getResultSet(), false, statementId, objectMapper), payload);
        }
        return updateResult(
                statement,
                Math.max(0, statement.getUpdateCount()),
                generatedKeysRequested(payload));
    }

    private JsonNode executePreparedBatch(
            long requestId,
            AgentSession session,
            JsonNode payload) throws SQLException, IOException {
        String statementId = requiredText(payload, "statementId");
        PreparedStatement statement = session.preparedStatement(statementId);
        JsonNode parameterSets = payload.path("parameterSets");
        if (!parameterSets.isArray()) {
            throw new IllegalArgumentException("parameterSets must be an array");
        }
        statement.clearBatch();
        for (JsonNode parameters : parameterSets) {
            statement.clearParameters();
            bind(statement, parameters);
            statement.addBatch();
        }
        statement.setQueryTimeout(Math.max(0, payload.path("queryTimeoutSeconds").asInt(0)));
        session.activate(requestId, statement);
        try {
            int[] updateCounts = statement.executeBatch();
            ObjectNode result = JsonNodeFactory.instance.objectNode();
            ArrayNode counts = result.putArray("updateCounts");
            for (int updateCount : updateCounts) {
                counts.add(updateCount);
            }
            return result;
        } finally {
            session.deactivate(requestId);
            try {
                statement.clearBatch();
            } catch (SQLException exception) {
                log.debug("Unable to clear JDBC batch after execution", exception);
            }
        }
    }

    private JsonNode closeStatement(AgentSession session, JsonNode payload) throws SQLException {
        session.closePreparedStatement(requiredText(payload, "statementId"));
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode firstPage(AgentSession session, AgentCursor cursor, JsonNode payload)
            throws SQLException, IOException {
        String cursorId = UUID.randomUUID().toString();
        try {
            ObjectNode page = cursor.fetchPage(
                    cursorId, pageSize(payload), responseBytes(payload));
            if (cursor.hasMore()) {
                session.registerCursor(cursorId, cursor);
            }
            return page;
        } catch (SQLException | IOException | RuntimeException exception) {
            cursor.close();
            throw exception;
        }
    }

    private JsonNode fetchPage(long requestId, AgentSession session, JsonNode payload)
            throws SQLException, IOException {
        String cursorId = requiredText(payload, "cursorId");
        AgentCursor cursor = session.cursor(cursorId);
        if (Objects.nonNull(cursor.statement())) {
            session.activate(requestId, cursor.statement());
        }
        try {
            ObjectNode page = cursor.fetchPage(
                    cursorId, pageSize(payload), responseBytes(payload));
            if (!cursor.hasMore()) {
                session.closeCursor(cursorId);
            }
            return page;
        } finally {
            session.deactivate(requestId);
        }
    }

    private JsonNode closeCursor(AgentSession session, JsonNode payload) throws SQLException {
        session.closeCursor(requiredText(payload, "cursorId"));
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode cancel(JsonNode payload) throws SQLException {
        AgentSession session = sessions.get(requiredText(payload, "sessionId"));
        boolean cancelled = false;
        if (Objects.nonNull(session)) {
            String statementId = nullableText(payload, "statementId");
            cancelled = Objects.nonNull(statementId)
                    ? session.cancelPreparedStatement(statementId)
                    : session.cancel(payload.path("targetRequestId").asLong());
        }
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        result.put("cancelled", cancelled);
        return result;
    }

    private PreparedStatement prepareForExecute(
            Connection connection,
            String sql,
            JsonNode generatedKeys) throws SQLException {
        String mode = generatedKeys.path("mode").asText("none");
        return switch (mode) {
            case "auto" -> connection.prepareStatement(sql, generatedKeys.path("value").asInt());
            case "column_indexes" -> connection.prepareStatement(sql, intArray(generatedKeys.path("value")));
            case "column_names" -> connection.prepareStatement(sql, stringArray(generatedKeys.path("value")));
            case "none" -> connection.prepareStatement(sql);
            default -> throw new IllegalArgumentException("unsupported generatedKeys mode " + mode);
        };
    }

    private boolean generatedKeysRequested(JsonNode payload) {
        return !"none".equals(payload.path("generatedKeys").path("mode").asText("none"));
    }

    private JsonNode begin(AgentSession session) throws SQLException {
        session.connection().setAutoCommit(false);
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode commit(AgentSession session) throws SQLException {
        session.connection().commit();
        session.clearSavepoints();
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode rollback(AgentSession session) throws SQLException {
        session.connection().rollback();
        session.clearSavepoints();
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode setSavepoint(AgentSession session, JsonNode payload) throws SQLException {
        String name = nullableText(payload, "name");
        java.sql.Savepoint savepoint = Objects.isNull(name)
                ? session.connection().setSavepoint()
                : session.connection().setSavepoint(name);
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        result.put("savepointId", session.registerSavepoint(savepoint));
        return result;
    }

    private JsonNode rollbackToSavepoint(AgentSession session, JsonNode payload) throws SQLException {
        session.connection().rollback(session.savepoint(requiredText(payload, "savepointId")));
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode releaseSavepoint(AgentSession session, JsonNode payload) throws SQLException {
        session.releaseSavepoint(requiredText(payload, "savepointId"));
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode validateConnection(AgentSession session, JsonNode params) throws SQLException {
        int timeoutSeconds = params.path("timeoutSeconds").asInt(5);
        if (timeoutSeconds < 0) {
            throw new SQLException("JDBC validation timeout must be non-negative");
        }
        try {
            if (session.connection().isValid(timeoutSeconds)) {
                return JsonNodeFactory.instance.objectNode();
            }
        } catch (SQLFeatureNotSupportedException ignored) {
            log.debug("JDBC driver does not implement Connection.isValid");
        }
        if (Objects.nonNull(session.validationQuery())) {
            try (Statement statement = session.connection().createStatement()) {
                statement.execute(session.validationQuery());
                return JsonNodeFactory.instance.objectNode();
            }
        }
        throw new SQLException("JDBC connection validation failed");
    }

    private JsonNode setAutoCommit(AgentSession session, JsonNode payload) throws SQLException {
        session.connection().setAutoCommit(payload.path("value").asBoolean());
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode setReadOnly(AgentSession session, JsonNode payload) throws SQLException {
        session.connection().setReadOnly(payload.path("value").asBoolean());
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode setTransactionIsolation(AgentSession session, JsonNode payload) throws SQLException {
        session.connection().setTransactionIsolation(payload.path("value").asInt());
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode setCatalog(AgentSession session, JsonNode payload) throws SQLException {
        session.connection().setCatalog(nullableText(payload, "value"));
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode setSchema(AgentSession session, JsonNode payload) throws SQLException {
        session.connection().setSchema(nullableText(payload, "value"));
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode listCatalogs(AgentSession session) throws SQLException {
        ArrayNode values = JsonNodeFactory.instance.arrayNode();
        try (ResultSet resultSet = session.connection().getMetaData().getCatalogs()) {
            while (resultSet.next()) {
                values.add(resultSet.getString(1));
            }
        }
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        result.set("values", values);
        return result;
    }

    private JsonNode listSchemas(AgentSession session) throws SQLException {
        ArrayNode values = JsonNodeFactory.instance.arrayNode();
        try (ResultSet resultSet = session.connection().getMetaData().getSchemas()) {
            while (resultSet.next()) {
                values.add(resultSet.getString(1));
            }
        }
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        result.set("values", values);
        return result;
    }

    private JsonNode databaseMetadata(AgentSession session) throws SQLException {
        DatabaseMetaData metadata = session.connection().getMetaData();
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        result.put("driverName", metadata.getDriverName());
        result.put("driverVersion", metadata.getDriverVersion());
        result.put("databaseProductName", metadata.getDatabaseProductName());
        result.put("databaseProductVersion", metadata.getDatabaseProductVersion());
        result.put("url", metadata.getURL());
        result.put("userName", metadata.getUserName());
        result.put("supportsTransactions", metadata.supportsTransactions());
        return result;
    }

    private ObjectNode updateResult(
            PreparedStatement statement,
            long rowsAffected,
            boolean generatedKeysRequested) throws SQLException {
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        result.put("kind", "update");
        result.put("rowsAffected", rowsAffected);
        if (!generatedKeysRequested) {
            return result;
        }
        try (ResultSet generatedKeys = statement.getGeneratedKeys()) {
            if (Objects.nonNull(generatedKeys) && generatedKeys.next()) {
                long value = generatedKeys.getLong(1);
                if (!generatedKeys.wasNull()) {
                    result.put("lastInsertId", value);
                }
            }
        } catch (SQLFeatureNotSupportedException ignored) {
            log.debug("JDBC driver does not expose generated keys");
        } catch (SQLException exception) {
            // The update has already succeeded. Some JDBC drivers (including
            // HSQLDB for statements that produce no key) report an invalid
            // cursor instead of returning an empty generated-key ResultSet.
            // Preserve rowsAffected and represent the optional key as absent.
            log.debug("JDBC driver could not read generated keys", exception);
        }
        return result;
    }

    private void bind(PreparedStatement statement, JsonNode parameters) throws SQLException, IOException {
        if (!parameters.isArray()) {
            throw new IllegalArgumentException("params must be an array");
        }
        for (int index = 0; index < parameters.size(); index++) {
            AgentValue value = objectMapper.treeToValue(parameters.get(index), AgentValue.class);
            bindValue(statement, index + 1, value);
        }
    }

    private void bindValue(PreparedStatement statement, int index, AgentValue value) throws SQLException {
        String kind = Objects.requireNonNull(value.kind(), "value kind");
        JsonNode node = value.value();
        switch (kind) {
            case "null" -> statement.setObject(index, null);
            case "bool" -> statement.setBoolean(index, node.asBoolean());
            case "int" -> statement.setLong(index, node.asLong());
            case "float" -> statement.setDouble(index, node.asDouble());
            case "decimal" -> statement.setBigDecimal(index, new BigDecimal(node.asText()));
            case "date" -> statement.setDate(index, Date.valueOf(LocalDate.parse(node.asText())));
            case "time" -> statement.setTime(index, Time.valueOf(LocalTime.parse(node.asText())));
            case "timestamp" -> statement.setTimestamp(index, Timestamp.valueOf(LocalDateTime.parse(node.asText())));
            case "string" -> statement.setString(index, node.asText());
            case "bytes" -> statement.setBytes(index, Base64.getDecoder().decode(node.asText()));
            default -> throw new IllegalArgumentException("unsupported AgentValue kind " + kind);
        }
    }

    private ObjectNode value(boolean value) {
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        result.put("value", value);
        return result;
    }

    private ObjectNode value(int value) {
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        result.put("value", value);
        return result;
    }

    private ObjectNode nullableValue(String value) {
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        putNullable(result, "value", value);
        return result;
    }

    private void putNullable(ObjectNode target, String field, String value) {
        if (Objects.isNull(value)) {
            target.putNull(field);
        } else {
            target.put(field, value);
        }
    }

    private String requiredText(JsonNode payload, String field) {
        Objects.requireNonNull(payload, "payload");
        JsonNode value = Objects.requireNonNull(payload.get(field), field);
        String text = value.asText();
        if (text.isBlank()) {
            throw new IllegalArgumentException(field + " must not be blank");
        }
        return text;
    }

    private String nullableText(JsonNode payload, String field) {
        if (Objects.isNull(payload)) {
            return null;
        }
        JsonNode value = payload.get(field);
        return Objects.isNull(value) || value.isNull() ? null : value.asText();
    }

    private int pageSize(JsonNode payload) {
        int requested = payload.path("pageSize").asInt(DEFAULT_PAGE_SIZE);
        return Math.max(1, Math.min(requested, MAX_PAGE_SIZE));
    }

    private int responseBytes(JsonNode payload) {
        int requested = payload.path("maxResponseBytes").asInt(DEFAULT_RESPONSE_BYTES);
        return Math.max(MIN_RESPONSE_BYTES, Math.min(requested, DEFAULT_RESPONSE_BYTES));
    }

    private int[] intArray(JsonNode values) {
        int[] result = new int[values.size()];
        for (int index = 0; index < values.size(); index++) {
            result[index] = values.get(index).asInt();
        }
        return result;
    }

    private String[] stringArray(JsonNode values) {
        String[] result = new String[values.size()];
        for (int index = 0; index < values.size(); index++) {
            result[index] = values.get(index).asText();
        }
        return result;
    }

    private String safeCatalog(Connection connection) {
        try {
            return connection.getCatalog();
        } catch (SQLException | AbstractMethodError ignored) {
            return null;
        }
    }

    private String safeSchema(Connection connection) {
        try {
            return connection.getSchema();
        } catch (SQLException | AbstractMethodError ignored) {
            return null;
        }
    }

    private boolean supportsCatalog(DatabaseMetaData metadata) {
        try {
            return metadata.supportsCatalogsInDataManipulation();
        } catch (SQLException | AbstractMethodError ignored) {
            return false;
        }
    }

    private boolean supportsSchema(DatabaseMetaData metadata) {
        try {
            return metadata.supportsSchemasInDataManipulation();
        } catch (SQLException | AbstractMethodError ignored) {
            return false;
        }
    }

    private synchronized void writeLine(JsonNode value) throws IOException {
        String line = objectMapper.writeValueAsString(value);
        if (line.getBytes(StandardCharsets.UTF_8).length > MAX_FRAME_BYTES) {
            throw new IOException("JSON-RPC response exceeds maximum frame size");
        }
        output.write(line);
        output.newLine();
        output.flush();
    }

    private void requestShutdown() {
        if (!shuttingDown.compareAndSet(false, true)) {
            return;
        }
        try {
            input.close();
        } catch (IOException exception) {
            log.debug("Unable to close JDBC Agent input during shutdown", exception);
        }
    }

    private void awaitRequests() {
        try {
            if (!requests.awaitTermination(10, TimeUnit.SECONDS)) {
                requests.shutdownNow();
            }
        } catch (InterruptedException exception) {
            Thread.currentThread().interrupt();
            requests.shutdownNow();
        }
    }

    /** 关闭全部 session、请求执行器和协议流。 */
    @Override
    public void close() throws IOException {
        if (!closed.compareAndSet(false, true)) {
            return;
        }
        requestShutdown();
        requests.shutdownNow();
        IOException failure = null;
        for (AgentSession session : sessions.values()) {
            try {
                session.close();
            } catch (IOException exception) {
                failure = exception;
            }
        }
        sessions.clear();
        try {
            output.close();
        } catch (IOException exception) {
            failure = exception;
        }
        try {
            input.close();
        } catch (IOException exception) {
            failure = exception;
        }
        if (Objects.nonNull(failure)) {
            throw failure;
        }
    }
}
