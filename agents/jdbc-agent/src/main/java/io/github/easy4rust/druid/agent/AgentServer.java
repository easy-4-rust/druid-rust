package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.JsonNodeFactory;
import com.fasterxml.jackson.databind.node.ObjectNode;
import lombok.extern.slf4j.Slf4j;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.Closeable;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.OutputStreamWriter;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.sql.Connection;
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

/** JSON-RPC 2.0 NDJSON 服务端；一个共享进程可隔离承载多个 JDBC session。 */
@Slf4j
public final class AgentServer implements Closeable {

    private static final int PROTOCOL_VERSION = 1;
    private static final int MAX_FRAME_BYTES = 16 * 1024 * 1024;
    private static final int DEFAULT_PAGE_SIZE = 500;
    private static final int MAX_PAGE_SIZE = 10_000;
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
            "native-prepared-batch");

    private final BufferedReader input;
    private final BufferedWriter output;
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
        this.input = new BufferedReader(new InputStreamReader(
                Objects.requireNonNull(input, "input"), StandardCharsets.UTF_8));
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
            while (!shuttingDown.get() && Objects.nonNull(line = input.readLine())) {
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
            try {
                writeLine(objectMapper.valueToTree(
                        AgentResponse.failure(requestId, sessionId, exception)));
            } catch (IOException writeException) {
                log.error("Unable to write JDBC Agent error response", writeException);
                requestShutdown();
            }
            log.debug("JDBC operation failed: requestId={}", requestId, exception);
        }
    }

    private void validateRequest(AgentRequest request) {
        Objects.requireNonNull(request, "request");
        Objects.requireNonNull(request.method(), "method");
        Objects.requireNonNull(request.params(), "params");
        if (!"2.0".equals(request.jsonrpc())) {
            throw new IllegalArgumentException("unsupported jsonrpc version " + request.jsonrpc());
        }
    }

    private JsonNode dispatch(AgentRequest request) throws SQLException, IOException {
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
            JsonNode params) throws SQLException, IOException {
        return switch (method) {
            case "validate_connection" -> validateConnection(session);
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
            default -> throw new UnsupportedOperationException("unsupported method " + method);
        };
    }

    private JsonNode handshake(JsonNode params) {
        if (params.path("protocolVersion").asInt() != PROTOCOL_VERSION) {
            throw new IllegalArgumentException("unsupported protocolVersion "
                    + params.path("protocolVersion").asInt());
        }
        JsonNode requestedCapabilities = params.path("capabilities");
        if (!requestedCapabilities.isArray()) {
            throw new IllegalArgumentException("handshake capabilities must be an array");
        }
        for (JsonNode capability : requestedCapabilities) {
            String name = capability.asText();
            if (!AGENT_CAPABILITIES.contains(name)) {
                throw new UnsupportedOperationException(
                        "unsupported JDBC Agent capability " + name);
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
        String url = requiredText(payload, "url");
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
        Connection connection = DriverManager.getConnection(url, properties);
        try {
            DatabaseMetaData metadata = connection.getMetaData();
            String sessionId = UUID.randomUUID().toString();
            sessions.put(sessionId, new AgentSession(connection, validationQuery));
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
            return result;
        } catch (SQLException | RuntimeException exception) {
            connection.close();
            throw exception;
        }
    }

    private JsonNode closeSession(JsonNode payload) throws IOException {
        AgentSession session = sessions.remove(requiredText(payload, "sessionId"));
        if (Objects.nonNull(session)) {
            session.close();
        }
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode withSession(JsonNode payload, SessionOperation operation)
            throws SQLException, IOException {
        String sessionId = requiredText(payload, "sessionId");
        AgentSession session = sessions.get(sessionId);
        if (Objects.isNull(session) || session.connection().isClosed()) {
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
        JsonNode execute(AgentSession session) throws SQLException, IOException;
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
        session.activate(requestId, cursor.statement());
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

    private JsonNode validateConnection(AgentSession session) throws SQLException {
        try {
            if (session.connection().isValid(5)) {
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
